//! The scheduler module manages the distribution of mining jobs to hash boards
//! and ASIC chips.
//!
//! This is a work-in-progress. It's currently the main and initial place where
//! functionality is added, after which the functionality is refactored out to
//! where it belongs.

use tokio_serial::{self, SerialPortBuilderExt};
use tokio_util::sync::CancellationToken;

use crate::board::{bitaxe::BitaxeBoard, Board};
use crate::tracing::prelude::*;

const CONTROL_SERIAL: &str = "/dev/ttyACM0";
const DATA_SERIAL: &str = "/dev/ttyACM1";

pub async fn task(running: CancellationToken) {
    trace!("Scheduler task started.");

    // In the future, a DeviceManager would create boards based on USB detection
    // For now, we'll create a single board with known serial ports
    let control_port = tokio_serial::new(CONTROL_SERIAL, 115200)
        .open_native_async()
        .expect("failed to open control serial port");
    
    let data_port = tokio_serial::new(DATA_SERIAL, 115200)
        .open_native_async()
        .expect("failed to open data serial port");
    
    let mut board = BitaxeBoard::new(control_port, data_port);
    
    // Initialize the board (reset + chip discovery)
    match board.initialize().await {
        Ok(()) => {
            info!("Board initialized successfully");
            info!("Found {} chip(s)", board.chips().len());
        }
        Err(e) => {
            error!("Failed to initialize board: {e}");
            return;
        }
    }
    
    // Main scheduler loop
    info!("Starting mining scheduler");
    
    while !running.is_cancelled() {
        // TODO: Main mining loop
        // 1. Get work from pool
        // 2. Distribute to chips via board.chips_mut()
        // 3. Collect nonces
        // 4. Submit shares
        
        // For now, just wait
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                trace!("Scheduler heartbeat");
            }
            _ = running.cancelled() => {
                info!("Scheduler shutdown requested");
                break;
            }
        }
    }
    
    trace!("Scheduler task stopped.");
}