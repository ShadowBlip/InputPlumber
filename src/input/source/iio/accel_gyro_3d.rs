use std::{collections::HashSet, error::Error, fmt::Debug};

use crate::{
    config,
    drivers::iio_imu::{driver::Driver, info::TriggerStrategy},
    input::{
        capability::Capability,
        event::native::NativeEvent,
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

use super::common::{
    corrections_from_config, mount_matrix_from_config, sample_rate_from_config, translate_events,
    CAPABILITIES,
};

pub struct AccelGyro3dImu {
    driver: Driver,
    accel_correction: f64,
    gyro_correction: f64,
}

impl AccelGyro3dImu {
    /// Create a new Accel Gyro 3D source device with the given udev
    /// device information
    pub fn new(
        device_info: UdevDevice,
        config: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mount_matrix = mount_matrix_from_config(config.as_ref());
        let sample_rate = sample_rate_from_config(config.as_ref());
        let (accel_correction, gyro_correction) = corrections_from_config(config.as_ref());

        let id = device_info.sysname();
        let driver = Driver::new(id, mount_matrix, sample_rate, TriggerStrategy::FindExisting)?;

        Ok(Self {
            driver,
            accel_correction,
            gyro_correction,
        })
    }
}

impl SourceInputDevice for AccelGyro3dImu {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        Ok(translate_events(
            events,
            self.accel_correction,
            self.gyro_correction,
        ))
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }

    fn update_event_filter(&mut self, events: HashSet<Capability>) -> Result<(), InputError> {
        self.driver.update_filtered_events(events);
        Ok(())
    }

    fn get_default_event_filter(&self) -> Result<HashSet<Capability>, InputError> {
        Ok(self.driver.get_default_event_filter()?)
    }
}

impl SourceOutputDevice for AccelGyro3dImu {}

impl Debug for AccelGyro3dImu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccelGyro3dImu").finish()
    }
}

// NOTE: Mark this struct as thread-safe as it will only ever be called from
// a single thread.
unsafe impl Send for AccelGyro3dImu {}
