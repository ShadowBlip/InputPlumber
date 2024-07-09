use std::{error::Error, thread, time::Duration};

use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::{
    drivers::fts3528::{
        self,
        driver::Driver,
        event::TouchAxisInput,
        hid_report::{TOUCHSCREEN_X_MAX, TOUCHSCREEN_Y_MAX},
    },
    input::{
        capability::{Capability, Touch},
        composite_device::client::CompositeDeviceClient,
        event::{native::NativeEvent, value::InputValue, Event},
        source::SourceCommand,
    },
    udev::device::UdevDevice,
};

/// How long to sleep before polling for events.
const POLL_RATE: Duration = Duration::from_millis(1);

#[derive(Debug)]
pub struct Fts3528TouchScreen {
    device: UdevDevice,
    composite_device: CompositeDeviceClient,
    rx: Option<mpsc::Receiver<SourceCommand>>,
    device_id: String,
}

impl Fts3528TouchScreen {
    pub fn new(
        device: UdevDevice,
        composite_device: CompositeDeviceClient,
        rx: mpsc::Receiver<SourceCommand>,
        device_id: String,
    ) -> Self {
        Self {
            device,
            composite_device,
            rx: Some(rx),
            device_id,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting FTS3528 Touchscreen driver");
        let mut rx = self.rx.take().unwrap();
        let composite_device = self.composite_device.clone();
        let path = self.device.devpath();
        let device_path = path.clone();
        let device_id = self.device_id.clone();

        // Spawn a blocking task to read the events
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut driver = Driver::new(device_path.clone())?;
                loop {
                    // Process events
                    let events = driver.poll()?;
                    let native_events = translate_events(events);
                    for event in native_events {
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

                    // Receive commands/output events
                    match rx.try_recv() {
                        Ok(cmd) => {
                            if let SourceCommand::Stop = cmd {
                                log::debug!("Received stop command");
                                break;
                            }
                        }
                        Err(e) => match e {
                            TryRecvError::Empty => (),
                            TryRecvError::Disconnected => {
                                log::debug!("Receive channel disconnected");
                                break;
                            }
                        },
                    }

                    // Polling interval is about 4ms so we can sleep a little
                    thread::sleep(POLL_RATE);
                }

                Ok(())
            });

        // Wait for the task to finish
        if let Err(e) = task.await? {
            log::error!("Error running driver: {e:?}");
            return Err(e.to_string().into());
        }

        log::debug!("FTS3528 Touchscreen driver stopped");

        Ok(())
    }
}

// Returns a value between 0.0 and 1.0 based on the given value with its
// maximum.
fn normalize_unsigned_value(raw_value: u16, max: u16) -> f64 {
    raw_value as f64 / max as f64
}

/// Normalizes the given input into an input value
fn normalize_axis_value(touch: TouchAxisInput) -> InputValue {
    // Normalize the x, y values if touching
    let (x, y) = match touch.is_touching {
        true => {
            // NOTE: X and Y are flipped due to panel rotation.
            let x = normalize_unsigned_value(touch.y, TOUCHSCREEN_Y_MAX);
            let y = 1.0 - normalize_unsigned_value(touch.x, TOUCHSCREEN_X_MAX);
            (Some(x), Some(y))
        }
        false => (None, None),
    };

    InputValue::Touch {
        index: touch.index,
        is_touching: touch.is_touching,
        pressure: Some(1.0),
        x,
        y,
    }
}

/// Translate the given touchscreen events into native events
fn translate_events(events: Vec<fts3528::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given touchscreen event into a native event
fn translate_event(event: fts3528::event::Event) -> NativeEvent {
    match event {
        fts3528::event::Event::Touch(touch) => NativeEvent::new(
            Capability::Touchscreen(Touch::Motion),
            normalize_axis_value(touch),
        ),
    }
}

/// List of all capabilities that the Touchscreen driver implements
pub const CAPABILITIES: &[Capability] = &[Capability::Touchscreen(Touch::Motion)];
