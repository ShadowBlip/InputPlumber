use std::{error::Error, fmt::Debug};

use crate::{
    drivers::lego::{
        event::{self, ImuAxisInput},
        xinput_driver::Driver,
        MOUSE_WHEEL_MAX, PAD_FORCE_MAX, PAD_X_MAX, PAD_Y_MAX, STICK_X_MAX, STICK_X_MIN,
        STICK_Y_MAX, STICK_Y_MIN, TRIGG_MAX,
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

enum ImuSourceDevice {
    //Screen,
    //LeftHandle,
    //RightHandle,
    CombinedHandles,
}
/// Legion Go Controller source device implementation
pub struct LegionXInputController {
    driver: Driver,
    last_left_accel: ImuAxisInput,
    last_right_accel: ImuAxisInput,
    last_left_gyro: ImuAxisInput,
    last_right_gyro: ImuAxisInput,
    imu_source_device: ImuSourceDevice,
}

impl LegionXInputController {
    /// Create a new Legion controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info.devnode())?;
        let last_left_accel = ImuAxisInput { x: 0, y: 0, z: 0 };
        let last_left_gyro = ImuAxisInput { x: 0, y: 0, z: 0 };
        let last_right_accel = ImuAxisInput { x: 0, y: 0, z: 0 };
        let last_right_gyro = ImuAxisInput { x: 0, y: 0, z: 0 };
        let imu_source_device = ImuSourceDevice::CombinedHandles;
        Ok(Self {
            driver,
            last_left_accel,
            last_left_gyro,
            last_right_accel,
            last_right_gyro,
            imu_source_device,
        })
    }

    /// Translate the given Legion Go events into native events
    fn translate_events(&mut self, events: Vec<event::Event>) -> Vec<NativeEvent> {
        //events.into_iter().map(translate_event).collect()
        let mut new_events = Vec::new();

        for event in events {
            let new_event = self.translate_event(event);
            new_events.push(new_event);
        }

        new_events
    }

    /// Translate the given Legion Go event into a native event
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
                event::GamepadButtonEvent::M1(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::MouseClick(value) => NativeEvent::new(
                    Capability::Mouse(Mouse::Button(MouseButton::Middle)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::ShowDesktop(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::AltTab(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
                    InputValue::Bool(value.pressed),
                ),
            },
            event::Event::Axis(axis) => match axis.clone() {
                event::AxisEvent::LStick(_) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                    normalize_axis_value(axis),
                ),
                event::AxisEvent::RStick(_) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                    normalize_axis_value(axis),
                ),
                event::AxisEvent::Touchpad(_) => NativeEvent::new(
                    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
                    normalize_axis_value(axis),
                ),

                _ => match self.imu_source_device {
                    //ImuSourceDevice::Screen => {
                    //    NativeEvent::new(Capability::NotImplemented, InputValue::Bool(false))
                    //}
                    //ImuSourceDevice::LeftHandle => match axis {
                    //    event::AxisEvent::LeftAccel(value) => NativeEvent::new(
                    //        Capability::Gamepad(Gamepad::Accelerometer),
                    //        InputValue::Vector3 {
                    //            x: Some(value.x as f64),
                    //            y: Some(value.y as f64),
                    //            z: Some(value.z as f64),
                    //        },
                    //    ),
                    //    event::AxisEvent::LeftGyro(value) => NativeEvent::new(
                    //        Capability::Gamepad(Gamepad::Gyro),
                    //        InputValue::Vector3 {
                    //            x: Some(value.x as f64),
                    //            y: Some(value.y as f64),
                    //            z: Some(value.z as f64),
                    //        },
                    //    ),
                    //    event::AxisEvent::Touchpad(_)
                    //    | event::AxisEvent::LStick(_)
                    //    | event::AxisEvent::RStick(_)
                    //    | event::AxisEvent::RightAccel(_)
                    //    | event::AxisEvent::RightGyro(_) => {
                    //        NativeEvent::new(Capability::NotImplemented, InputValue::Bool(false))
                    //    }
                    //},
                    //ImuSourceDevice::RightHandle => match axis {
                    //    event::AxisEvent::RightAccel(value) => NativeEvent::new(
                    //        Capability::Gamepad(Gamepad::Accelerometer),
                    //        InputValue::Vector3 {
                    //            x: Some(value.x as f64),
                    //            y: Some(value.y as f64),
                    //            z: Some(value.z as f64),
                    //        },
                    //    ),
                    //    event::AxisEvent::RightGyro(value) => NativeEvent::new(
                    //        Capability::Gamepad(Gamepad::Gyro),
                    //        InputValue::Vector3 {
                    //            x: Some(value.x as f64),
                    //            y: Some(value.y as f64),
                    //            z: Some(value.z as f64),
                    //        },
                    //    ),
                    //    event::AxisEvent::LeftAccel(_)
                    //    | event::AxisEvent::LeftGyro(_)
                    //    | event::AxisEvent::Touchpad(_)
                    //    | event::AxisEvent::LStick(_)
                    //    | event::AxisEvent::RStick(_) => {
                    //        NativeEvent::new(Capability::NotImplemented, InputValue::Bool(false))
                    //    }
                    //},
                    ImuSourceDevice::CombinedHandles => {
                        let (cap, value) = match axis {
                            event::AxisEvent::LeftAccel(value) => {
                                self.last_left_accel = value;
                                let new_input = InputValue::Vector3 {
                                    x: Some(
                                        (self.last_left_accel.x + self.last_right_accel.x) as f64,
                                    ),
                                    y: Some(
                                        (self.last_left_accel.y + self.last_right_accel.y) as f64,
                                    ),
                                    z: Some(
                                        (self.last_left_accel.z + self.last_right_accel.z) as f64,
                                    ),
                                };

                                (Capability::Gamepad(Gamepad::Accelerometer), new_input)
                            }
                            event::AxisEvent::RightAccel(value) => {
                                self.last_right_accel = value;
                                let new_input = InputValue::Vector3 {
                                    x: Some(
                                        (self.last_left_accel.x + self.last_right_accel.x) as f64,
                                    ),
                                    y: Some(
                                        (self.last_left_accel.y + self.last_right_accel.y) as f64,
                                    ),
                                    z: Some(
                                        (self.last_left_accel.z + self.last_right_accel.z) as f64,
                                    ),
                                };

                                (Capability::Gamepad(Gamepad::Accelerometer), new_input)
                            }
                            event::AxisEvent::LeftGyro(value) => {
                                self.last_left_gyro = value;
                                let new_input = InputValue::Vector3 {
                                    x: Some(
                                        ((self.last_left_gyro.x + self.last_right_gyro.x) / 2)
                                            as f64,
                                    ),
                                    y: Some(
                                        ((self.last_left_gyro.y + self.last_right_gyro.y) / 2)
                                            as f64,
                                    ),
                                    z: Some(
                                        (-(self.last_left_gyro.z + self.last_right_gyro.z) / 2)
                                            as f64,
                                    ),
                                };

                                (Capability::Gamepad(Gamepad::Gyro), new_input)
                            }
                            event::AxisEvent::RightGyro(value) => {
                                self.last_right_gyro = value;
                                let new_input = InputValue::Vector3 {
                                    x: Some(
                                        ((self.last_left_gyro.x + self.last_right_gyro.x) / 2)
                                            as f64,
                                    ),
                                    y: Some(
                                        ((self.last_left_gyro.y + self.last_right_gyro.y) / 2)
                                            as f64,
                                    ),
                                    z: Some(
                                        (-(self.last_left_gyro.z + self.last_right_gyro.z) - 2)
                                            as f64,
                                    ),
                                };

                                (Capability::Gamepad(Gamepad::Gyro), new_input)
                            }
                            event::AxisEvent::Touchpad(_)
                            | event::AxisEvent::LStick(_)
                            | event::AxisEvent::RStick(_) => {
                                (Capability::NotImplemented, InputValue::Bool(false))
                            }
                        };
                        NativeEvent::new(cap, value)
                    }
                },
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
                event::TriggerEvent::RpadForce(_) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)),
                    normalize_trigger_value(trigg),
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
}

impl SourceInputDevice for LegionXInputController {
    /// Poll the source device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = self.translate_events(events);

        Ok(native_events)
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for LegionXInputController {}

impl Debug for LegionXInputController {
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

/// Normalize the value to something between -1.0 and 1.0 based on the
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: event::AxisEvent) -> InputValue {
    match event {
        event::AxisEvent::LStick(value) => {
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
        event::AxisEvent::RStick(value) => {
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
        event::AxisEvent::LeftAccel(value) => InputValue::Vector3 {
            x: Some(value.x as f64),
            y: Some(value.y as f64),
            z: Some(value.z as f64),
        },
        event::AxisEvent::LeftGyro(value) => InputValue::Vector3 {
            x: Some(value.x as f64),
            y: Some(value.y as f64),
            z: Some(value.z as f64),
        },
        event::AxisEvent::RightAccel(value) => InputValue::Vector3 {
            x: Some(value.x as f64),
            y: Some(value.y as f64),
            z: Some(value.z as f64),
        },
        event::AxisEvent::RightGyro(value) => InputValue::Vector3 {
            x: Some(value.x as f64),
            y: Some(value.y as f64),
            z: Some(value.z as f64),
        },
        event::AxisEvent::Touchpad(value) => {
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
fn normalize_trigger_value(event: event::TriggerEvent) -> InputValue {
    match event {
        event::TriggerEvent::ATriggerL(value) => {
            let max = TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::ATriggerR(value) => {
            let max = TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::MouseWheel(value) => {
            let max = MOUSE_WHEEL_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::RpadForce(value) => {
            let max = PAD_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
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
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle3)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
];
