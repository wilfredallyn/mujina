//! HashThread abstraction for schedulable mining workers.
//!
//! A HashThread represents a schedulable group of hashing engines that work
//! together to execute mining tasks. The scheduler assigns work to HashThreads
//! without needing to know about the underlying hardware topology (single chip,
//! chip chain, engine groups, etc.).
//!
//! # Share Processing (Three-Layer Filtering)
//!
//! Share filtering happens at three independent levels:
//!
//! 1. **Chip TicketMask (hardware pre-filter):**
//!    Thread configures chip with low difficulty for frequent health signals.
//!    Chip only reports nonces meeting this hardware threshold.
//!
//! 2. **HashTask.share_target (thread-to-scheduler filter):**
//!    Scheduler sets when assigning work. Thread computes hash for every chip
//!    nonce and emits ShareFound only for shares meeting task.share_target.
//!    Controls message volume to scheduler.
//!
//! 3. **JobTemplate.share_target (scheduler-to-source filter):**
//!    Scheduler performs final filtering before submission. Only shares
//!    meeting this threshold are forwarded to the source.
//!
//! This provides scheduler with frequent monitoring data (task.share_target)
//! while limiting pool submissions (template.share_target). Message volume
//! is manageable: ~1-2 shares/sec to scheduler, fewer to pool.

use std::sync::Arc;

use async_trait::async_trait;
use bitcoin::block::Version;
use bitcoin::pow::Target;
use bitcoin::BlockHash;
use tokio::sync::mpsc;

use crate::job_source::{Extranonce2, Extranonce2Range};
use crate::scheduler::ActiveJob;
use crate::types::HashRate;

/// HashThread capabilities reported to scheduler for work assignment decisions.
#[derive(Debug, Clone)]
pub struct HashThreadCapabilities {
    /// Estimated hashrate
    pub hashrate_estimate: HashRate,
    // Future capabilities:
    // pub can_roll_version: bool,
    // pub version_roll_bits: u32,
    // pub can_roll_ntime: bool,
    // pub ntime_range: Option<std::ops::Range<u32>>,
    // pub can_iterate_extranonce2: bool,
}

/// Current runtime status of a HashThread.
#[derive(Debug, Clone, Default)]
pub struct HashThreadStatus {
    /// Current hashrate estimate
    pub hashrate: HashRate,

    /// Number of shares found (at chip target level, before pool filtering)
    pub chip_shares_found: u64,

    /// Number of shares submitted to pool (after filtering)
    pub pool_shares_submitted: u64,

    /// Number of hardware errors detected
    pub hardware_errors: u64,

    /// Current chip temperature if available
    pub temperature_c: Option<f32>,

    /// Whether thread is actively working
    pub is_active: bool,
}

/// Events emitted by HashThreads back to the scheduler.
///
/// When a thread shuts down (USB unplug, fault, user request, etc.), it closes
/// its event channel instead of sending an event. The scheduler detects channel
/// closure and handles thread removal.
#[derive(Debug)]
pub enum HashThreadEvent {
    /// Valid share found (already filtered by pool_target)
    ShareFound(Share),

    /// Work approaching exhaustion (warning to scheduler)
    WorkDepletionWarning {
        /// Estimated remaining time in milliseconds
        estimated_remaining_ms: u64,
    },

    /// Work completely exhausted
    WorkExhausted {
        /// Number of EN2 values searched
        en2_searched: u64,
    },

    /// Periodic status update
    StatusUpdate(HashThreadStatus),
}

/// Error types for HashThread operations.
#[derive(Debug, thiserror::Error)]
pub enum HashThreadError {
    #[error("Thread has been shut down")]
    ThreadOffline,

    #[error("Channel closed: {0}")]
    ChannelClosed(String),

    #[error("Work assignment failed: {0}")]
    WorkAssignmentFailed(String),

    #[error("Preemption failed: {0}")]
    PreemptionFailed(String),

    #[error("Shutdown timeout")]
    ShutdownTimeout,

    #[error("Chip initialization failed: {0}")]
    InitializationFailed(String),
}

// ---------------------------------------------------------------------------
// Hardware abstraction traits for hash threads
// ---------------------------------------------------------------------------

/// ASIC enable/disable control.
///
/// Hash threads use this to enable chips during initialization and disable
/// them during shutdown. The underlying mechanism (reset pin, power gate,
/// etc.) is an implementation detail.
#[async_trait]
pub trait AsicEnable: Send + Sync {
    /// Enable the ASIC (allow it to run).
    async fn enable(&mut self) -> anyhow::Result<()>;

    /// Disable the ASIC (put it in a safe, non-hashing state).
    async fn disable(&mut self) -> anyhow::Result<()>;
}

/// Voltage regulator control for ASIC core voltage.
///
/// Hash threads may use this to adjust voltage for tuning.
#[async_trait]
pub trait VoltageRegulator: Send + Sync {
    /// Set output voltage in volts.
    async fn set_voltage(&mut self, volts: f32) -> anyhow::Result<()>;
}

/// Hardware interfaces provided by the board to the hash thread.
///
/// Bundles optional hardware capabilities. Not all boards provide all
/// interfaces. Hash threads should handle missing capabilities gracefully.
pub struct BoardPeripherals {
    /// ASIC enable/disable control
    pub asic_enable: Option<Box<dyn AsicEnable>>,

    /// Voltage regulator control
    pub voltage_regulator: Option<Box<dyn VoltageRegulator>>,
}

/// Signal from board to hash thread for shutdown coordination.
///
/// Board sends this via watch channel to signal thread shutdown. Thread checks
/// in its select loop and exits when signal changes from Running.
///
/// The reason is useful for logging but thread behavior is identical for all
/// non-Running variants: clean up and exit.
#[derive(Clone, Debug, PartialEq)]
pub enum ThreadRemovalSignal {
    /// Thread should continue running normally
    Running,

    /// Board was unplugged from USB
    BoardDisconnected,

    /// Board detected hardware fault (overheating, power issue, etc.)
    HardwareFault { description: String },

    /// User requested board disable via API
    UserRequested,

    /// Graceful system shutdown
    Shutdown,
}

/// HashThread trait - the scheduler's view of a schedulable worker.
///
/// A HashThread represents a group of hashing engines that can be assigned work
/// as a unit. The scheduler interacts with threads through this trait without
/// needing to know about the underlying hardware topology.
///
/// Threads are autonomous actors that:
/// - Operate their hardware
/// - Report events asynchronously
#[async_trait]
pub trait HashThread: Send {
    /// Get thread capabilities for scheduling decisions
    fn capabilities(&self) -> &HashThreadCapabilities;

    /// Update current work (shares from old work still valid)
    ///
    /// Thread continues hashing old work until new work is ready. Late-arriving
    /// shares from the old work can still be submitted (they're valuable).
    /// Returns the old task for potential resumption (None if thread was idle).
    ///
    /// Used when pool sends updated job (difficulty change, new transactions in
    /// mempool) but the work is fundamentally still valid.
    async fn update_work(
        &mut self,
        new_work: HashTask,
    ) -> std::result::Result<Option<HashTask>, HashThreadError>;

    /// Replace current work (old work invalidated)
    ///
    /// Old work is immediately invalid - discard it and don't submit shares
    /// from it. Returns the old task for tracking purposes (None if thread was
    /// idle).
    ///
    /// Used when blockchain tip changes (new prevhash) or pool signals
    /// clean_jobs.
    async fn replace_work(
        &mut self,
        new_work: HashTask,
    ) -> std::result::Result<Option<HashTask>, HashThreadError>;

    /// Put thread in idle state (low power, no hashing)
    ///
    /// Returns the current task if thread was working (None if already idle).
    /// Thread enters low-power mode, stops hashing.
    async fn go_idle(&mut self) -> std::result::Result<Option<HashTask>, HashThreadError>;

    /// Take ownership of the event receiver for this thread
    ///
    /// Called once by scheduler after thread creation. The scheduler uses this
    /// to receive events (shares, status updates, etc.) from the thread.
    fn take_event_receiver(&mut self) -> Option<mpsc::Receiver<HashThreadEvent>>;

    /// Get current runtime status
    ///
    /// This is cached and may be slightly stale (updated periodically by
    /// thread's status updates).
    fn status(&self) -> HashThreadStatus;
}

// ---------------------------------------------------------------------------
// Work assignment types
// ---------------------------------------------------------------------------

/// Work assignment from scheduler to hash thread.
///
/// Represents actual mining work from a job source (pool or dummy). Contains
/// the job (template + source association), the extranonce2 range allocated
/// to this thread, and state for resumable work iteration.
///
/// The scheduler maps jobs back to sources via the ActiveJob. Threads don't
/// need to know about sources. If a thread has no HashTask (None), it's idle
/// (low power, no hashing).
#[derive(Debug, Clone)]
pub struct HashTask {
    /// Job to work on (template + source association)
    pub job: Arc<ActiveJob>,

    /// Extranonce2 range allocated to this thread.
    ///
    /// None for header-only mining (Stratum v2). Current HashThread
    /// implementations require EN2 iteration, so None will cause errors
    /// until header-only support is added.
    pub en2_range: Option<Extranonce2Range>,

    /// Extranonce2 value.
    ///
    /// When scheduler assigns work: starting EN2.
    /// When stored as snapshot: the EN2 value that was used.
    /// None for header-only mining (Stratum v2).
    pub en2: Option<Extranonce2>,

    /// Share target for thread-to-scheduler submission threshold.
    ///
    /// Thread emits ShareFound only for shares meeting this target. Allows
    /// scheduler to control message volume independently from pool submission
    /// difficulty. Typically set easier than source threshold for monitoring.
    pub share_target: Target,

    /// Current ntime value
    ///
    /// May be rolled forward during mining. To start, uses the job's time field.
    pub ntime: u32,
}

/// Valid share found by a HashThread.
///
/// Hash has been computed and verified against the job target. Scheduler uses
/// the task reference to route shares back to the originating source.
#[derive(Debug, Clone)]
pub struct Share {
    /// Task this share solves (contains job template and source mapping)
    pub task: Arc<HashTask>,

    /// Winning nonce
    pub nonce: u32,

    /// Computed block hash
    pub hash: BlockHash,

    /// Threshold difficulty this share was validated against.
    ///
    /// This represents the expected hashing work, not the achieved difficulty.
    /// Used for hashrate calculation where each share represents the same
    /// expected work regardless of its actual hash difficulty (which is luck).
    pub threshold_difficulty: f64,

    /// Version bits
    pub version: Version,

    /// Block timestamp
    pub ntime: u32,

    /// Extranonce2 value used (None in, e.g., header-only mining in Stratum v2)
    pub extranonce2: Option<Extranonce2>,
}
