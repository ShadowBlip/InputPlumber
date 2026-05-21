use std::{collections::HashSet, error::Error, fmt::Debug};

use crate::{
    config::ImuConfig,
    drivers::ssc::{self, driver::Driver},
    input::{
        capability::{Capability, Source},
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

pub struct SscImu {
    driver: Driver,
}

impl SscImu {
    pub fn new(
        _device_info: UdevDevice,
        imu_config: Option<ImuConfig>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let mount_matrix = imu_config.as_ref().and_then(|c| c.mount_matrix.clone());

        let driver = Driver::new(mount_matrix)?;

        Ok(Self { driver })
    }
}

impl SourceInputDevice for SscImu {
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

impl SourceOutputDevice for SscImu {}

impl Debug for SscImu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SscImu").finish()
    }
}

// NOTE: Mark this struct as thread-safe as it will only ever be called from
// a single thread.
unsafe impl Send for SscImu {}

/// Translate the given driver events into native events
fn translate_events(events: Vec<ssc::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given driver event into a native event
fn translate_event(event: ssc::event::Event) -> NativeEvent {
    match event {
        ssc::event::Event::Accelerometer(data) => {
            let cap = Capability::Accelerometer(Source::Center);
            let value = InputValue::Vector3 {
                x: Some(data.roll),
                y: Some(data.pitch),
                z: Some(data.yaw),
            };
            NativeEvent::new(cap, value)
        }
        ssc::event::Event::Gyro(data) => {
            let cap = Capability::Gyroscope(Source::Center);
            let value = InputValue::Vector3 {
                // Similar to bmi_imu, these values needs to be scaled a bunch
                // This seems close to correct, needs some more testing
                x: Some(data.roll.to_degrees() * 14.0),
                y: Some(data.pitch.to_degrees() * 14.0),
                z: Some(data.yaw.to_degrees() * 14.0),
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
