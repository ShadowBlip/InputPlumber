use std::{error::Error, fmt::Debug};

use crate::{
    drivers::legos::{event, imu_driver::IMUDriver},
    input::{
        capability::{Capability, Gamepad},
        event::{native::NativeEvent, value::InputValue},
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Legion Go Controller source device implementation
pub struct LegionSImuController {
    driver: IMUDriver,
}

impl LegionSImuController {
    /// Create a new Legion controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = IMUDriver::new(device_info.devnode())?;
        Ok(Self { driver })
    }
}

impl SourceInputDevice for LegionSImuController {
    /// Poll the source device for input events
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

impl SourceOutputDevice for LegionSImuController {
    /// Write the given output event to the source device. Output events are
    /// events that flow from an application (like a game) to the physical
    /// input device, such as force feedback events.
    fn write_event(&mut self, _event: OutputEvent) -> Result<(), OutputError> {
        Ok(())
    }
}

impl Debug for LegionSImuController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegionSController").finish()
    }
}

/// Translate the given Legion Go events into native events
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given Legion Go event into a native event
fn translate_event(event: event::Event) -> NativeEvent {
    match event {
        event::Event::Inertia(motion) => match motion {
            event::InertialEvent::Accelerometer(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Accelerometer),
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
            event::InertialEvent::Gyro(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Gyro),
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
        },
        _ => NativeEvent::new(Capability::NotImplemented, InputValue::None),
    }
}

/// List of all capabilities that the Legion Go driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Accelerometer),
    Capability::Gamepad(Gamepad::Gyro),
];
