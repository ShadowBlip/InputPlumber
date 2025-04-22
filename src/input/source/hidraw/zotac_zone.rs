use std::{error::Error, fmt::Debug};

use crate::{
    drivers::zotac_zone::driver::Driver,
    input::{
        capability::Capability,
        event::native::NativeEvent,
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// ZotacZone source device implementation
pub struct ZotacZone {
    _driver: Driver,
}

impl ZotacZone {
    /// Create a new source device with the given udev device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info)?;
        Ok(Self { _driver: driver })
    }
}

impl Debug for ZotacZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZotacZone").finish()
    }
}

impl SourceInputDevice for ZotacZone {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        Ok(vec![])
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![])
    }
}

impl SourceOutputDevice for ZotacZone {}
