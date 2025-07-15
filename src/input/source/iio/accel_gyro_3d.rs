use std::{error::Error, f64::consts::PI, fmt::Debug};

use crate::{
    config,
    drivers::iio_imu::{self, driver::Driver, info::MountMatrix},
    input::{
        capability::{Capability, Gamepad},
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

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

        let id = device_info.sysname();
        let name = device_info.name();
        let driver = Driver::new(id, name, mount_matrix)?;

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
            let cap = Capability::Gamepad(Gamepad::Accelerometer);
            let value = InputValue::Vector3 {
                x: Some(data.x * 10.0),
                y: Some(data.y * 10.0),
                z: Some(data.z * 10.0),
            };
            NativeEvent::new(cap, value)
        }
        iio_imu::event::Event::Gyro(data) => {
            // Translate gyro values into the expected units of degrees per sec
            // We apply a 500x scale so the motion feels like natural 1:1 motion.
            // Adjusting the scale is not possible on the accel_gyro_3d IMU.
            // From testing this is the highest scale we can apply before noise
            // is amplified to the point the gyro cannot calibrate.
            let cap = Capability::Gamepad(Gamepad::Gyro);
            let value = InputValue::Vector3 {
                x: Some(data.x * (180.0 / PI) * 500.0),
                y: Some(data.y * (180.0 / PI) * 500.0),
                z: Some(data.z * (180.0 / PI) * 500.0),
            };
            NativeEvent::new(cap, value)
        }
    }
}

/// List of all capabilities that the driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Accelerometer),
    Capability::Gamepad(Gamepad::Gyro),
];
