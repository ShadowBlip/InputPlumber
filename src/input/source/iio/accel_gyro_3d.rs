use std::{collections::HashSet, error::Error, fmt::Debug};

use crate::{
    config,
    drivers::iio_imu::{self, driver::Driver, info::MountMatrix},
    input::{
        capability::{Capability, Source},
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

// Scale from IIO SI units to Steam Deck UHID raw LSB.
// IIO channels report m/s² for accel and rad/s for gyro after applying scale:
//   https://www.kernel.org/doc/Documentation/ABI/testing/sysfs-bus-iio
// UHID LSB constants from src/drivers/steam_deck/driver.rs.
const ACCEL_SCALE_FACTOR: f64 = 1632.6530612244898; // 1 / 0.0006125 (m/s² → UHID LSB)
const GYRO_SCALE_FACTOR: f64 = 916.7324722093172; // (180/π) / 0.0625 (rad/s → °/s → UHID LSB)

pub struct AccelGyro3dImu {
    driver: Driver,
}

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
        let name = device_info.name();
        let driver = Driver::new(id, name, mount_matrix, sample_rate)?;

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
        let filtered_events = self.driver.get_default_event_filter();
        let filtered_events = match filtered_events {
            Ok(events) => events,
            Err(e) => {
                return Err(format!("Failed to get default event filter: {:?}", e).into());
            }
        };
        Ok(filtered_events)
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
                x: Some(data.roll * ACCEL_SCALE_FACTOR),
                y: Some(data.pitch * ACCEL_SCALE_FACTOR),
                z: Some(data.yaw * ACCEL_SCALE_FACTOR),
            };
            NativeEvent::new(cap, value)
        }
        iio_imu::event::Event::Gyro(data) => {
            let cap = Capability::Gyroscope(Source::Center);
            let value = InputValue::Vector3 {
                x: Some(data.roll * GYRO_SCALE_FACTOR),
                y: Some(data.pitch * GYRO_SCALE_FACTOR),
                z: Some(data.yaw * GYRO_SCALE_FACTOR),
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
