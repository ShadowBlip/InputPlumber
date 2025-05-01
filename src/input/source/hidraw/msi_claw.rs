use std::{error::Error, fmt::Debug};

use crate::{
    drivers::msi_claw::{
        driver::Driver,
        hid_report::{GamepadMode, MkeysFunction},
    },
    input::{
        capability::Capability,
        event::native::NativeEvent,
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

pub struct MsiClaw {
    driver: Driver,
}

impl MsiClaw {
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info)?;
        log::debug!("Setting gamepad to XInput mode");
        driver.set_mode(GamepadMode::XInput, MkeysFunction::Macro)?;
        driver.set_mode(GamepadMode::XInput, MkeysFunction::Combination)?;
        driver.set_mode(GamepadMode::XInput, MkeysFunction::Macro)?;
        if let Err(e) = driver.get_mode() {
            log::error!("Failed to send get gamepad mode request: {e}");
        }
        Ok(Self { driver })
    }
}

impl Debug for MsiClaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MsiClaw").finish()
    }
}
impl SourceInputDevice for MsiClaw {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        if let Err(e) = self.driver.poll() {
            log::error!("Error polling: {e}");
        }
        Ok(vec![])
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![])
    }
}

impl SourceOutputDevice for MsiClaw {}
