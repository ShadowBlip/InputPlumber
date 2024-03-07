//! The Deck implementation has been largly based off of the OpenSD project:
//! https://gitlab.com/open-sd/opensd/
use std::{error::Error, thread, time};

use hidapi::DeviceInfo;
use tokio::sync::broadcast;

use crate::{
    drivers::steam_deck::{self, driver::Driver, hid_report::LIZARD_SLEEP_SEC},
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        composite_device::Command,
        event::{
            native::{InputValue, NativeEvent},
            Event,
        },
    },
};

pub const VID: u16 = 0x28de;
pub const PID: u16 = 0x1205;

/// Steam Deck Controller implementation of HIDRaw interface
#[derive(Debug)]
pub struct DeckController {
    info: DeviceInfo,
    composite_tx: broadcast::Sender<Command>,
}

impl DeckController {
    pub fn new(info: DeviceInfo, composite_tx: broadcast::Sender<Command>) -> Self {
        Self { info, composite_tx }
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting Steam Deck Controller driver");
        let path = self.info.path().to_string_lossy().to_string();
        let tx = self.composite_tx.clone();

        // Spawn a blocking task to read the events
        let device_path = path.clone();
        let task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let mut driver = Driver::new(device_path.clone())?;
                loop {
                    let events = driver.poll()?;
                    let native_events = translate_events(events);
                    for event in native_events {
                        // Don't send un-implemented events
                        if matches!(event.as_capability(), Capability::NotImplemented) {
                            continue;
                        }
                        tx.send(Command::ProcessEvent(Event::Native(event)))?;
                    }

                    // Polling interval is about 4ms so we can sleep a little
                    let duration = time::Duration::from_micros(250);
                    thread::sleep(duration);
                }
            });

        // Spawn a blocking task to handle lizard mode
        let lizard_task =
            tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
                let driver = Driver::new(path.clone())?;
                loop {
                    // Keep the lizard asleep
                    driver.handle_lizard_mode()?;

                    // Polling interval is about 4ms so we can sleep a little
                    let duration = time::Duration::from_secs(LIZARD_SLEEP_SEC as u64);
                    thread::sleep(duration);
                }
            });

        // Wait for the task to finish
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }
        if let Err(e) = lizard_task.await? {
            return Err(e.to_string().into());
        }

        log::debug!("Steam Deck Controller driver stopped");

        Ok(())
    }
}

/// Returns a value between -1.0 and 1.0 based on the given value with its
/// minimum and maximum values.
fn normalize_signed_value(raw_value: f64, min: f64, max: f64) -> f64 {
    let mid = (max + min) / 2.0;
    let event_value = raw_value - mid;

    // Normalize the value
    if event_value >= 0.0 {
        let maximum = max - mid;
        event_value / maximum
    } else {
        let minimum = min - mid;
        let value = event_value / minimum;
        -value
    }
}

// Returns a value between 0.0 and 1.0 based on the given value with its
// maximum.
fn normalize_unsigned_value(raw_value: f64, max: f64) -> f64 {
    raw_value / max
}

/// Normalize the value to something between -1.0 and 1.0 based on the Deck's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: steam_deck::event::AxisEvent) -> InputValue {
    match event {
        steam_deck::event::AxisEvent::LPad(value) => {
            let min = steam_deck::hid_report::PAD_X_MIN;
            let max = steam_deck::hid_report::PAD_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::PAD_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::PAD_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        steam_deck::event::AxisEvent::RPad(value) => {
            let min = steam_deck::hid_report::PAD_X_MIN;
            let max = steam_deck::hid_report::PAD_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::PAD_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::PAD_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        steam_deck::event::AxisEvent::LStick(value) => {
            let min = steam_deck::hid_report::STICK_X_MIN;
            let max = steam_deck::hid_report::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::STICK_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        steam_deck::event::AxisEvent::RStick(value) => {
            let min = steam_deck::hid_report::STICK_X_MIN;
            let max = steam_deck::hid_report::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::STICK_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the Deck's
/// maximum axis ranges.
fn normalize_trigger_value(event: steam_deck::event::TriggerEvent) -> InputValue {
    match event {
        steam_deck::event::TriggerEvent::LTrigger(value) => {
            let max = steam_deck::hid_report::TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::RTrigger(value) => {
            let max = steam_deck::hid_report::TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::LPadForce(value) => {
            let max = steam_deck::hid_report::PAD_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::RPadForce(value) => {
            let max = steam_deck::hid_report::PAD_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::LStickForce(value) => {
            let max = steam_deck::hid_report::STICK_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::RStickForce(value) => {
            let max = steam_deck::hid_report::STICK_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// Translate the given Steam Deck events into native events
fn translate_events(events: Vec<steam_deck::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given Steam Deck event into a native event
fn translate_event(event: steam_deck::event::Event) -> NativeEvent {
    match event {
        steam_deck::event::Event::Button(button) => match button {
            steam_deck::event::ButtonEvent::A(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::X(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::B(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Y(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Menu(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Options(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Steam(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::QuickAccess(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadDown(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadUp(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadLeft(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadRight(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L5(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R5(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::RPadTouch(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTouchpadTouch)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LPadTouch(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTouchpadTouch)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::RPadPress(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTouchpadPress)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LPadPress(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTouchpadPress)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::RStickTouch(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LStickTouch(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
                InputValue::Bool(value.pressed),
            ),
        },
        steam_deck::event::Event::Accelerometer(accel) => match accel {
            steam_deck::event::AccelerometerEvent::Accelerometer(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
            steam_deck::event::AccelerometerEvent::Attitude(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
        },
        steam_deck::event::Event::Axis(axis) => match axis.clone() {
            steam_deck::event::AxisEvent::LPad(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_axis_value(axis))
            }
            steam_deck::event::AxisEvent::RPad(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_axis_value(axis))
            }
            steam_deck::event::AxisEvent::LStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                normalize_axis_value(axis),
            ),
            steam_deck::event::AxisEvent::RStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                normalize_axis_value(axis),
            ),
        },
        steam_deck::event::Event::Trigger(trigg) => match trigg.clone() {
            steam_deck::event::TriggerEvent::LTrigger(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                normalize_trigger_value(trigg),
            ),
            steam_deck::event::TriggerEvent::RTrigger(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                normalize_trigger_value(trigg),
            ),
            steam_deck::event::TriggerEvent::LPadForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
            steam_deck::event::TriggerEvent::RPadForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
            steam_deck::event::TriggerEvent::LStickForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
            steam_deck::event::TriggerEvent::RStickForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
        },
    }
}
