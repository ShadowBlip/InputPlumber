use std::{error::Error, fmt::Debug};

use crate::{
    input::source::{SourceInputDevice, SourceOutputDevice},
    udev::device::UdevDevice,
};

/// Source device implementation to block hidraw events
#[derive(Debug)]
pub struct BlockedHidrawDevice {}

impl BlockedHidrawDevice {
    /// Create a new [BlockedHidrawDevice] source device from the given udev info
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = device_info.devnode();
        log::info!("Blocking input events from {path}");

        Ok(Self {})
    }
}

impl SourceInputDevice for BlockedHidrawDevice {}

impl SourceOutputDevice for BlockedHidrawDevice {}
