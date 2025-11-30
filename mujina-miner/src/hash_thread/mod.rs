//! HashThread abstraction for schedulable mining workers.
//!
//! This module re-exports types from `crate::asic::hash_thread` for backwards
//! compatibility. New code should import from `asic::hash_thread` directly.

pub mod bm13xx;
pub mod task;

// Re-export everything from the canonical location
pub use crate::asic::hash_thread::{
    AsicEnable, BoardPeripherals, HashTask, HashThread, HashThreadCapabilities, HashThreadError,
    HashThreadEvent, HashThreadStatus, Share, ThreadRemovalSignal, VoltageRegulator,
};
