use std::{error::Error, fmt::Debug};

use crate::{input::source::SourceOutputDevice, udev::device::UdevDevice};

/// OrangePi Neo Touchpad source device implementation
pub struct MultiColorChassis {
    device_info: UdevDevice,
}

impl MultiColorChassis {
    /// Create a new OrangePi Neo touchscreen source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(Self { device_info })
    }
}

impl SourceOutputDevice for MultiColorChassis {}

impl Debug for MultiColorChassis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiColorChassis").finish()
    }
}
