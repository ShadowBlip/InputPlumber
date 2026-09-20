use std::{collections::HashSet, error::Error, fmt::Debug};

use crate::{
    config,
    drivers::iio_imu::{
        self,
        driver::Driver,
        info::{MountMatrix, TriggerStrategy},
    },
    input::{
        capability::{Capability, Source},
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

pub struct AccelGyro3dImu {
    driver: Driver,
}

// Sensor fusion hub devices produce bogus scale factors.
const ACCEL_CORRECTION: f64 = 10.0;
const GYRO_CORRECTION: f64 = 100.0;

impl AccelGyro3dImu {
    /// Create a new Accel Gyro 3D source device with the given udev
    /// device information
    pub fn new(
        device_info: UdevDevice,
        config: Option<config::IIO>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // Override the mount matrix if one is defined in the config
        let mount_matrix = if let Some(config) = config.as_ref() {
            #[allow(deprecated)]
            if let Some(matrix_config) = config.mount_matrix.as_ref() {
                let matrix = MountMatrix {
                    x: (matrix_config.x[0], matrix_config.x[1], matrix_config.x[2]),
                    y: (matrix_config.y[0], matrix_config.y[1], matrix_config.y[2]),
                    z: (matrix_config.z[0], matrix_config.z[1], matrix_config.z[2]),
                };
                Some(matrix)
            } else {
                None
            }
        } else {
            None
        };

        let sample_rate = config.as_ref().and_then(|c| c.sample_rate);

        let id = device_info.sysname();
        let driver = Driver::new(id, mount_matrix, sample_rate, TriggerStrategy::FindExisting)?;

        Ok(Self { driver })
    }
}

impl SourceInputDevice for AccelGyro3dImu {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = translate_events(events);
        Ok(native_events)
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

/// Translate the given driver events into native events
fn translate_events(events: Vec<iio_imu::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given driver event into a native event
fn translate_event(event: iio_imu::event::Event) -> NativeEvent {
    match event {
        iio_imu::event::Event::Accelerometer(data) => {
            let cap = Capability::Accelerometer(Source::Center);
            let value = InputValue::Vector3 {
                x: Some(data.roll * data.scale * ACCEL_CORRECTION),
                y: Some(data.pitch * data.scale * ACCEL_CORRECTION),
                z: Some(data.yaw * data.scale * ACCEL_CORRECTION),
            };
            NativeEvent::new(cap, value)
        }
        iio_imu::event::Event::Gyro(data) => {
            let cap = Capability::Gyroscope(Source::Center);
            let value = InputValue::Vector3 {
                x: Some(data.roll * data.scale * GYRO_CORRECTION),
                y: Some(data.pitch * data.scale * GYRO_CORRECTION),
                z: Some(data.yaw * data.scale * GYRO_CORRECTION),
            };
            NativeEvent::new(cap, value)
        }
    }
}

/// List of all capabilities that the driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Accelerometer(Source::Center),
    Capability::Gyroscope(Source::Center),
];
