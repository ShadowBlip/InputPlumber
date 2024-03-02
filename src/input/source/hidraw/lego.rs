use std::{error::Error, thread, time};

use hidapi::DeviceInfo;
use tokio::sync::broadcast;

use crate::{
    drivers::lego::{self, driver::Driver, event::MouseAxisInput},
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Mouse, MouseButton,
        },
        composite_device::Command,
        event::{
            native::{InputValue, NativeEvent},
            Event,
        },
    },
};

/// Legion Go implementation of HIDRAW interface
#[derive(Debug)]
pub struct LegionController {
    info: DeviceInfo,
    composite_tx: broadcast::Sender<Command>,
}

impl LegionController {
    pub fn new(info: DeviceInfo, composite_tx: broadcast::Sender<Command>) -> Self {
        Self { info, composite_tx }
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting Legion Controller driver");
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

        // Wait for the task to finish
        if let Err(e) = task.await? {
            return Err(e.to_string().into());
        }

        log::debug!("Legion Controller driver stopped");

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
fn normalize_axis_value(event: lego::event::AxisEvent) -> InputValue {
    match event {
        lego::event::AxisEvent::Touchpad(value) => {
            let min = lego::driver::PAD_X_MIN;
            let max = lego::driver::PAD_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = lego::driver::PAD_Y_MAX; // uses inverted Y-axis
            let max = lego::driver::PAD_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        lego::event::AxisEvent::LStick(value) => {
            let min = lego::driver::STICK_X_MIN;
            let max = lego::driver::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = lego::driver::STICK_Y_MAX; // uses inverted Y-axis
            let max = lego::driver::STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        lego::event::AxisEvent::RStick(value) => {
            let min = lego::driver::STICK_X_MIN;
            let max = lego::driver::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = lego::driver::STICK_Y_MAX; // uses inverted Y-axis
            let max = lego::driver::STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        lego::event::AxisEvent::Mouse(value) => {
            let min = lego::driver::MOUSE_X_MIN;
            let max = lego::driver::MOUSE_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = lego::driver::MOUSE_Y_MIN;
            let max = lego::driver::MOUSE_Y_MAX;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the Deck's
/// maximum axis ranges.
fn normalize_trigger_value(event: lego::event::TriggerEvent) -> InputValue {
    match event {
        lego::event::TriggerEvent::ATriggerL(value) => {
            let max = lego::driver::TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        lego::event::TriggerEvent::ATriggerR(value) => {
            let max = lego::driver::TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        lego::event::TriggerEvent::MouseWheel(value) => {
            let max = lego::driver::MOUSE_WHEEL_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// Translate the given Steam Deck events into native events
fn translate_events(events: Vec<lego::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given Steam Deck event into a native event
fn translate_event(event: lego::event::Event) -> NativeEvent {
    match event {
        lego::event::Event::Button(button) => match button {
            lego::event::ButtonEvent::A(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::X(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::B(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::Y(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::Menu(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::View(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::Legion(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::QuickAccess(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Base)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::DPadDown(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::DPadUp(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::DPadLeft(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::DPadRight(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::LB(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::DTriggerL(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::ThumbL(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::Y1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::Y2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::RB(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::DTriggerR(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::ThumbR(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::Y3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::M3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::M2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle3)),
                InputValue::Bool(value.pressed),
            ),
            lego::event::ButtonEvent::MouseClick(value) => NativeEvent::new(
                Capability::Mouse(Mouse::Button(MouseButton::Middle)),
                InputValue::Bool(value.pressed),
            ),
        },
        /*
        lego::event::Event::Accelerometer(accel) => match accel {
            lego::event::AccelerometerEvent::Accelerometer(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
            lego::event::AccelerometerEvent::Attitude(value) => NativeEvent::new(
                Capability::NotImplemented,
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
        },
        */
        lego::event::Event::Axis(axis) => match axis.clone() {
            lego::event::AxisEvent::Touchpad(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_axis_value(axis))
            }
            lego::event::AxisEvent::LStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                normalize_axis_value(axis),
            ),
            lego::event::AxisEvent::RStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                normalize_axis_value(axis),
            ),
            lego::event::AxisEvent::Mouse(_) => {
                NativeEvent::new(Capability::Mouse(Mouse::Motion), normalize_axis_value(axis))
            }
        },
        lego::event::Event::Trigger(trigg) => match trigg.clone() {
            lego::event::TriggerEvent::ATriggerL(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                normalize_trigger_value(trigg),
            ),
            lego::event::TriggerEvent::ATriggerR(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                normalize_trigger_value(trigg),
            ),
            lego::event::TriggerEvent::MouseWheel(_) => {
                NativeEvent::new(Capability::NotImplemented, InputValue::Bool(false))
            }
        },
        _ => NativeEvent::new(Capability::NotImplemented, InputValue::Bool(false)),
    }
}
