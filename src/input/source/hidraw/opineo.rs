use std::{error::Error, thread, time};

use hidapi::DeviceInfo;
use tokio::sync::mpsc;

use crate::{
    drivers::opineo::{
        driver::{self, Driver},
        event,
    },
    input::{
        capability::{Capability, Touch, Touchpad},
        composite_device::command::Command,
        event::{native::NativeEvent, value::InputValue, Event},
    },
    udev::get_device,
};

#[derive(Debug, Clone, Copy)]
enum TouchpadSide {
    Unknown,
    Left,
    Right,
}

/// OrangePi NEO implementation of HIDRAW interface
#[derive(Debug)]
pub struct OrangePiNeoTouchpad {
    info: DeviceInfo,
    composite_tx: mpsc::Sender<Command>,
    device_id: String,
}

impl OrangePiNeoTouchpad {
    pub fn new(info: DeviceInfo, composite_tx: mpsc::Sender<Command>, device_id: String) -> Self {
        Self {
            info,
            composite_tx,
            device_id,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting OrangePi NEO Touchpad driver");
        let path = self.info.path().to_string_lossy().to_string();
        let tx = self.composite_tx.clone();

        // Query the udev module to determine if this is the left or right touchpad.
        let dev_info = get_device(path.clone()).await?;

        log::debug!("udev info for device: {dev_info:?}");

        let touchpad_side = {
            if dev_info.path.contains("i2c-OPI0001:00") {
                log::debug!("Detected left pad.");
                TouchpadSide::Left
            } else if dev_info.path.contains("i2c-OPI0002:00") {
                log::debug!("Detected right pad.");
                TouchpadSide::Right
            } else {
                log::debug!("Unable to detect pad side.");
                TouchpadSide::Unknown
            }
        };

        // Spawn a blocking task to read the events
        let device_path = path.clone();
        let device_id = self.device_id.clone();
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut driver = Driver::new(device_path.clone())?;
                loop {
                    let events = driver.poll()?;
                    let native_events = translate_events(events, touchpad_side);
                    for event in native_events {
                        // Don't send un-implemented events
                        if matches!(event.as_capability(), Capability::NotImplemented) {
                            continue;
                        }
                        tx.blocking_send(Command::ProcessEvent(
                            device_id.clone(),
                            Event::Native(event),
                        ))?;
                    }

                    // Polling interval is about 4ms so we can sleep a little
                    let duration = time::Duration::from_micros(250);
                    thread::sleep(duration);
                }
            });

        // Wait for the task to finish
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }

        log::debug!("OrangePi NEO Touchpad driver stopped");

        Ok(())
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
