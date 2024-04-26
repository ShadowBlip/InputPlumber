use core::time;
use std::{error::Error, f64::consts::PI, thread};

use tokio::sync::{
    broadcast,
    mpsc::{self, error::TryRecvError},
};

use crate::{
    config,
    drivers::bmi_imu::{self, driver::Driver, info::MountMatrix},
    iio::device::Device,
    input::{
        capability::{Capability, Gamepad},
        composite_device::Command,
        event::{native::NativeEvent, value::InputValue, Event},
        source::SourceCommand,
    },
};

/// BMI IMU implementation of IIO interface
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
pub struct IMU {
    info: Device,
    config: Option<config::IIO>,
    composite_tx: broadcast::Sender<Command>,
    rx: Option<mpsc::Receiver<SourceCommand>>,
    device_id: String,
}

impl IMU {
    pub fn new(
        info: Device,
        config: Option<config::IIO>,
        composite_tx: broadcast::Sender<Command>,
        rx: mpsc::Receiver<SourceCommand>,
        device_id: String,
    ) -> Self {
        Self {
            info,
            config,
            composite_tx,
            rx: Some(rx),
            device_id,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting BMI IMU driver");

        // Get the device id and name for the driver
        let id = self.info.id.clone().unwrap_or_default();
        let name = self.info.name.clone().unwrap_or_default();
        let device_id = self.device_id.clone();
        let tx = self.composite_tx.clone();
        let mut rx = self.rx.take().unwrap();

        // Override the mount matrix if one is defined in the config
        let mount_matrix = if let Some(config) = self.config.as_ref() {
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

        // Spawn a blocking task with the given poll rate to poll the IMU for
        // data.
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let driver = Driver::new(id, name, mount_matrix)?;
                loop {
                    receive_commands(&mut rx)?;
                    let events = driver.poll()?;
                    let native_events = translate_events(events);
                    for event in native_events {
                        log::trace!("Sending event to CompositeDevice: {:?}", event);
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
            log::error!("Error running BMI Driver: {:?}", e);
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
            // Translate gyro values into the expected units of degrees per sec
            let cap = Capability::Gamepad(Gamepad::Gyro);
            let value = InputValue::Vector3 {
                x: Some(data.x * (180.0 / PI)),
                y: Some(data.y * (180.0 / PI)),
                z: Some(data.z * (180.0 / PI)),
            };
            NativeEvent::new(cap, value)
        }
    }
}

/// Read commands sent to this device from the channel until it is
/// empty.
fn receive_commands(
    rx: &mut mpsc::Receiver<SourceCommand>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    const MAX_COMMANDS: u8 = 64;
    let mut commands_processed = 0;
    loop {
        match rx.try_recv() {
            Ok(cmd) => match cmd {
                SourceCommand::Stop => return Err("Device stopped".into()),
                _ => {}
            },
            Err(e) => match e {
                TryRecvError::Empty => return Ok(()),
                TryRecvError::Disconnected => {
                    log::debug!("Receive channel disconnected");
                    return Err("Receive channel disconnected".into());
                }
            },
        };

        commands_processed += 1;
        if commands_processed >= MAX_COMMANDS {
            return Ok(());
        }
    }
}
