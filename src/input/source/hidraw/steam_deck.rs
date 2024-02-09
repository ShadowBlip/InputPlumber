//! The Deck implementation has been largly based off of the OpenSD project:
//! https://gitlab.com/open-sd/opensd/
use std::error::Error;

use hidapi::DeviceInfo;
use packed_struct::PackedStruct;
use std::ffi::CString;
use tokio::sync::broadcast;

use crate::{
    drivers::steam_deck::hid_report::PackedInputDataReport, input::composite_device::Command,
};

pub const VID: u16 = 0x28de;
pub const PID: u16 = 0x1205;
const PACKET_SIZE: usize = 64;
const HID_TIMEOUT: i32 = 5000;

type Packet = [u8; PACKET_SIZE];

/// Steam Deck Controller implementation of HIDRaw interface
#[derive(Debug)]
pub struct DeckController {
    info: DeviceInfo,
    composite_tx: broadcast::Sender<Command>,
}

impl DeckController {
    pub fn new(info: DeviceInfo, composite_tx: broadcast::Sender<Command>) -> Self {
        Self { info, composite_tx }
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting Steam Deck Controller driver");
        let path = CString::new(self.info.path().to_string_lossy().to_string()).unwrap();

        // Spawn a blocking task to read the events
        let task = tokio::task::spawn_blocking(
            move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let api = hidapi::HidApi::new()?;
                let device = api.open_path(&path)?;
                loop {
                    // Read data from the device into a buffer
                    let mut buf = [0; PACKET_SIZE];
                    let bytes_read = device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

                    // All report descriptors are 64 bytes, so this is just to be safe
                    if bytes_read != PACKET_SIZE {
                        let msg = format!("Invalid input report size was received from gamepad device: {bytes_read}/{PACKET_SIZE}");
                        return Err(msg.into());
                    }

                    // Unpack the input report
                    let input_report = PackedInputDataReport::unpack(&buf)?;
                    log::debug!("----- REPORT Frame: {} ----", input_report.frame);
                    log::debug!("{:?}", input_report);
                    log::debug!("----- END REPORT ----");
                }
            },
        );

        // Wait for the task to finish
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }

        log::debug!("Steam Deck Controller driver stopped");

        Ok(())
    }
}
