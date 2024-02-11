use std::collections::HashMap;

use evdev::{AbsInfo, AbsoluteAxisCode, EventType, InputEvent, KeyCode, KeyEvent};

use crate::input::capability::{Capability, Gamepad, GamepadAxis, GamepadButton};

use super::{native::NativeEvent, MappableEvent};

#[derive(Debug, Clone)]
pub struct EvdevEvent {
    event: InputEvent,
    abs_info: Option<AbsInfo>,
}

impl EvdevEvent {
    pub fn new() -> EvdevEvent {
        EvdevEvent {
            event: InputEvent::new(0, 0, 0),
            abs_info: None,
        }
    }

    /// Returns the normalized value of the event. This will be a value that
    /// ranges from -1.0 to 1.0 based on the minimum and maximum values.
    pub fn get_normalized_value(&self) -> f64 {
        let raw_value = self.event.value();

        // If this event has ABS info, normalize the value
        if let Some(info) = self.abs_info {
            let mid = (info.maximum() + info.minimum()) / 2;
            let event_value = (raw_value - mid) as f64;

            // Normalize the value
            if event_value >= 0.0 {
                let maximum = (info.maximum() - mid) as f64;
                event_value / maximum
            } else {
                let minimum = (info.minimum() - mid) as f64;
                let value = event_value / minimum;
                -value
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

    /// Returns the capability that this event fulfills.
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
                _ => Capability::None,
            },
            EventType::RELATIVE => Capability::None,
            EventType::ABSOLUTE => Capability::None,
            EventType::MISC => Capability::None,
            EventType::SWITCH => Capability::None,
            EventType::LED => Capability::None,
            EventType::SOUND => Capability::None,
            EventType::REPEAT => Capability::None,
            EventType::FORCEFEEDBACK => Capability::None,
            EventType::POWER => Capability::None,
            EventType::FORCEFEEDBACKSTATUS => Capability::None,
            EventType::UINPUT => Capability::None,
            _ => Capability::None,
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
    /// Convert a [NativeEvent] into an [EvdevEvent].
    pub fn from_native_event(
        event: NativeEvent,
        axis_map: HashMap<AbsoluteAxisCode, AbsInfo>,
    ) -> Self {
        let event_type = match event.as_capability() {
            Capability::Sync => EventType::SYNCHRONIZATION,
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(_) => EventType::KEY,
                Gamepad::Axis(_) => EventType::ABSOLUTE,
                Gamepad::Trigger(_) => todo!(),
                Gamepad::Accelerometer => todo!(),
                Gamepad::Gyro => todo!(),
            },
            _ => EventType::KEY,
        };

        let code = match event.as_capability() {
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(btn) => match btn {
                    GamepadButton::South => KeyCode::BTN_SOUTH.0,
                    GamepadButton::East => KeyCode::BTN_EAST.0,
                    GamepadButton::North => KeyCode::BTN_NORTH.0,
                    GamepadButton::West => KeyCode::BTN_WEST.0,
                    GamepadButton::LeftBumper => KeyCode::BTN_TL.0,
                    GamepadButton::RightBumper => KeyCode::BTN_TR.0,
                    GamepadButton::Start => KeyCode::BTN_START.0,
                    GamepadButton::Select => KeyCode::BTN_SELECT.0,
                    GamepadButton::Guide => KeyCode::BTN_MODE.0,
                    GamepadButton::Base => KeyCode::BTN_BASE.0,
                    GamepadButton::LeftStick => KeyCode::BTN_THUMBL.0,
                    GamepadButton::RightStick => KeyCode::BTN_THUMBR.0,
                    GamepadButton::DPadUp => KeyCode::BTN_TRIGGER_HAPPY1.0,
                    GamepadButton::DPadDown => KeyCode::BTN_TRIGGER_HAPPY2.0,
                    GamepadButton::DPadLeft => KeyCode::BTN_TRIGGER_HAPPY3.0,
                    GamepadButton::DPadRight => KeyCode::BTN_TRIGGER_HAPPY4.0,
                    GamepadButton::LeftTrigger => todo!(),
                    GamepadButton::LeftPaddle1 => todo!(),
                    GamepadButton::LeftPaddle2 => todo!(),
                    GamepadButton::LeftStickTouch => todo!(),
                    GamepadButton::LeftTouchpadTouch => todo!(),
                    GamepadButton::LeftTouchpadPress => todo!(),
                    GamepadButton::RightTrigger => todo!(),
                    GamepadButton::RightPaddle1 => todo!(),
                    GamepadButton::RightPaddle2 => todo!(),
                    GamepadButton::RightStickTouch => todo!(),
                    GamepadButton::RightTouchpadTouch => todo!(),
                    GamepadButton::RightTouchpadPress => todo!(),
                },
                Gamepad::Axis(axis) => match axis {
                    GamepadAxis::LeftStick => todo!(),
                    GamepadAxis::RightStick => todo!(),
                    GamepadAxis::Hat1 => todo!(),
                    GamepadAxis::Hat2 => todo!(),
                },
                Gamepad::Trigger(_) => todo!(),
                Gamepad::Accelerometer => todo!(),
                Gamepad::Gyro => todo!(),
            },
            _ => 0,
        };

        // Get the axis information if this is an ABS event.
        let abs_info = if event_type == EventType::ABSOLUTE {
            axis_map.get(&AbsoluteAxisCode(code)).copied()
        } else {
            None
        };

        // Denormalize the native value (e.g. 1.0) into the appropriate value
        // based on the ABS min/max range.
        let normalized_value = event.get_value();
        let denormalized_value = if let Some(info) = abs_info {
            let mid = (info.maximum() + info.minimum()) / 2;
            let normalized_value_abs = normalized_value.abs();
            if normalized_value >= 0.0 {
                let maximum = (info.maximum() - mid) as f64;
                let value = normalized_value * maximum + (mid as f64);
                value as i32
            } else {
                let minimum = (info.minimum() - mid) as f64;
                let value = normalized_value_abs * minimum + (mid as f64);
                value as i32
            }
        } else {
            normalized_value as i32
        };

        EvdevEvent {
            event: InputEvent::new(event_type.0, code, denormalized_value),
            abs_info,
        }
    }
}

impl MappableEvent for EvdevEvent {
    fn matches<T>(&self, event: T) -> bool
    where
        T: MappableEvent,
    {
        match event.kind() {
            super::Event::Evdev(event) => {
                self.event.code() == event.event.code()
                    && self.event.event_type() == event.event.event_type()
            }
            _ => false,
        }
    }

    fn set_value(&mut self, value: f64) {
        self.event = InputEvent::new(
            self.event.event_type().0,
            self.event.code(),
            value.round() as i32,
        );
    }

    fn get_value(&self) -> f64 {
        self.event.value() as f64
    }

    fn get_signature(&self) -> String {
        todo!()
    }

    fn kind(&self) -> super::Event {
        super::Event::Evdev(self.clone())
    }
}
