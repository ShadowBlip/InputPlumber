use crate::{
    drivers::legion_go_s::{
        event, ACCEL_RAW_TO_MPS2, GYRO_RAW_TO_RAD_S, PAD_FORCE_MAX, PAD_MOTION_MAX, STICK_X_MAX,
        STICK_X_MIN, STICK_Y_MAX, STICK_Y_MIN, TRIGG_MAX,
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Source, Touch,
            TouchButton, Touchpad,
        },
        event::value::{
            normalize_accel_value_i16, normalize_gyro_value_i16, normalize_signed_value,
            normalize_unsigned_value, InputValue,
        },
        source::EventCapability,
    },
};

pub mod imu;
pub mod touchpad;
pub mod xinput;

/// Normalize the value to something between -1.0 and 1.0 based on the
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: &event::AxisEvent) -> InputValue {
    match event {
        event::AxisEvent::Touchpad(value) => {
            let x = normalize_unsigned_value(value.x as f64, PAD_MOTION_MAX);
            let x = Some(x);

            let y = normalize_unsigned_value(value.y as f64, PAD_MOTION_MAX);
            let y = Some(y);

            InputValue::Touch {
                index: value.index,
                is_touching: value.is_touching,
                pressure: Some(value.is_touching as i32 as f64),
                x,
                y,
            }
        }
        event::AxisEvent::LStick(value) | event::AxisEvent::RStick(value) => {
            let x = normalize_signed_value(value.x as f64, STICK_X_MIN, STICK_X_MAX);
            let x = Some(x);

            let y = normalize_signed_value(value.y as f64, STICK_Y_MIN, STICK_Y_MAX);
            let y = Some(-y);

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the
/// Legion Go's maximum axis ranges.
fn normalize_trigger_value(event: &event::TriggerEvent) -> InputValue {
    match event {
        event::TriggerEvent::ATriggerL(value) | event::TriggerEvent::ATriggerR(value) => {
            InputValue::Float(normalize_unsigned_value(value.value as f64, TRIGG_MAX))
        }
        event::TriggerEvent::RpadForce(value) => {
            InputValue::Float(normalize_unsigned_value(value.value as f64, PAD_FORCE_MAX))
        }
    }
}

/// All Legion Go S sources share the driver events, so the capability mapping
/// is shared. It covers the union of all three devices' capabilities.
impl EventCapability for event::Event {
    fn to_value(&self) -> Option<InputValue> {
        match self {
            event::Event::Button(button) => Some(InputValue::Bool(button.pressed())),
            event::Event::Axis(axis) => Some(normalize_axis_value(axis)),
            event::Event::Trigger(trigger) => Some(normalize_trigger_value(trigger)),
            event::Event::Inertia(inertia) => match inertia {
                event::InertialEvent::Accelerometer(value) => Some(InputValue::Vector3 {
                    x: Some(normalize_accel_value_i16(value.x, ACCEL_RAW_TO_MPS2)),
                    y: Some(normalize_accel_value_i16(value.y, ACCEL_RAW_TO_MPS2)),
                    z: Some(normalize_accel_value_i16(value.z, ACCEL_RAW_TO_MPS2)),
                }),
                event::InertialEvent::Gyroscope(value) => Some(InputValue::Vector3 {
                    x: Some(normalize_gyro_value_i16(value.x, GYRO_RAW_TO_RAD_S)),
                    y: Some(normalize_gyro_value_i16(value.y, GYRO_RAW_TO_RAD_S)),
                    z: Some(normalize_gyro_value_i16(value.z, GYRO_RAW_TO_RAD_S)),
                }),
            },
        }
    }

    fn to_capability(&self) -> Capability {
        match self {
            event::Event::Button(button) => match button {
                event::ButtonEvent::A(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::South))
                }
                event::ButtonEvent::X(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::North))
                }
                event::ButtonEvent::B(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::East))
                }
                event::ButtonEvent::Y(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::West))
                }
                event::ButtonEvent::Menu(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Start))
                }
                event::ButtonEvent::View(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Select))
                }
                event::ButtonEvent::Legion(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide))
                }
                event::ButtonEvent::QuickAccess(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess))
                }
                event::ButtonEvent::DPadDown(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown))
                }
                event::ButtonEvent::DPadUp(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp))
                }
                event::ButtonEvent::DPadLeft(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft))
                }
                event::ButtonEvent::DPadRight(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight))
                }
                event::ButtonEvent::LB(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper))
                }
                event::ButtonEvent::DTriggerL(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger))
                }
                event::ButtonEvent::ThumbL(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick))
                }
                event::ButtonEvent::Y1(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1))
                }
                event::ButtonEvent::RB(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper))
                }
                event::ButtonEvent::DTriggerR(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger))
                }
                event::ButtonEvent::ThumbR(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick))
                }
                event::ButtonEvent::Y2(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1))
                }
                event::ButtonEvent::RPadPress(_) => {
                    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press)))
                }
            },
            event::Event::Axis(axis) => match axis {
                event::AxisEvent::Touchpad(_) => {
                    Capability::Touchpad(Touchpad::RightPad(Touch::Motion))
                }
                event::AxisEvent::LStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
                }
                event::AxisEvent::RStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
                }
            },
            event::Event::Inertia(inertia) => match inertia {
                event::InertialEvent::Accelerometer(_) => Capability::Accelerometer(Source::Center),
                event::InertialEvent::Gyroscope(_) => Capability::Gyroscope(Source::Center),
            },
            event::Event::Trigger(trigg) => match trigg {
                event::TriggerEvent::ATriggerL(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
                }
                event::TriggerEvent::ATriggerR(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
                }
                event::TriggerEvent::RpadForce(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce))
                }
            },
        }
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        match capability {
            Capability::Gamepad(Gamepad::Button(button)) => {
                let event = match button {
                    GamepadButton::South => event::ButtonEvent::A(Default::default()),
                    GamepadButton::North => event::ButtonEvent::X(Default::default()),
                    GamepadButton::East => event::ButtonEvent::B(Default::default()),
                    GamepadButton::West => event::ButtonEvent::Y(Default::default()),
                    GamepadButton::Start => event::ButtonEvent::Menu(Default::default()),
                    GamepadButton::Select => event::ButtonEvent::View(Default::default()),
                    GamepadButton::Guide => event::ButtonEvent::Legion(Default::default()),
                    GamepadButton::QuickAccess => {
                        event::ButtonEvent::QuickAccess(Default::default())
                    }
                    GamepadButton::DPadDown => event::ButtonEvent::DPadDown(Default::default()),
                    GamepadButton::DPadUp => event::ButtonEvent::DPadUp(Default::default()),
                    GamepadButton::DPadLeft => event::ButtonEvent::DPadLeft(Default::default()),
                    GamepadButton::DPadRight => event::ButtonEvent::DPadRight(Default::default()),
                    GamepadButton::LeftBumper => event::ButtonEvent::LB(Default::default()),
                    GamepadButton::LeftTrigger => event::ButtonEvent::DTriggerL(Default::default()),
                    GamepadButton::LeftStick => event::ButtonEvent::ThumbL(Default::default()),
                    GamepadButton::LeftPaddle1 => event::ButtonEvent::Y1(Default::default()),
                    GamepadButton::RightBumper => event::ButtonEvent::RB(Default::default()),
                    GamepadButton::RightTrigger => {
                        event::ButtonEvent::DTriggerR(Default::default())
                    }
                    GamepadButton::RightStick => event::ButtonEvent::ThumbR(Default::default()),
                    GamepadButton::RightPaddle1 => event::ButtonEvent::Y2(Default::default()),
                    _ => return None,
                };
                Some(event::Event::Button(event))
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
                GamepadTrigger::LeftTrigger => Some(event::Event::Trigger(
                    event::TriggerEvent::ATriggerL(Default::default()),
                )),
                GamepadTrigger::RightTrigger => Some(event::Event::Trigger(
                    event::TriggerEvent::ATriggerR(Default::default()),
                )),
                GamepadTrigger::RightTouchpadForce => Some(event::Event::Trigger(
                    event::TriggerEvent::RpadForce(Default::default()),
                )),
                _ => None,
            },
            Capability::Accelerometer(Source::Center) => Some(event::Event::Inertia(
                event::InertialEvent::Accelerometer(Default::default()),
            )),
            Capability::Gyroscope(Source::Center) => Some(event::Event::Inertia(
                event::InertialEvent::Gyroscope(Default::default()),
            )),
            Capability::Touchpad(Touchpad::RightPad(Touch::Motion)) => Some(event::Event::Axis(
                event::AxisEvent::Touchpad(Default::default()),
            )),
            Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))) => Some(
                event::Event::Button(event::ButtonEvent::RPadPress(Default::default())),
            ),
            _ => None,
        }
    }
}
