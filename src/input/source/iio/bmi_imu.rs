use core::time;
use std::{error::Error, thread};

use tokio::sync::broadcast;

use crate::{
    drivers::bmi_imu::{self, driver::Driver},
    iio::device::Device,
    input::{
        capability::{Capability, Gamepad},
        composite_device::Command,
        event::{native::NativeEvent, value::InputValue, Event},
    },
};

/// BMI IMU implementation of IIO interface
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
pub struct IMU {
    info: Device,
    composite_tx: broadcast::Sender<Command>,
    device_id: String,
}

impl IMU {
    pub fn new(info: Device, composite_tx: broadcast::Sender<Command>, device_id: String) -> Self {
        Self {
            info,
            composite_tx,
            device_id,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting BMI IMU driver");

        // Get the device id and name for the driver
        let id = self.info.id.clone().unwrap_or_default();
        let name = self.info.name.clone().unwrap_or_default();
        let device_id = self.device_id.clone();
        let tx = self.composite_tx.clone();

        // Spawn a blocking task with the given poll rate to poll the IMU for
        // data.
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let driver = Driver::new(id, name)?;
                loop {
                    let events = driver.poll()?;
                    let native_events = translate_events(events);
                    for event in native_events {
                        // Don't send un-implemented events
                        if matches!(event.as_capability(), Capability::NotImplemented) {
                            continue;
                        }
                        tx.send(Command::ProcessEvent(
                            device_id.clone(),
                            Event::Native(event),
                        ))?;
                    }

                    // Sleep between each poll iteration
                    let duration = time::Duration::from_micros(250);
                    thread::sleep(duration);
                }
            });

        // Wait for the task to finish
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }

        log::debug!("BMI IMU driver stopped");

        Ok(())
    }
}

/// Translate the given driver events into native events
fn translate_events(events: Vec<bmi_imu::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given driver event into a native event
fn translate_event(event: bmi_imu::event::Event) -> NativeEvent {
    match event {
        bmi_imu::event::Event::Accelerometer(data) => {
            let cap = Capability::Gamepad(Gamepad::Accelerometer);
            let value = InputValue::Vector3 {
                x: Some(data.x),
                y: Some(data.y),
                z: Some(data.z),
            };
            NativeEvent::new(cap, value)
        }
        bmi_imu::event::Event::Gyro(data) => {
            let cap = Capability::Gamepad(Gamepad::Gyro);
            let value = InputValue::Vector3 {
                x: Some(data.x),
                y: Some(data.y),
                z: Some(data.z),
            };
            NativeEvent::new(cap, value)
        }
    }
}
