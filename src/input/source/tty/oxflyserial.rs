use crate::{
    drivers::oxp_tty::{
        driver::Driver,
        event::{self, AxisEvent},
        OxpDriverType, STICK_MAX, STICK_MIN, TRIGG_MAX,
    },
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};
use std::{error::Error, fmt::Debug};

/// [OneXFlySerial] source device implementation
pub struct OneXFlySerial {
    driver: Driver,
}

impl OneXFlySerial {
    /// Create a new [OneXFlySerial] source device with the given udev
    /// device information
    pub fn new(
        device: UdevDevice,
        driver_type: OxpDriverType,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device.devnode().as_str(), driver_type)?;
        Ok(Self { driver })
    }

    /// Translate the given [Driver] events into native events
    fn translate_events(&mut self, events: Vec<event::Event>) -> Vec<NativeEvent> {
        //events.into_iter().map(translate_event).collect()
        let mut new_events = Vec::new();

        for event in events {
            let new_event = self.translate_event(event);
            new_events.push(new_event);
        }

        new_events
    }

    /// Translate the given [Driver] event into a native event
    fn translate_event(&mut self, event: event::Event) -> NativeEvent {
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
                event::GamepadButtonEvent::DPadUp(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::DPadDown(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
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
                event::GamepadButtonEvent::RB(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::TriggerL(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::TriggerR(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::ThumbL(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::ThumbR(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::M1(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTop)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::M2(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTop)),
                    InputValue::Bool(value.pressed),
                ),
                _ => NativeEvent::new(Capability::None, InputValue::None),
            },
            event::Event::Axis(axis) => match axis.clone() {
                AxisEvent::LStick(_) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                    normalize_axis_value(axis),
                ),
                AxisEvent::RStick(_) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                    normalize_axis_value(axis),
                ),
                AxisEvent::TriggerL(_) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                    normalize_axis_value(axis),
                ),
                AxisEvent::TriggerR(_) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                    normalize_axis_value(axis),
                ),
            },
        }
    }
}

impl Debug for OneXFlySerial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OneXFlySerial").finish()
    }
}

impl SourceInputDevice for OneXFlySerial {
    fn poll(&mut self) -> Result<Vec<crate::input::event::native::NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = self.translate_events(events);
        Ok(native_events)
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for OneXFlySerial {}

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

/// Normalize the value to something between -1.0 and 1.0 based on the
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: AxisEvent) -> InputValue {
    match event {
        AxisEvent::LStick(value) => {
            let max = STICK_MAX;
            let min = STICK_MIN;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y);

            InputValue::Vector2 { x, y }
        }
        AxisEvent::RStick(value) => {
            let max = STICK_MAX;
            let min = STICK_MIN;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y);

            InputValue::Vector2 { x, y }
        }
        AxisEvent::TriggerL(value) => {
            let max = TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        AxisEvent::TriggerR(value) => {
            let max = TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// List of all capabilities that the [OneXFlySerial] implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTop)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTop)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
];
