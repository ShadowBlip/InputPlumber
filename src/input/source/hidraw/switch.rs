use std::{error::Error, fmt::Debug};

use crate::{
    drivers::switch::driver::Driver,
    input::{
        capability::Capability,
        event::native::NativeEvent,
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

pub struct SwitchController {
    driver: Driver,
    device_info: UdevDevice,
}

impl SwitchController {
    /// Create a new Switch Controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info.devnode())?;

        Ok(Self {
            driver,
            device_info,
        })
    }
}

impl SourceInputDevice for SwitchController {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let _ = self.driver.poll()?;
        Ok(vec![])
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![])
    }
}

impl SourceOutputDevice for SwitchController {}

impl Debug for SwitchController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwitchController")
            .field("device_info", &self.device_info)
            .finish()
    }
}
