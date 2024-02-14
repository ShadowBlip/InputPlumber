use crate::input::capability::{Capability, Gamepad, GamepadButton};

use super::native::NativeEvent;

/// Actions represent all possible DBus event actions
#[derive(Debug, Clone)]
pub enum Action {
    None,
    Guide,
    Quick,
    Context,
    Option,
    Accept,
    Back,
    Left,
    Right,
    Up,
    Down,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::None => "none",
            Action::Guide => "ui_guide",
            Action::Quick => "ui_quick",
            Action::Context => "ui_context",
            Action::Option => "ui_option",
            Action::Accept => "ui_accept",
            Action::Back => "ui_back",
            Action::Left => "ui_left",
            Action::Right => "ui_right",
            Action::Up => "ui_up",
            Action::Down => "ui_down",
        }
    }

    pub fn as_string(&self) -> String {
        self.as_str().to_string()
    }
}

/// A [DBusEvent] can be sent as a DBus signal
#[derive(Debug, Clone)]
pub struct DBusEvent {
    pub action: Action,
    pub value: f64,
}

impl DBusEvent {
    pub fn new(action: Action, value: f64) -> DBusEvent {
        DBusEvent { action, value }
    }
}

impl Default for DBusEvent {
    fn default() -> Self {
        Self::new(Action::None, 0.0)
    }
}

impl From<NativeEvent> for DBusEvent {
    /// Convert the [NativeEvent] into a [DBusEvent]
    fn from(item: NativeEvent) -> Self {
        let input_value = item.get_value();
        let value = match input_value {
            super::native::InputValue::Bool(value) => {
                if value {
                    1.0
                } else {
                    0.0
                }
            }
            super::native::InputValue::Float(value) => value,
            super::native::InputValue::Vector2 { x, y } => 0.0,
            super::native::InputValue::Vector3 { x, y, z } => 0.0,
        };

        // Translate the event to an action
        let action = match item.as_capability() {
            Capability::None => Action::None,
            Capability::NotImplemented => Action::None,
            Capability::Sync => Action::None,
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(btn) => match btn {
                    GamepadButton::South => Action::Accept,
                    GamepadButton::East => Action::Back,
                    GamepadButton::North => Action::Context,
                    GamepadButton::West => Action::None,
                    GamepadButton::Start => Action::Option,
                    GamepadButton::Select => Action::None,
                    GamepadButton::Guide => Action::Guide,
                    GamepadButton::Base => Action::Quick,
                    GamepadButton::DPadUp => Action::Up,
                    GamepadButton::DPadDown => Action::Down,
                    GamepadButton::DPadLeft => Action::Left,
                    GamepadButton::DPadRight => Action::Right,
                    GamepadButton::LeftBumper => Action::None,
                    GamepadButton::LeftTrigger => Action::None,
                    GamepadButton::LeftPaddle1 => Action::None,
                    GamepadButton::LeftPaddle2 => Action::None,
                    GamepadButton::LeftStick => Action::None,
                    GamepadButton::LeftStickTouch => Action::None,
                    GamepadButton::LeftTouchpadTouch => Action::None,
                    GamepadButton::LeftTouchpadPress => Action::None,
                    GamepadButton::RightBumper => Action::None,
                    GamepadButton::RightTrigger => Action::None,
                    GamepadButton::RightPaddle1 => Action::None,
                    GamepadButton::RightPaddle2 => Action::None,
                    GamepadButton::RightStick => Action::None,
                    GamepadButton::RightStickTouch => Action::None,
                    GamepadButton::RightTouchpadTouch => Action::None,
                    GamepadButton::RightTouchpadPress => Action::None,
                },
                Gamepad::Axis(_) => Action::None,
                _ => Action::None,
            },
            Capability::Mouse(_) => Action::None,
            Capability::Keyboard(_) => Action::None,
        };

        Self { action, value }
    }
}
