use std::{
    error::Error,
    fmt::Debug,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::time::{interval, Interval};

use crate::{
    drivers::msi_claw::{
        driver::Driver,
        hid_report::{GamepadMode, MkeysFunction},
    },
    input::{
        event::native::NativeEvent,
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

pub struct MsiClaw {
    driver: Arc<Mutex<Driver>>,
    interval: Interval,
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
        let interval = interval(Duration::from_millis(40));
        Ok(Self {
            driver: Arc::new(Mutex::new(driver)),
            interval,
        })
    }
}

impl Debug for MsiClaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MsiClaw").finish()
    }
}

impl SourceInputDevice for MsiClaw {
    async fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        self.interval.tick().await;
        if let Err(e) = self.driver.lock().unwrap().poll() {
            log::error!("Error polling: {e}");
        }
        Ok(vec![])
    }
}

impl SourceOutputDevice for MsiClaw {}
