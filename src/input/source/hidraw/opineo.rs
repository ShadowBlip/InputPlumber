use std::{error::Error, thread, time::Duration};

use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::{
    drivers::opineo::{
        driver::{self, Driver},
        event,
    },
    input::{
        capability::{Capability, Touch, Touchpad},
        composite_device::client::CompositeDeviceClient,
        event::{native::NativeEvent, value::InputValue, Event},
        source::command::SourceCommand,
    },
    udev::device::UdevDevice,
};

const POLL_RATE: Duration = Duration::from_micros(250);

#[derive(Debug, Clone, Copy)]
enum TouchpadSide {
    Unknown,
    Left,
    Right,
}

/// OrangePi NEO implementation of HIDRAW interface
#[derive(Debug)]
pub struct OrangePiNeoTouchpad {
    device: UdevDevice,
    composite_device: CompositeDeviceClient,
    rx: Option<mpsc::Receiver<SourceCommand>>,
}

impl OrangePiNeoTouchpad {
    pub fn new(
        device: UdevDevice,
        composite_device: CompositeDeviceClient,
        rx: mpsc::Receiver<SourceCommand>,
    ) -> Self {
        Self {
            device,
            composite_device,
            rx: Some(rx),
        }
    }

    pub async fn run(mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting OrangePi NEO Touchpad driver");
        let rx = self.rx.take().unwrap();
        let composite_device = self.composite_device.clone();

        // Query the udev module to determine if this is the left or right touchpad.
        let name = self.device.name();
        let touchpad_side = {
            if name == "OPI0001:00" {
                log::debug!("Detected left pad.");
                TouchpadSide::Left
            } else if name == "OPI0002:00" {
                log::debug!("Detected right pad.");
                TouchpadSide::Right
            } else {
                log::debug!("Unable to detect pad side.");
                TouchpadSide::Unknown
            }
        };

        // Spawn a blocking task to read the events
        let device = self.device.clone();

        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut output_handler = OpiOutput::new(rx);
                let mut driver = Driver::new(device.clone())?;
                loop {
                    let events = driver.poll()?;
                    let native_events = translate_events(events, touchpad_side);
                    for event in native_events {
                        // Don't send un-implemented events
                        if matches!(event.as_capability(), Capability::NotImplemented) {
                            continue;
                        }
                        let res = composite_device
                            .blocking_process_event(device.sysname().clone(), Event::Native(event));
                        if let Err(e) = res {
                            return Err(e.to_string().into());
                        }
                    }

                    // Receive commands/output events
                    if let Err(e) = output_handler.receive_commands() {
                        log::debug!("Error receiving commands: {:?}", e);
                        break;
                    }

                    // Polling interval is about 4ms so we can sleep a little
                    thread::sleep(POLL_RATE);
                }
                Ok(())
            });

        // Wait for the task to finish
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }

        log::debug!("OrangePi NEO Touchpad driver stopped");

        Ok(())
    }
}

/// Manages handling output events and source device commands
#[derive(Debug)]
struct OpiOutput {
    rx: mpsc::Receiver<SourceCommand>,
}

impl OpiOutput {
    pub fn new(rx: mpsc::Receiver<SourceCommand>) -> Self {
        Self { rx }
    }

    /// Read commands sent to this device from the channel until it is
    /// empty.
    fn receive_commands(&mut self) -> Result<(), Box<dyn Error>> {
        const MAX_COMMANDS: u8 = 64;
        let mut commands_processed = 0;
        loop {
            match self.rx.try_recv() {
                Ok(_) => (),
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
}

// Returns a value between 0.0 and 1.0 based on the given value with its
// maximum.
fn normalize_unsigned_value(raw_value: f64, max: f64) -> f64 {
    raw_value / max
}

/// Normalize the value to something between -1.0 and 1.0 based on the Deck's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: event::TouchAxisInput) -> InputValue {
    let max = driver::PAD_X_MAX;
    let x = normalize_unsigned_value(event.x as f64, max);

    let max = driver::PAD_Y_MAX;
    let y = normalize_unsigned_value(event.y as f64, max);

    // If this is an UP event, don't override the position of X/Y
    let (x, y) = if !event.is_touching {
        (None, None)
    } else {
        (Some(x), Some(y))
    };

    InputValue::Touch {
        index: event.index,
        is_touching: event.is_touching,
        pressure: Some(1.0),
        x,
        y,
    }
}

/// Translate the given OrangePi NEO events into native events
fn translate_events(events: Vec<event::Event>, touchpad_side: TouchpadSide) -> Vec<NativeEvent> {
    let mut translated = Vec::with_capacity(events.len());
    for event in events.into_iter() {
        translated.push(translate_event(event, touchpad_side));
    }
    translated
}

/// Translate the given OrangePi NEO event into a native event
fn translate_event(event: event::Event, touchpad_side: TouchpadSide) -> NativeEvent {
    match event {
        event::Event::TouchAxis(axis) => match touchpad_side {
            TouchpadSide::Unknown => {
                NativeEvent::new(Capability::NotImplemented, InputValue::Bool(false))
            }
            TouchpadSide::Left => NativeEvent::new(
                Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
                normalize_axis_value(axis),
            ),
            TouchpadSide::Right => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
                normalize_axis_value(axis),
            ),
        },
        //_ => NativeEvent::new(Capability::NotImplemented, InputValue::Bool(false)),
    }
}

/// List of all capabilities that the OrangePi NEO driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
];
