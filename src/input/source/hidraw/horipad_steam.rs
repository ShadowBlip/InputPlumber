use std::{collections::HashSet, error::Error, fmt::Debug};

use crate::{
    drivers::horipad_steam::{
        driver::Driver, event, HORIPAD_ACCEL_RAW_TO_MPS2, HORIPAD_GYRO_RAW_TO_RAD_S, JOY_AXIS_MAX,
        JOY_AXIS_MIN, TRIGGER_AXIS_MAX,
    },
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Source},
        event::{
            native::NativeEvent,
            value::{
                normalize_accel_value_i16, normalize_gyro_value_i16, normalize_signed_value,
                normalize_unsigned_value, InputValue,
            },
        },
        source::{EventCapability, InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// HoripadSteam source device implementation
pub struct HoripadSteam {
    driver: Driver,
}

impl HoripadSteam {
    /// Create a new source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info)?;
        Ok(Self { driver })
    }
}

impl SourceOutputDevice for HoripadSteam {}

impl SourceInputDevice for HoripadSteam {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = translate_events(events);
        Ok(native_events)
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }

    fn update_event_filter(&mut self, events: HashSet<Capability>) -> Result<(), InputError> {
        let filtered = events
            .iter()
            .filter_map(EventCapability::from_capability)
            .collect();
        self.driver.update_filtered_events(filtered);
        Ok(())
    }
}

impl Debug for HoripadSteam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HoripadSteam").finish()
    }
}

/// Normalize the value to something between -1.0 and 1.0 based on the
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: &event::JoystickEvent) -> InputValue {
    let min = JOY_AXIS_MIN;
    let max = JOY_AXIS_MAX;
    match event {
        event::JoystickEvent::LStick(value) => {
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
        event::JoystickEvent::RStick(value) => {
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the
/// maximum axis range.
fn normalize_trigger_value(event: &event::TriggerEvent) -> InputValue {
    let max = TRIGGER_AXIS_MAX;
    match event {
        event::TriggerEvent::LTAnalog(value) => {
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::RTAnalog(value) => {
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// Translate the given events into native events
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    events.into_iter().filter_map(translate_event).collect()
}

/// Translate the given event into a native event
fn translate_event(event: event::Event) -> Option<NativeEvent> {
    let value = event.to_value()?;
    Some(NativeEvent::new(event.to_capability(), value))
}

impl EventCapability for event::Event {
    fn to_value(&self) -> Option<InputValue> {
        match self {
            event::Event::Button(button) => Some(InputValue::Bool(button.pressed())),
            event::Event::Joystick(axis) => Some(normalize_axis_value(axis)),
            event::Event::Trigger(trigger) => Some(normalize_trigger_value(trigger)),
            event::Event::Inertia(inertia) => match inertia {
                event::InertialEvent::Accelerometer(value) => Some(InputValue::Vector3 {
                    x: Some(normalize_accel_value_i16(
                        value.x,
                        HORIPAD_ACCEL_RAW_TO_MPS2,
                    )),
                    y: Some(normalize_accel_value_i16(
                        value.y,
                        HORIPAD_ACCEL_RAW_TO_MPS2,
                    )),
                    z: Some(normalize_accel_value_i16(
                        value.z,
                        HORIPAD_ACCEL_RAW_TO_MPS2,
                    )),
                }),
                event::InertialEvent::Gyroscope(value) => Some(InputValue::Vector3 {
                    x: Some(normalize_gyro_value_i16(value.x, HORIPAD_GYRO_RAW_TO_RAD_S)),
                    y: Some(normalize_gyro_value_i16(value.y, HORIPAD_GYRO_RAW_TO_RAD_S)),
                    z: Some(normalize_gyro_value_i16(value.z, HORIPAD_GYRO_RAW_TO_RAD_S)),
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
                event::ButtonEvent::Steam(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide))
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
                event::ButtonEvent::LSClick(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick))
                }
                event::ButtonEvent::RB(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper))
                }
                event::ButtonEvent::RSClick(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick))
                }
                event::ButtonEvent::Quick(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess))
                }
                event::ButtonEvent::M1(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2))
                }
                event::ButtonEvent::M2(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2))
                }
                event::ButtonEvent::L4(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1))
                }
                event::ButtonEvent::R4(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1))
                }
                event::ButtonEvent::LSTouch(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch))
                }
                event::ButtonEvent::RSTouch(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch))
                }
                event::ButtonEvent::LTDigital(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger))
                }
                event::ButtonEvent::RTDigital(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger))
                }
            },
            event::Event::Joystick(joystick) => match joystick {
                event::JoystickEvent::LStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
                }
                event::JoystickEvent::RStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
                }
            },
            event::Event::Trigger(trigger) => match trigger {
                event::TriggerEvent::LTAnalog(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
                }
                event::TriggerEvent::RTAnalog(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
                }
            },
            event::Event::Inertia(inertia) => match inertia {
                event::InertialEvent::Accelerometer(_) => Capability::Accelerometer(Source::Center),
                event::InertialEvent::Gyroscope(_) => Capability::Gyroscope(Source::Center),
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
                    GamepadButton::Guide => event::ButtonEvent::Steam(Default::default()),
                    GamepadButton::DPadDown => event::ButtonEvent::DPadDown(Default::default()),
                    GamepadButton::DPadUp => event::ButtonEvent::DPadUp(Default::default()),
                    GamepadButton::DPadLeft => event::ButtonEvent::DPadLeft(Default::default()),
                    GamepadButton::DPadRight => event::ButtonEvent::DPadRight(Default::default()),
                    GamepadButton::LeftBumper => event::ButtonEvent::LB(Default::default()),
                    GamepadButton::LeftStick => event::ButtonEvent::LSClick(Default::default()),
                    GamepadButton::RightBumper => event::ButtonEvent::RB(Default::default()),
                    GamepadButton::RightStick => event::ButtonEvent::RSClick(Default::default()),
                    GamepadButton::QuickAccess => event::ButtonEvent::Quick(Default::default()),
                    GamepadButton::LeftPaddle2 => event::ButtonEvent::M1(Default::default()),
                    GamepadButton::RightPaddle2 => event::ButtonEvent::M2(Default::default()),
                    GamepadButton::LeftPaddle1 => event::ButtonEvent::L4(Default::default()),
                    GamepadButton::RightPaddle1 => event::ButtonEvent::R4(Default::default()),
                    GamepadButton::LeftStickTouch => {
                        event::ButtonEvent::LSTouch(Default::default())
                    }
                    GamepadButton::RightStickTouch => {
                        event::ButtonEvent::RSTouch(Default::default())
                    }
                    GamepadButton::LeftTrigger => event::ButtonEvent::LTDigital(Default::default()),
                    GamepadButton::RightTrigger => {
                        event::ButtonEvent::RTDigital(Default::default())
                    }
                    _ => return None,
                };
                Some(event::Event::Button(event))
            }
            Capability::Gamepad(Gamepad::Axis(axis)) => match axis {
                GamepadAxis::LeftStick => Some(event::Event::Joystick(
                    event::JoystickEvent::LStick(Default::default()),
                )),
                GamepadAxis::RightStick => Some(event::Event::Joystick(
                    event::JoystickEvent::RStick(Default::default()),
                )),
                _ => None,
            },
            Capability::Gamepad(Gamepad::Trigger(trigger)) => match trigger {
                GamepadTrigger::LeftTrigger => Some(event::Event::Trigger(
                    event::TriggerEvent::LTAnalog(Default::default()),
                )),
                GamepadTrigger::RightTrigger => Some(event::Event::Trigger(
                    event::TriggerEvent::RTAnalog(Default::default()),
                )),
                _ => None,
            },
            Capability::Accelerometer(source) if *source == Source::Center => Some(
                event::Event::Inertia(event::InertialEvent::Accelerometer(Default::default())),
            ),
            Capability::Gyroscope(source) if *source == Source::Center => Some(
                event::Event::Inertia(event::InertialEvent::Gyroscope(Default::default())),
            ),
            _ => None,
        }
    }
}

/// List of all capabilities that the driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Accelerometer(Source::Center),
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
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
    Capability::Gyroscope(Source::Center),
];
