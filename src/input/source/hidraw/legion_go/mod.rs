pub mod go1;
pub mod go2;
pub mod touchpad;

use crate::{
    drivers::legion_go::{
        event::{
            self, AxisEvent, BinaryInput, ImuAxisInput, JoyAxisInput, MouseWheelInput,
            TouchAxisInput, TriggerInput,
        },
        GO2_ACCEL_RAW_TO_MPS2, GO2_GYRO_RAW_TO_RAD_S, PAD_FORCE_MAX, PAD_X_MAX, PAD_Y_MAX,
        STICK_X_MAX, STICK_X_MIN, STICK_Y_MAX, STICK_Y_MIN, TRIGG_MAX,
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Mouse, MouseButton,
            Source, Touch, TouchButton, Touchpad,
        },
        event::{
            value::InputValue,
            value::{
                normalize_accel_value_i16, normalize_gyro_value_i16, normalize_signed_value,
                normalize_unsigned_value,
            },
        },
        source::EventCapability,
    },
};

fn normalize_axis_value(event: &AxisEvent) -> InputValue {
    match event {
        AxisEvent::LStick(value) => {
            let min = STICK_X_MIN;
            let max = STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = STICK_Y_MAX; // uses inverted Y-axis
            let max = STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        AxisEvent::RStick(value) => {
            let min = STICK_X_MIN;
            let max = STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = STICK_Y_MAX; // uses inverted Y-axis
            let max = STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        AxisEvent::LeftAccel(value)
        | AxisEvent::RightAccel(value)
        | AxisEvent::MultiAccel(value) => InputValue::Vector3 {
            x: Some(normalize_accel_value_i16(
                value.pitch,
                GO2_ACCEL_RAW_TO_MPS2,
            )),
            y: Some(normalize_accel_value_i16(value.roll, GO2_ACCEL_RAW_TO_MPS2)),
            z: Some(normalize_accel_value_i16(value.yaw, GO2_ACCEL_RAW_TO_MPS2)),
        },
        AxisEvent::LeftGyro(value) | AxisEvent::RightGyro(value) | AxisEvent::MultiGyro(value) => {
            InputValue::Vector3 {
                x: Some(normalize_gyro_value_i16(value.pitch, GO2_GYRO_RAW_TO_RAD_S)),
                y: Some(normalize_gyro_value_i16(value.roll, GO2_GYRO_RAW_TO_RAD_S)),
                z: Some(normalize_gyro_value_i16(value.yaw, GO2_GYRO_RAW_TO_RAD_S)),
            }
        }
        AxisEvent::Touchpad(value) => {
            let max = PAD_X_MAX;
            let x = normalize_unsigned_value(value.x as f64, max);

            let max = PAD_Y_MAX;
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
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the
/// Legion Go's maximum axis ranges.
fn normalize_trigger_value(event: &event::TriggerEvent) -> InputValue {
    match event {
        event::TriggerEvent::ATriggerL(value) => {
            let max = TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::ATriggerR(value) => {
            let max = TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::MouseWheel(value) => InputValue::Vector2 {
            x: None,
            y: Some(value.value as f64),
        },
        event::TriggerEvent::RpadForce(value) => {
            let max = PAD_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

impl EventCapability for event::Event {
    fn to_capability(&self) -> Capability {
        match self {
            event::Event::GamepadButton(button) => {
                let button = match button {
                    event::GamepadButtonEvent::MouseClick(_) => {
                        return Capability::Mouse(Mouse::Button(MouseButton::Middle));
                    }
                    event::GamepadButtonEvent::A(_) => GamepadButton::South,
                    event::GamepadButtonEvent::X(_) => GamepadButton::North,
                    event::GamepadButtonEvent::B(_) => GamepadButton::East,
                    event::GamepadButtonEvent::Y(_) => GamepadButton::West,
                    event::GamepadButtonEvent::Menu(_) => GamepadButton::Start,
                    event::GamepadButtonEvent::View(_) => GamepadButton::Select,
                    event::GamepadButtonEvent::Legion(_) => GamepadButton::Guide,
                    event::GamepadButtonEvent::QuickAccess(_) => GamepadButton::QuickAccess,
                    event::GamepadButtonEvent::DPadDown(_) => GamepadButton::DPadDown,
                    event::GamepadButtonEvent::DPadUp(_) => GamepadButton::DPadUp,
                    event::GamepadButtonEvent::DPadLeft(_) => GamepadButton::DPadLeft,
                    event::GamepadButtonEvent::DPadRight(_) => GamepadButton::DPadRight,
                    event::GamepadButtonEvent::LB(_) => GamepadButton::LeftBumper,
                    event::GamepadButtonEvent::DTriggerL(_) => GamepadButton::LeftTrigger,
                    event::GamepadButtonEvent::ThumbL(_) => GamepadButton::LeftStick,
                    event::GamepadButtonEvent::Y1(_) => GamepadButton::LeftPaddle1,
                    event::GamepadButtonEvent::Y2(_) => GamepadButton::LeftPaddle2,
                    event::GamepadButtonEvent::Y3(_) => GamepadButton::RightPaddle1,
                    event::GamepadButtonEvent::RB(_) => GamepadButton::RightBumper,
                    event::GamepadButtonEvent::DTriggerR(_) => GamepadButton::RightTrigger,
                    event::GamepadButtonEvent::ThumbR(_) => GamepadButton::RightStick,
                    event::GamepadButtonEvent::M1(_) => GamepadButton::LeftStickTouch,
                    event::GamepadButtonEvent::M2(_) => GamepadButton::RightStickTouch,
                    event::GamepadButtonEvent::M3(_) => GamepadButton::RightPaddle2,
                    event::GamepadButtonEvent::ShowDesktop(_) => GamepadButton::Keyboard,
                    event::GamepadButtonEvent::AltTab(_) => GamepadButton::QuickAccess2,
                };
                Capability::Gamepad(Gamepad::Button(button))
            }
            event::Event::Axis(axis) => match axis {
                AxisEvent::LStick(_) => Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                AxisEvent::RStick(_) => Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                AxisEvent::Touchpad(_) => Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
                AxisEvent::LeftAccel(_) => Capability::Accelerometer(Source::Left),
                AxisEvent::LeftGyro(_) => Capability::Gyroscope(Source::Left),
                AxisEvent::RightAccel(_) => Capability::Accelerometer(Source::Right),
                AxisEvent::RightGyro(_) => Capability::Gyroscope(Source::Right),
                AxisEvent::MultiAccel(_) => Capability::Accelerometer(Source::Center),
                AxisEvent::MultiGyro(_) => Capability::Gyroscope(Source::Center),
            },
            event::Event::Trigger(trigger) => match trigger {
                event::TriggerEvent::ATriggerL(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
                }
                event::TriggerEvent::ATriggerR(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
                }
                event::TriggerEvent::MouseWheel(_) => Capability::Mouse(Mouse::Wheel),
                event::TriggerEvent::RpadForce(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce))
                }
            },
            event::Event::TouchButton(button) => match button {
                event::TouchButtonEvent::Left(_) => {
                    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press)))
                }
            },
        }
    }

    fn to_value(&self) -> Option<InputValue> {
        match self {
            event::Event::GamepadButton(button) => Some(InputValue::Bool(button.pressed())),
            event::Event::Axis(axis) => Some(normalize_axis_value(axis)),
            event::Event::Trigger(trigger) => Some(normalize_trigger_value(trigger)),
            event::Event::TouchButton(button) => Some(InputValue::Bool(button.pressed())),
        }
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        let button = |b: &GamepadButton| {
            let event = match b {
                GamepadButton::South => event::GamepadButtonEvent::A(BinaryInput::default()),
                GamepadButton::North => event::GamepadButtonEvent::X(BinaryInput::default()),
                GamepadButton::East => event::GamepadButtonEvent::B(BinaryInput::default()),
                GamepadButton::West => event::GamepadButtonEvent::Y(BinaryInput::default()),
                GamepadButton::Start => event::GamepadButtonEvent::Menu(BinaryInput::default()),
                GamepadButton::Select => event::GamepadButtonEvent::View(BinaryInput::default()),
                GamepadButton::Guide => event::GamepadButtonEvent::Legion(BinaryInput::default()),
                GamepadButton::QuickAccess => {
                    event::GamepadButtonEvent::QuickAccess(BinaryInput::default())
                }
                GamepadButton::DPadDown => {
                    event::GamepadButtonEvent::DPadDown(BinaryInput::default())
                }
                GamepadButton::DPadUp => event::GamepadButtonEvent::DPadUp(BinaryInput::default()),
                GamepadButton::DPadLeft => {
                    event::GamepadButtonEvent::DPadLeft(BinaryInput::default())
                }
                GamepadButton::DPadRight => {
                    event::GamepadButtonEvent::DPadRight(BinaryInput::default())
                }
                GamepadButton::LeftBumper => event::GamepadButtonEvent::LB(BinaryInput::default()),
                GamepadButton::LeftTrigger => {
                    event::GamepadButtonEvent::DTriggerL(BinaryInput::default())
                }
                GamepadButton::LeftStick => {
                    event::GamepadButtonEvent::ThumbL(BinaryInput::default())
                }
                GamepadButton::LeftPaddle1 => event::GamepadButtonEvent::Y1(BinaryInput::default()),
                GamepadButton::LeftPaddle2 => event::GamepadButtonEvent::Y2(BinaryInput::default()),
                GamepadButton::RightPaddle1 => {
                    event::GamepadButtonEvent::Y3(BinaryInput::default())
                }
                GamepadButton::RightBumper => event::GamepadButtonEvent::RB(BinaryInput::default()),
                GamepadButton::RightTrigger => {
                    event::GamepadButtonEvent::DTriggerR(BinaryInput::default())
                }
                GamepadButton::RightStick => {
                    event::GamepadButtonEvent::ThumbR(BinaryInput::default())
                }
                GamepadButton::LeftStickTouch => {
                    event::GamepadButtonEvent::M1(BinaryInput::default())
                }
                GamepadButton::RightStickTouch => {
                    event::GamepadButtonEvent::M2(BinaryInput::default())
                }
                GamepadButton::RightPaddle2 => {
                    event::GamepadButtonEvent::M3(BinaryInput::default())
                }
                GamepadButton::Keyboard => {
                    event::GamepadButtonEvent::ShowDesktop(BinaryInput::default())
                }
                GamepadButton::QuickAccess2 => {
                    event::GamepadButtonEvent::AltTab(BinaryInput::default())
                }
                _ => return None,
            };
            Some(event::Event::GamepadButton(event))
        };
        match capability {
            Capability::Gamepad(Gamepad::Button(b)) => button(b),
            Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)) => Some(event::Event::Axis(
                AxisEvent::LStick(JoyAxisInput::default()),
            )),
            Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)) => Some(
                event::Event::Axis(AxisEvent::RStick(JoyAxisInput::default())),
            ),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)) => Some(
                event::Event::Trigger(event::TriggerEvent::ATriggerL(TriggerInput::default())),
            ),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)) => Some(
                event::Event::Trigger(event::TriggerEvent::ATriggerR(TriggerInput::default())),
            ),
            Capability::Mouse(Mouse::Button(MouseButton::Middle)) => {
                Some(event::Event::GamepadButton(
                    event::GamepadButtonEvent::MouseClick(BinaryInput::default()),
                ))
            }
            Capability::Mouse(Mouse::Wheel) => Some(event::Event::Trigger(
                event::TriggerEvent::MouseWheel(MouseWheelInput::default()),
            )),
            Capability::Accelerometer(Source::Left) => Some(event::Event::Axis(
                AxisEvent::LeftAccel(ImuAxisInput::default()),
            )),
            Capability::Accelerometer(Source::Right) => Some(event::Event::Axis(
                AxisEvent::RightAccel(ImuAxisInput::default()),
            )),
            Capability::Accelerometer(Source::Center) => Some(event::Event::Axis(
                AxisEvent::MultiAccel(ImuAxisInput::default()),
            )),
            Capability::Gyroscope(Source::Left) => Some(event::Event::Axis(AxisEvent::LeftGyro(
                ImuAxisInput::default(),
            ))),
            Capability::Gyroscope(Source::Right) => Some(event::Event::Axis(AxisEvent::RightGyro(
                ImuAxisInput::default(),
            ))),
            Capability::Gyroscope(Source::Center) => Some(event::Event::Axis(
                AxisEvent::MultiGyro(ImuAxisInput::default()),
            )),
            Capability::Touchpad(Touchpad::RightPad(Touch::Motion)) => Some(event::Event::Axis(
                AxisEvent::Touchpad(TouchAxisInput::default()),
            )),
            Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))) => Some(
                event::Event::TouchButton(event::TouchButtonEvent::Left(BinaryInput::default())),
            ),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)) => Some(
                event::Event::Trigger(event::TriggerEvent::RpadForce(TriggerInput::default())),
            ),
            _ => None,
        }
    }
}
