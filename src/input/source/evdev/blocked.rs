use std::{error::Error, fmt::Debug};

use evdev::Device;

use crate::{
    input::{
        capability::Capability,
        event::native::NativeEvent,
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Source device implementation to block evdev events
pub struct BlockedEventDevice {
    _device: Device,
}

impl BlockedEventDevice {
    /// Create a new [BlockedEventDevice] source device from the given udev info
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = device_info.devnode();
        log::debug!("Opening device at: {}", path);
        let mut device = Device::open(path.clone())?;
        device.grab()?;
        log::info!("Blocking input events from {path}");

        Ok(Self { _device: device })
    }
}

impl SourceInputDevice for BlockedEventDevice {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        Ok(vec![])
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![])
    }
}

impl SourceOutputDevice for BlockedEventDevice {}

impl Debug for BlockedEventDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockedEventDevice").finish()
    }
}
