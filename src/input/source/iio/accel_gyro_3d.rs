use std::{error::Error, f64::consts::PI, thread};

use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::{
    config,
    drivers::iio_imu::{self, driver::Driver, info::MountMatrix},
    input::{
        capability::{Capability, Gamepad},
        composite_device::client::CompositeDeviceClient,
        event::{native::NativeEvent, value::InputValue, Event},
        source::SourceCommand,
    },
    udev::device::UdevDevice,
};

/// AccelGyro3D implementation of IIO interface used by the Legion Go and MSI Claw
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
pub struct IMU {
    device: UdevDevice,
    config: Option<config::IIO>,
    composite_device: CompositeDeviceClient,
    rx: Option<mpsc::Receiver<SourceCommand>>,
    device_id: String,
}

impl IMU {
    pub fn new(
        device: UdevDevice,
        config: Option<config::IIO>,
        composite_device: CompositeDeviceClient,
        rx: mpsc::Receiver<SourceCommand>,
        device_id: String,
    ) -> Self {
        Self {
            device,
            config,
            composite_device,
            rx: Some(rx),
            device_id,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting Accel Gyro IMU driver");

        // Get the device id and name for the driver
        let id = self.device.sysname();
        let name = self.device.name();
        let device_id = self.device_id.clone();
        let composite_device = self.composite_device.clone();
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
                        log::trace!("Sending IMU event to CompositeDevice: {:?}", event);
                        // Don't send un-implemented events
                        if matches!(event.as_capability(), Capability::NotImplemented) {
                            continue;
                        }
                        let res = composite_device
                            .blocking_process_event(device_id.clone(), Event::Native(event));
                        if let Err(e) = res {
                            return Err(e.to_string().into());
                        }
                    }
                    // Sleep between each poll iteration
                    thread::sleep(driver.sample_delay);
                }
            });

        // Wait for the task to finish
        if let Err(e) = task.await? {
            log::error!("Error running Accel Gyro IMU Driver: {:?}", e);
            return Err(e.to_string().into());
        }

        log::debug!("Accel Gyro IMU driver stopped");

        Ok(())
    }
}

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
                SourceCommand::EraseEffect(_, _) => (),
                SourceCommand::UpdateEffect(_, _) => (),
                SourceCommand::UploadEffect(_, _) => (),
                SourceCommand::WriteEvent(_) => (),
                SourceCommand::Stop => return Err("Device stopped".into()),
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
