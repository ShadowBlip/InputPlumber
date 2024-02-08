use evdev::{EventType, InputEvent, KeyCode, KeyEvent};

use crate::input::capability::{Capability, Gamepad, GamepadButton};

use super::{native::NativeEvent, MappableEvent};

#[derive(Debug, Clone)]
pub struct EvdevEvent {
    event: InputEvent,
}

impl EvdevEvent {
    pub fn new() -> EvdevEvent {
        EvdevEvent {
            event: InputEvent::new(0, 0, 0),
        }
    }

    pub fn as_input_event(&self) -> InputEvent {
        self.event
    }

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
        }
    }
}

impl From<InputEvent> for EvdevEvent {
    fn from(item: InputEvent) -> Self {
        EvdevEvent { event: item }
    }
}

impl From<NativeEvent> for EvdevEvent {
    /// Convert a [NativeEvent] into an [EvdevEvent].
    fn from(item: NativeEvent) -> Self {
        let event_type = match item.as_capability() {
            Capability::Sync => EventType::SYNCHRONIZATION,
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(_) => EventType::KEY,
                Gamepad::Axis(_) => EventType::ABSOLUTE,
            },
            _ => EventType::KEY,
        };

        let code = match item.as_capability() {
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(btn) => match btn {
                    GamepadButton::South => KeyCode::BTN_SOUTH.0,
                    GamepadButton::East => KeyCode::BTN_EAST.0,
                    GamepadButton::North => KeyCode::BTN_NORTH.0,
                    GamepadButton::West => KeyCode::BTN_WEST.0,
                    GamepadButton::LeftBumper => todo!(),
                    GamepadButton::RightBumper => todo!(),
                    GamepadButton::Start => todo!(),
                    GamepadButton::Select => todo!(),
                    GamepadButton::Guide => todo!(),
                    GamepadButton::Base => todo!(),
                    GamepadButton::LeftStick => todo!(),
                    GamepadButton::RightStick => todo!(),
                    GamepadButton::DPadUp => todo!(),
                    GamepadButton::DPadDown => todo!(),
                    GamepadButton::DPadLeft => todo!(),
                    GamepadButton::DPadRight => todo!(),
                },
                Gamepad::Axis(axis) => todo!(),
            },
            _ => 0,
        };
        EvdevEvent {
            event: InputEvent::new(event_type.0, code, item.get_value() as i32),
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
