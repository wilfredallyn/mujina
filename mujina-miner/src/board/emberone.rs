//! EmberOne mining board support.
//!
//! The EmberOne is a mining board with 12 BM1362 ASIC chips, communicating via
//! USB using the bitaxe-raw protocol (same as Bitaxe boards).

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_serial::SerialPortBuilderExt;
use tokio_util::codec::{FramedRead, FramedWrite};

use super::{
    pattern::{BoardPattern, Match, StringMatch},
    Board, BoardDescriptor, BoardError, BoardEvent, BoardInfo,
};
use crate::{
    asic::{
        bm13xx::{self, thread_v2},
        hash_thread::{AsicEnable, BoardPeripherals, HashThread},
        ChipInfo,
    },
    error::Error,
    hw_trait::gpio::{Gpio, GpioPin, PinValue},
    mgmt_protocol::bitaxe_raw::gpio::{BitaxeRawGpioController, BitaxeRawGpioPin},
    mgmt_protocol::ControlChannel,
    tracing::prelude::*,
    transport::{serial::SerialStream, UsbDeviceInfo},
};

/// EmberOne mining board.
pub struct EmberOne {
    device_info: UsbDeviceInfo,
    control_port_path: String,
    data_port_path: String,
}

impl EmberOne {
    /// GPIO pin number for ASIC reset control (active low).
    const ASIC_RESET_PIN: u8 = 0;

    /// Create a new EmberOne board instance.
    pub fn new(
        device_info: UsbDeviceInfo,
        control_port_path: String,
        data_port_path: String,
    ) -> Result<Self, BoardError> {
        Ok(Self {
            device_info,
            control_port_path,
            data_port_path,
        })
    }
}

#[async_trait]
impl Board for EmberOne {
    async fn reset(&mut self) -> Result<(), BoardError> {
        Err(BoardError::InitializationFailed(
            "EmberOne reset not yet implemented".into(),
        ))
    }

    async fn hold_in_reset(&mut self) -> Result<(), BoardError> {
        Err(BoardError::InitializationFailed(
            "EmberOne hold_in_reset not yet implemented".into(),
        ))
    }

    async fn initialize(&mut self) -> Result<mpsc::Receiver<BoardEvent>, BoardError> {
        Err(BoardError::InitializationFailed(
            "EmberOne initialize not yet implemented".into(),
        ))
    }

    fn chip_count(&self) -> usize {
        // EmberOne has 12 BM1362 chips, but not yet implemented
        0
    }

    fn chip_infos(&self) -> &[ChipInfo] {
        // No chips discovered yet in stub
        &[]
    }

    fn board_info(&self) -> BoardInfo {
        BoardInfo {
            model: "EmberOne".to_string(),
            firmware_version: None,
            serial_number: self.device_info.serial_number.clone(),
        }
    }

    fn take_event_receiver(&mut self) -> Option<mpsc::Receiver<BoardEvent>> {
        // Stub has no event receiver yet
        None
    }

    async fn shutdown(&mut self) -> Result<(), BoardError> {
        Ok(())
    }

    async fn create_hash_threads(&mut self) -> Result<Vec<Box<dyn HashThread>>, BoardError> {
        // Open control port
        let control_port = tokio_serial::new(&self.control_port_path, 115200)
            .open_native_async()
            .map_err(|e| {
                BoardError::InitializationFailed(format!("Failed to open control port: {}", e))
            })?;
        let control_channel = ControlChannel::new(control_port);

        // Get reset pin via GPIO controller
        let mut gpio = BitaxeRawGpioController::new(control_channel.clone());
        let reset_pin = gpio.pin(Self::ASIC_RESET_PIN).await.map_err(|e| {
            BoardError::InitializationFailed(format!("Failed to get reset pin: {}", e))
        })?;

        // Open data port
        let data_stream = SerialStream::new(&self.data_port_path, 115200).map_err(|e| {
            BoardError::InitializationFailed(format!("Failed to open data port: {}", e))
        })?;
        let (data_reader, data_writer, _data_control) = data_stream.split();

        // Create framed reader/writer for BM13xx protocol
        let chip_rx = FramedRead::new(data_reader, bm13xx::FrameCodec);
        let chip_tx = FramedWrite::new(data_writer, bm13xx::FrameCodec);

        // Bundle peripherals for the thread
        let peripherals = BoardPeripherals {
            asic_enable: Some(Box::new(EmberOneAsicEnable { reset_pin })),
            voltage_regulator: None,
        };

        // Create the hash thread (this enumerates chips)
        let thread = thread_v2::BM13xxThread::new(chip_rx, chip_tx, peripherals)
            .await
            .map_err(|e| {
                BoardError::InitializationFailed(format!("Failed to create hash thread: {}", e))
            })?;

        debug!("Created BM13xx hash thread for EmberOne");

        Ok(vec![Box::new(thread)])
    }
}

/// Adapter implementing `AsicEnable` for EmberOne's GPIO-based reset control.
struct EmberOneAsicEnable {
    reset_pin: BitaxeRawGpioPin,
}

#[async_trait]
impl AsicEnable for EmberOneAsicEnable {
    async fn enable(&mut self) -> anyhow::Result<()> {
        // Release reset (nRST is active-low, so High = running)
        self.reset_pin
            .write(PinValue::High)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to release reset: {}", e))
    }

    async fn disable(&mut self) -> anyhow::Result<()> {
        // Assert reset (nRST is active-low, so Low = reset)
        self.reset_pin
            .write(PinValue::Low)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to assert reset: {}", e))
    }
}

// Factory function to create EmberOne board from USB device info
async fn create_from_usb(device: UsbDeviceInfo) -> crate::error::Result<Box<dyn Board + Send>> {
    // Get serial ports
    let serial_ports = device.serial_ports()?;

    // EmberOne uses 2 serial ports like Bitaxe (control + data)
    if serial_ports.len() != 2 {
        return Err(Error::Hardware(format!(
            "EmberOne requires exactly 2 serial ports, found {}",
            serial_ports.len()
        )));
    }

    // Clone paths before moving device
    let control_port = serial_ports[0].clone();
    let data_port = serial_ports[1].clone();

    debug!(
        serial = ?device.serial_number,
        control = %control_port,
        data = %data_port,
        "EmberOne serial ports"
    );

    let board = EmberOne::new(device, control_port, data_port)
        .map_err(|e| Error::Hardware(format!("Failed to create board: {}", e)))?;

    Ok(Box::new(board))
}

// Register this board type with the inventory system
inventory::submit! {
    BoardDescriptor {
        pattern: BoardPattern {
            vid: Match::Any,
            pid: Match::Any,
            manufacturer: Match::Specific(StringMatch::Exact("256F")),
            product: Match::Specific(StringMatch::Exact("EmberOne00")),
            serial_pattern: Match::Any,
        },
        name: "EmberOne",
        create_fn: |device| Box::pin(create_from_usb(device)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_creation() {
        let device = UsbDeviceInfo::new_for_test(
            0xc0de,
            0xcafe,
            Some("TEST001".to_string()),
            Some("EmberOne".to_string()),
            Some("Mining Board".to_string()),
            "/sys/devices/test".to_string(),
        );

        let board = EmberOne::new(
            device,
            "/dev/ttyACM0".to_string(),
            "/dev/ttyACM1".to_string(),
        );
        assert!(board.is_ok());

        let board = board.unwrap();
        assert_eq!(board.chip_count(), 0);
        assert_eq!(board.board_info().model, "EmberOne");
    }
}
