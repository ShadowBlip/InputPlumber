use crate::{
    drivers::oxp_tty::{event, STICK_MAX, STICK_MIN, TRIGG_MAX},
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        event::value::InputValue,
        source::EventCapability,
    },
};

pub mod onexfly;
pub mod x1;

/// Both sources share the OXP TTY driver events, so the capability mapping is
/// shared. It covers the union of both devices' capabilities.
impl EventCapability for event::Event {
    fn to_value(&self) -> Option<InputValue> {
        match self {
            event::Event::GamepadButton(event::GamepadButtonEvent::None) => None,
            event::Event::GamepadButton(button) => Some(InputValue::Bool(button.pressed())),
            event::Event::Axis(axis) => Some(normalize_axis_value(axis)),
        }
    }

    fn to_capability(&self) -> Capability {
        match self {
            event::Event::GamepadButton(button) => match button {
                event::GamepadButtonEvent::A(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::South))
                }
                event::GamepadButtonEvent::X(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::North))
                }
                event::GamepadButtonEvent::B(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::East))
                }
                event::GamepadButtonEvent::Y(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::West))
                }
                event::GamepadButtonEvent::Menu(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Start))
                }
                event::GamepadButtonEvent::View(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Select))
                }
                event::GamepadButtonEvent::DPadUp(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp))
                }
                event::GamepadButtonEvent::DPadDown(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown))
                }
                event::GamepadButtonEvent::DPadLeft(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft))
                }
                event::GamepadButtonEvent::DPadRight(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight))
                }
                event::GamepadButtonEvent::LB(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper))
                }
                event::GamepadButtonEvent::RB(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper))
                }
                event::GamepadButtonEvent::TriggerL(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger))
                }
                event::GamepadButtonEvent::TriggerR(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger))
                }
                event::GamepadButtonEvent::ThumbL(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick))
                }
                event::GamepadButtonEvent::ThumbR(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick))
                }
                event::GamepadButtonEvent::M1(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTop))
                }
                event::GamepadButtonEvent::M2(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTop))
                }
                event::GamepadButtonEvent::Keyboard(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Keyboard))
                }
                event::GamepadButtonEvent::None => Capability::None,
            },
            event::Event::Axis(axis) => match axis {
                event::AxisEvent::LStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
                }
                event::AxisEvent::RStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
                }
                event::AxisEvent::TriggerL(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
                }
                event::AxisEvent::TriggerR(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
                }
            },
        }
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        match capability {
            Capability::Gamepad(Gamepad::Button(button)) => {
                let event = match button {
                    GamepadButton::South => event::GamepadButtonEvent::A(Default::default()),
                    GamepadButton::North => event::GamepadButtonEvent::X(Default::default()),
                    GamepadButton::East => event::GamepadButtonEvent::B(Default::default()),
                    GamepadButton::West => event::GamepadButtonEvent::Y(Default::default()),
                    GamepadButton::Start => event::GamepadButtonEvent::Menu(Default::default()),
                    GamepadButton::Select => event::GamepadButtonEvent::View(Default::default()),
                    GamepadButton::DPadUp => event::GamepadButtonEvent::DPadUp(Default::default()),
                    GamepadButton::DPadDown => {
                        event::GamepadButtonEvent::DPadDown(Default::default())
                    }
                    GamepadButton::DPadLeft => {
                        event::GamepadButtonEvent::DPadLeft(Default::default())
                    }
                    GamepadButton::DPadRight => {
                        event::GamepadButtonEvent::DPadRight(Default::default())
                    }
                    GamepadButton::LeftBumper => event::GamepadButtonEvent::LB(Default::default()),
                    GamepadButton::RightBumper => event::GamepadButtonEvent::RB(Default::default()),
                    GamepadButton::LeftTrigger => {
                        event::GamepadButtonEvent::TriggerL(Default::default())
                    }
                    GamepadButton::RightTrigger => {
                        event::GamepadButtonEvent::TriggerR(Default::default())
                    }
                    GamepadButton::LeftStick => {
                        event::GamepadButtonEvent::ThumbL(Default::default())
                    }
                    GamepadButton::RightStick => {
                        event::GamepadButtonEvent::ThumbR(Default::default())
                    }
                    GamepadButton::LeftTop => event::GamepadButtonEvent::M1(Default::default()),
                    GamepadButton::RightTop => event::GamepadButtonEvent::M2(Default::default()),
                    GamepadButton::Keyboard => {
                        event::GamepadButtonEvent::Keyboard(Default::default())
                    }
                    _ => return None,
                };
                Some(event::Event::GamepadButton(event))
            }
            Capability::Gamepad(Gamepad::Axis(axis)) => match axis {
                GamepadAxis::LeftStick => Some(event::Event::Axis(event::AxisEvent::LStick(
                    Default::default(),
                ))),
                GamepadAxis::RightStick => Some(event::Event::Axis(event::AxisEvent::RStick(
                    Default::default(),
                ))),
                _ => None,
            },
            Capability::Gamepad(Gamepad::Trigger(trigger)) => match trigger {
                GamepadTrigger::LeftTrigger => Some(event::Event::Axis(
                    event::AxisEvent::TriggerL(Default::default()),
                )),
                GamepadTrigger::RightTrigger => Some(event::Event::Axis(
                    event::AxisEvent::TriggerR(Default::default()),
                )),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Normalize the given axis event to values between -1.0 and 1.0 based on
/// the minimum and maximum axis ranges.
fn normalize_axis_value(axis: &event::AxisEvent) -> InputValue {
    match axis {
        event::AxisEvent::LStick(value) => normalize_stick_value(value),
        event::AxisEvent::RStick(value) => normalize_stick_value(value),
        event::AxisEvent::TriggerL(value) => {
            InputValue::Float(normalize_unsigned_value(value.value as f64, TRIGG_MAX))
        }
        event::AxisEvent::TriggerR(value) => {
            InputValue::Float(normalize_unsigned_value(value.value as f64, TRIGG_MAX))
        }
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

/// Returns a value between 0.0 and 1.0 based on the given value with its
/// maximum.
fn normalize_unsigned_value(raw_value: f64, max: f64) -> f64 {
    raw_value / max
}

/// Normalize a stick to values between -1.0 and 1.0 based on the minimum and
/// maximum axis ranges.
fn normalize_stick_value(value: &event::JoyAxisInput) -> InputValue {
    let x = normalize_signed_value(value.x as f64, STICK_MIN, STICK_MAX);
    let y = normalize_signed_value(value.y as f64, STICK_MIN, STICK_MAX);

    InputValue::Vector2 {
        x: Some(x),
        y: Some(-y),
    }
}
