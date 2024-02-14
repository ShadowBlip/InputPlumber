use std::collections::HashMap;

use evdev::{AbsInfo, AbsoluteAxisCode, EventType, InputEvent, KeyCode};

use crate::input::capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger};

use super::native::{InputValue, NativeEvent};

#[derive(Debug, Clone)]
pub struct EvdevEvent {
    event: InputEvent,
    abs_info: Option<AbsInfo>,
}

impl EvdevEvent {
    /// Returns the normalized value of the event expressed as an [InputValue].
    pub fn get_value(&self) -> InputValue {
        let normal_value = self.get_normalized_value();
        let event_type = self.event.event_type();
        let code = self.event.code();

        // Select the appropiate value type based on the type of input. E.g. Axis
        // values should be in the form of a [InputValue::Vector2].
        match event_type {
            EventType::KEY => {
                KeyCode::new(code);
                if normal_value > 0.5 {
                    InputValue::Bool(true)
                } else {
                    InputValue::Bool(false)
                }
            }
            EventType::ABSOLUTE => match AbsoluteAxisCode(code) {
                AbsoluteAxisCode::ABS_X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                AbsoluteAxisCode::ABS_RX => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_RY => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                AbsoluteAxisCode::ABS_HAT0X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_HAT0Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                AbsoluteAxisCode::ABS_HAT1X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_HAT1Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                AbsoluteAxisCode::ABS_HAT2X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_HAT2Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                AbsoluteAxisCode::ABS_HAT3X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_HAT3Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                _ => InputValue::Float(normal_value),
            },
            _ => InputValue::Float(normal_value),
        }
    }

    /// Returns the normalized value of the event. This will be a value that
    /// ranges from -1.0 to 1.0 based on the minimum and maximum values.
    pub fn get_normalized_value(&self) -> f64 {
        let raw_value = self.event.value();

        // If this event has ABS info, normalize the value
        if let Some(info) = self.abs_info {
            let code = self.event.code();
            match AbsoluteAxisCode(code) {
                AbsoluteAxisCode::ABS_Z => normalize_unsigned_value(raw_value, info.maximum()),
                AbsoluteAxisCode::ABS_RZ => normalize_unsigned_value(raw_value, info.maximum()),
                _ => normalize_signed_value(raw_value, info.minimum(), info.maximum()),
            }
        } else {
            raw_value as f64
        }
    }

    /// Set Absolute Axis information on the event. This is typically used to
    /// normalize the value to something between -1.0 and 1.0 based on the
    /// minimum and maximum values.
    pub fn set_abs_info(&mut self, info: AbsInfo) {
        self.abs_info = Some(info)
    }

    /// Returns the event as a evdev [InputEvent]
    pub fn as_input_event(&self) -> InputEvent {
        self.event
    }

    /// Returns the capability that this event fulfills. This is used to translate
    /// evdev events into [NativeEvent]s.
    pub fn as_capability(&self) -> Capability {
        let event_type = self.event.event_type();
        let code = self.event.code();
        match event_type {
            EventType::SYNCHRONIZATION => Capability::Sync,
            EventType::KEY => match KeyCode::new(code) {
                KeyCode::BTN_SOUTH => Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                KeyCode::BTN_NORTH => Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
                KeyCode::BTN_WEST => Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
                KeyCode::BTN_EAST => Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
                KeyCode::BTN_TL => Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
                KeyCode::BTN_TR => Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                KeyCode::BTN_START => Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
                KeyCode::BTN_SELECT => Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
                KeyCode::BTN_MODE => Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                KeyCode::BTN_THUMBL => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick))
                }
                KeyCode::BTN_THUMBR => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick))
                }
                _ => Capability::NotImplemented,
            },
            EventType::ABSOLUTE => match AbsoluteAxisCode(code) {
                AbsoluteAxisCode::ABS_X => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
                }
                AbsoluteAxisCode::ABS_Y => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
                }
                AbsoluteAxisCode::ABS_RX => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
                }
                AbsoluteAxisCode::ABS_RY => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
                }
                AbsoluteAxisCode::ABS_Z => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
                }
                AbsoluteAxisCode::ABS_RZ => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
                }
                AbsoluteAxisCode::ABS_HAT0X => Capability::Gamepad(Gamepad::Axis(
                    GamepadAxis::Buttons(GamepadButton::DPadLeft, GamepadButton::DPadRight),
                )),
                AbsoluteAxisCode::ABS_HAT0Y => Capability::Gamepad(Gamepad::Axis(
                    GamepadAxis::Buttons(GamepadButton::DPadUp, GamepadButton::DPadDown),
                )),
                _ => Capability::NotImplemented,
            },
            EventType::RELATIVE => Capability::NotImplemented,
            EventType::MISC => Capability::NotImplemented,
            EventType::SWITCH => Capability::NotImplemented,
            EventType::LED => Capability::NotImplemented,
            EventType::SOUND => Capability::NotImplemented,
            EventType::REPEAT => Capability::NotImplemented,
            EventType::FORCEFEEDBACK => Capability::NotImplemented,
            EventType::POWER => Capability::NotImplemented,
            EventType::FORCEFEEDBACKSTATUS => Capability::NotImplemented,
            EventType::UINPUT => Capability::NotImplemented,
            _ => Capability::NotImplemented,
        }
    }
}

impl Default for EvdevEvent {
    fn default() -> Self {
        Self {
            event: InputEvent::new(0, 0, 0),
            abs_info: None,
        }
    }
}

impl From<InputEvent> for EvdevEvent {
    fn from(item: InputEvent) -> Self {
        EvdevEvent {
            event: item,
            abs_info: None,
        }
    }
}

impl EvdevEvent {
    /// Translate a [NativeEvent] into one or more [EvdevEvent].
    pub fn from_native_event(
        event: NativeEvent,
        axis_map: HashMap<AbsoluteAxisCode, AbsInfo>,
    ) -> Vec<Self> {
        // Native events can be translated into one or more evdev events
        let mut events = Vec::new();

        // Determine the event type to use based on the capability
        let event_type = event_type_from_capability(event.as_capability());

        // If an event type cannot be determined, return an empty event
        let Some(event_type) = event_type else {
            //log::debug!("Unable to determine evdev event type");
            return events;
        };

        // Determine the event code(s) based on capability. Axis events can
        // translate to multiple events.
        let codes = event_codes_from_capability(event.as_capability());

        // Process each event code
        for code in codes {
            // Get the axis information if this is an ABS event.
            let axis_info = if event_type == EventType::ABSOLUTE {
                axis_map.get(&AbsoluteAxisCode(code)).copied()
            } else {
                None
            };

            // Get the axis direction if this this an ABS event and we need to
            // translate binary input into axis input. (e.g. DPad buttons)
            let axis_direction = if event_type == EventType::ABSOLUTE {
                Some(axis_direction_from_capability(event.as_capability()))
            } else {
                None
            };

            // Get the input value from the event and convert it into an evdev
            // input event.
            let value = event.get_value();
            let event = input_event_from_value(event_type, code, axis_info, axis_direction, value);
            if event.is_none() {
                continue;
            }

            events.push(EvdevEvent::from(event.unwrap()));
        }

        events
    }
}

/// Returns the event type responsible for handling the given input capability.
fn event_type_from_capability(capability: Capability) -> Option<EventType> {
    match capability {
        Capability::Sync => Some(EventType::SYNCHRONIZATION),
        Capability::Gamepad(gamepad) => match gamepad {
            Gamepad::Button(button) => match button {
                GamepadButton::DPadUp => Some(EventType::ABSOLUTE),
                GamepadButton::DPadDown => Some(EventType::ABSOLUTE),
                GamepadButton::DPadLeft => Some(EventType::ABSOLUTE),
                GamepadButton::DPadRight => Some(EventType::ABSOLUTE),
                _ => Some(EventType::KEY),
            },
            Gamepad::Axis(_) => Some(EventType::ABSOLUTE),
            Gamepad::Trigger(_) => Some(EventType::ABSOLUTE),
            Gamepad::Accelerometer => None,
            Gamepad::Gyro => None,
        },
        _ => None,
    }
}

/// Returns the axis direction for the given input capability. This is primarily
/// used to translate direction binary button events into axis events.
fn axis_direction_from_capability(capability: Capability) -> AxisDirection {
    match capability {
        Capability::Gamepad(gamepad) => {
            if let Gamepad::Button(button) = gamepad {
                match button {
                    GamepadButton::DPadUp => AxisDirection::Negative,
                    GamepadButton::DPadDown => AxisDirection::Positive,
                    GamepadButton::DPadLeft => AxisDirection::Negative,
                    GamepadButton::DPadRight => AxisDirection::Positive,
                    _ => AxisDirection::None,
                }
            } else {
                AxisDirection::None
            }
        }
        _ => AxisDirection::None,
    }
}

/// Returns a list of event codes responsible for handling the given input capability.
/// This is typically used to translate a [NativeEvent] into an [EvdevEvent].
fn event_codes_from_capability(capability: Capability) -> Vec<u16> {
    match capability {
        Capability::None => vec![],
        Capability::NotImplemented => vec![],
        Capability::Sync => vec![0],
        Capability::Gamepad(gamepad) => match gamepad {
            Gamepad::Button(btn) => match btn {
                GamepadButton::South => vec![KeyCode::BTN_SOUTH.0],
                GamepadButton::East => vec![KeyCode::BTN_EAST.0],
                GamepadButton::North => vec![KeyCode::BTN_NORTH.0],
                GamepadButton::West => vec![KeyCode::BTN_WEST.0],
                GamepadButton::LeftBumper => vec![KeyCode::BTN_TL.0],
                GamepadButton::RightBumper => vec![KeyCode::BTN_TR.0],
                GamepadButton::Start => vec![KeyCode::BTN_START.0],
                GamepadButton::Select => vec![KeyCode::BTN_SELECT.0],
                GamepadButton::Guide => vec![KeyCode::BTN_MODE.0],
                GamepadButton::Base => vec![KeyCode::BTN_BASE.0],
                GamepadButton::LeftStick => vec![KeyCode::BTN_THUMBL.0],
                GamepadButton::RightStick => vec![KeyCode::BTN_THUMBR.0],
                GamepadButton::DPadUp => vec![AbsoluteAxisCode::ABS_HAT0Y.0],
                GamepadButton::DPadDown => vec![AbsoluteAxisCode::ABS_HAT0Y.0],
                GamepadButton::DPadLeft => vec![AbsoluteAxisCode::ABS_HAT0X.0],
                GamepadButton::DPadRight => vec![AbsoluteAxisCode::ABS_HAT0X.0],
                GamepadButton::LeftTrigger => vec![KeyCode::BTN_TL2.0],
                GamepadButton::LeftPaddle1 => vec![],
                GamepadButton::LeftPaddle2 => vec![],
                GamepadButton::LeftStickTouch => vec![],
                GamepadButton::LeftTouchpadTouch => vec![],
                GamepadButton::LeftTouchpadPress => vec![],
                GamepadButton::RightTrigger => vec![KeyCode::BTN_TR2.0],
                GamepadButton::RightPaddle1 => vec![],
                GamepadButton::RightPaddle2 => vec![],
                GamepadButton::RightStickTouch => vec![],
                GamepadButton::RightTouchpadTouch => vec![],
                GamepadButton::RightTouchpadPress => vec![],
            },
            Gamepad::Axis(axis) => match axis {
                GamepadAxis::LeftStick => {
                    vec![AbsoluteAxisCode::ABS_X.0, AbsoluteAxisCode::ABS_Y.0]
                }
                GamepadAxis::RightStick => {
                    vec![AbsoluteAxisCode::ABS_RX.0, AbsoluteAxisCode::ABS_RY.0]
                }
                GamepadAxis::Hat1 => {
                    vec![AbsoluteAxisCode::ABS_HAT0X.0, AbsoluteAxisCode::ABS_HAT0Y.0]
                }
                GamepadAxis::Hat2 => {
                    vec![AbsoluteAxisCode::ABS_HAT1X.0, AbsoluteAxisCode::ABS_HAT1Y.0]
                }
                GamepadAxis::Hat3 => {
                    vec![AbsoluteAxisCode::ABS_HAT2X.0, AbsoluteAxisCode::ABS_HAT2Y.0]
                }
                GamepadAxis::Buttons(_, _) => {
                    vec![AbsoluteAxisCode::ABS_HAT0X.0, AbsoluteAxisCode::ABS_HAT0Y.0]
                }
            },
            Gamepad::Trigger(trigg) => match trigg {
                GamepadTrigger::LeftTrigger => {
                    vec![AbsoluteAxisCode::ABS_Z.0]
                }
                GamepadTrigger::LeftTouchpadForce => vec![],
                GamepadTrigger::LeftStickForce => vec![],
                GamepadTrigger::RightTrigger => {
                    vec![AbsoluteAxisCode::ABS_RZ.0]
                }
                GamepadTrigger::RightTouchpadForce => vec![],
                GamepadTrigger::RightStickForce => vec![],
            },
            Gamepad::Accelerometer => vec![],
            Gamepad::Gyro => vec![],
        },
        Capability::Mouse(_) => vec![],
        Capability::Keyboard(_) => vec![],
    }
}

/// Returns a translated evdev input event from the given [InputValue].
fn input_event_from_value(
    event_type: EventType,
    code: u16,
    axis_info: Option<AbsInfo>,
    axis_direction: Option<AxisDirection>,
    input: InputValue,
) -> Option<InputEvent> {
    let value = match input {
        InputValue::Bool(value) => {
            // Convert the binary input value into an integar
            let value = if value { 1 } else { 0 };

            // If this value is for an axis, we need to convert this value into
            // the minimum and maximum values for that axis depending on the
            // axis direction. This is typically done for DPad button input that
            // needs to be translated to an ABS_HAT axis input.
            if axis_info.is_some() && axis_direction.is_some() {
                let info = axis_info.unwrap();
                let direction = axis_direction.unwrap();
                match direction {
                    AxisDirection::None => Some(value),
                    AxisDirection::Positive => Some(info.maximum() * value),
                    AxisDirection::Negative => Some(info.minimum() * value),
                }
            } else {
                Some(value)
            }
        }
        InputValue::Float(value) => Some(denormalize_unsigned_value(value, axis_info)),
        InputValue::Vector2 { x, y } => match AbsoluteAxisCode(code) {
            AbsoluteAxisCode::ABS_X => denormalize_optional_signed_value(x, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_Y => denormalize_optional_signed_value(y, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_RX => denormalize_optional_signed_value(x, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_RY => denormalize_optional_signed_value(y, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_HAT0X => denormalize_optional_signed_value(x, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_HAT0Y => denormalize_optional_signed_value(y, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_HAT1X => denormalize_optional_signed_value(x, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_HAT1Y => denormalize_optional_signed_value(y, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_HAT2X => denormalize_optional_signed_value(x, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_HAT2Y => denormalize_optional_signed_value(y, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_HAT3X => denormalize_optional_signed_value(x, axis_info.unwrap()),
            AbsoluteAxisCode::ABS_HAT3Y => denormalize_optional_signed_value(y, axis_info.unwrap()),
            _ => todo!(),
        },
        InputValue::Vector3 { x: _, y: _, z: _ } => todo!(),
    };
    if value.is_none() {
        return None;
    }

    Some(InputEvent::new(event_type.0, code, value.unwrap()))
}

/// Normalizes the given value from a real value with axis information into a range
/// between -1.0 - 1.0, indicating how far the axis has been pressed towards its
/// minimum and maximum values.
fn normalize_signed_value(raw_value: i32, min: i32, max: i32) -> f64 {
    let mid = (max + min) / 2;
    let event_value = raw_value - mid;

    let min = min as f64;
    let max = max as f64;
    let mid = mid as f64;
    let event_value = event_value as f64;

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
fn normalize_unsigned_value(raw_value: i32, max: i32) -> f64 {
    raw_value as f64 / max as f64
}

/// De-normalizes the given value from -1.0 - 1.0 into a real value based on the
/// minimum and maximum axis range from the given [AbsInfo].
fn denormalize_optional_signed_value(normal_value: Option<f64>, axis_info: AbsInfo) -> Option<i32> {
    if normal_value.is_none() {
        return None;
    }
    let normal_value = normal_value.unwrap();
    Some(denormalize_signed_value(normal_value, axis_info))
}

/// De-normalizes the given value from -1.0 - 1.0 into a real value based on the
/// minimum and maximum axis range from the given [AbsInfo].
fn denormalize_signed_value(normal_value: f64, axis_info: AbsInfo) -> i32 {
    let mid = (axis_info.maximum() + axis_info.minimum()) / 2;
    let normal_value_abs = normal_value.abs();
    if normal_value >= 0.0 {
        let maximum = (axis_info.maximum() - mid) as f64;
        let value = normal_value * maximum + (mid as f64);
        value as i32
    } else {
        let minimum = (axis_info.minimum() - mid) as f64;
        let value = normal_value_abs * minimum + (mid as f64);
        value as i32
    }
}

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_unsigned_value(normal_value: f64, axis_info: Option<AbsInfo>) -> i32 {
    // TODO: this better
    let Some(axis_info) = axis_info else {
        return normal_value as i32;
    };
    (normal_value * axis_info.maximum() as f64).round() as i32
}

/// The AxisDirection is used to determine if a button value should be mapped
/// towards the maximum axis value or the minimum axis value. For example,
/// when mapping a BTN_UP to an ABS_HAT0Y, the converted value should be
/// positive, towards that axis's maximum value, whereas BTN_DOWN should
/// be Negative, towards that axis's minimum value.
enum AxisDirection {
    None,
    Positive,
    Negative,
}
