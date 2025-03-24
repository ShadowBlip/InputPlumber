use std::{error::Error, fmt::Debug};

use crate::{
    drivers::msi_claw::{driver::Driver, hid_report::GamepadMode},
    input::{
        capability::Capability,
        event::native::NativeEvent,
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

pub struct MsiClaw {}

impl MsiClaw {
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info)?;
        log::debug!("Setting gamepad to XInput mode");
        driver.set_mode(GamepadMode::XInput)?;
        Ok(Self {})
    }
}

impl Debug for MsiClaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MsiClaw").finish()
    }
}
impl SourceInputDevice for MsiClaw {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        Ok(vec![])
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![])
    }
}

impl SourceOutputDevice for MsiClaw {}

