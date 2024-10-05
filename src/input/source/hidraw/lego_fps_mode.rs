use std::{error::Error, fmt::Debug};

use crate::{
    drivers::lego::{
        driver_fps_mode::{self, Driver},
        event,
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Mouse, MouseButton,
            Touch, TouchButton, Touchpad,
        },
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Legion Go Controller source device implementation
pub struct LegionControllerFPS {
    driver: Driver,
}

impl LegionControllerFPS {
    /// Create a new Legion controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info.devnode())?;
        Ok(Self { driver })
    }
}

impl SourceInputDevice for LegionControllerFPS {
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

impl SourceOutputDevice for LegionControllerFPS {}

impl Debug for LegionControllerFPS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegionController").finish()
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
fn normalize_axis_value(event: event::AxisEvent) -> InputValue {
    match event {
        event::AxisEvent::Touchpad(value) => {
            let max = driver_fps_mode::PAD_X_MAX;
            let x = normalize_unsigned_value(value.x as f64, max);

            let max = driver_fps_mode::PAD_Y_MAX;
            let y = normalize_unsigned_value(value.y as f64, max);

            // If this is an UP event, don't override the position of X/Y
            let (x, y) = if !value.is_touching {
                (None, None)
            } else {
                (Some(x), Some(y))
            };

            InputValue::Touch {
                index: value.index,
                is_touching: value.is_touching,
                pressure: Some(1.0),
                x,
                y,
            }
        }
        event::AxisEvent::LStick(value) => {
            let min = driver_fps_mode::STICK_X_MIN;
            let max = driver_fps_mode::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = driver_fps_mode::STICK_Y_MAX; // uses inverted Y-axis
            let max = driver_fps_mode::STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        event::AxisEvent::RStick(value) => {
            let min = driver_fps_mode::STICK_X_MIN;
            let max = driver_fps_mode::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = driver_fps_mode::STICK_Y_MAX; // uses inverted Y-axis
            let max = driver_fps_mode::STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        event::AxisEvent::Mouse(value) => {
            let x = value.x as f64;
            let x = Some(x);
            let y = value.y as f64;
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the
/// Legion Go's maximum axis ranges.
fn normalize_trigger_value(event: event::TriggerEvent) -> InputValue {
    match event {
        event::TriggerEvent::ATriggerL(value) => {
            let max = driver_fps_mode::TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::ATriggerR(value) => {
            let max = driver_fps_mode::TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::MouseWheel(value) => {
            let max = driver_fps_mode::MOUSE_WHEEL_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// Translate the given Legion Go events into native events
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given Legion Go event into a native event
fn translate_event(event: event::Event) -> NativeEvent {
    match event {
        event::Event::GamepadButton(button) => match button {
            event::GamepadButtonEvent::A(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::X(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::B(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::Y(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::Menu(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::View(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::Legion(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::QuickAccess(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::DPadDown(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::DPadUp(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::DPadLeft(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::DPadRight(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::LB(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::DTriggerL(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::ThumbL(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::Y1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::Y2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::RB(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::DTriggerR(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::ThumbR(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::Y3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::M3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::M2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle3)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::MouseClick(value) => NativeEvent::new(
                Capability::Mouse(Mouse::Button(MouseButton::Middle)),
                InputValue::Bool(value.pressed),
            ),
        },
        event::Event::Axis(axis) => match axis.clone() {
            event::AxisEvent::Touchpad(_) => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
                normalize_axis_value(axis),
            ),
            event::AxisEvent::LStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                normalize_axis_value(axis),
            ),
            event::AxisEvent::RStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                normalize_axis_value(axis),
            ),
            event::AxisEvent::Mouse(_) => {
                NativeEvent::new(Capability::Mouse(Mouse::Motion), normalize_axis_value(axis))
            }
        },
        event::Event::Trigger(trigg) => match trigg.clone() {
            event::TriggerEvent::ATriggerL(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                normalize_trigger_value(trigg),
            ),
            event::TriggerEvent::ATriggerR(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                normalize_trigger_value(trigg),
            ),
            event::TriggerEvent::MouseWheel(_) => {
                NativeEvent::new(Capability::NotImplemented, InputValue::Bool(false))
            }
        },
        event::Event::MouseButton(button) => match button {
            event::MouseButtonEvent::Y3(value) => NativeEvent::new(
                Capability::Mouse(Mouse::Button(MouseButton::Extra)),
                InputValue::Bool(value.pressed),
            ),
            event::MouseButtonEvent::M1(value) => NativeEvent::new(
                Capability::Mouse(Mouse::Button(MouseButton::Left)),
                InputValue::Bool(value.pressed),
            ),
            event::MouseButtonEvent::M2(value) => NativeEvent::new(
                Capability::Mouse(Mouse::Button(MouseButton::Right)),
                InputValue::Bool(value.pressed),
            ),
            event::MouseButtonEvent::M3(value) => NativeEvent::new(
                Capability::Mouse(Mouse::Button(MouseButton::Side)),
                InputValue::Bool(value.pressed),
            ),
            event::MouseButtonEvent::Left(value) => NativeEvent::new(
                Capability::Mouse(Mouse::Button(MouseButton::Middle)),
                InputValue::Bool(value.pressed),
            ),
        },
        event::Event::TouchButton(button) => match button {
            event::TouchButtonEvent::Left(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
                InputValue::Bool(value.pressed),
            ),
        },

        _ => NativeEvent::new(Capability::NotImplemented, InputValue::Bool(false)),
    }
}

/// List of all capabilities that the Legion Go driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle3)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
    Capability::Mouse(Mouse::Button(MouseButton::Extra)),
    Capability::Mouse(Mouse::Button(MouseButton::Left)),
    Capability::Mouse(Mouse::Button(MouseButton::Middle)),
    Capability::Mouse(Mouse::Button(MouseButton::Right)),
    Capability::Mouse(Mouse::Button(MouseButton::Side)),
    Capability::Mouse(Mouse::Motion),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
];
