use std::{error::Error, fmt::Debug};

use crate::{
    drivers::rog_ally::driver::Driver,
    input::source::{SourceInputDevice, SourceOutputDevice},
    udev::device::UdevDevice,
};

/// XpadUhid source device implementation
pub struct RogAlly {
    _driver: Driver,
}

impl RogAlly {
    /// Create a new source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info)?;
        Ok(Self { _driver: driver })
    }
}

impl Debug for RogAlly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RogAlly").finish()
    }
}
impl SourceInputDevice for RogAlly {}

impl SourceOutputDevice for RogAlly {}
