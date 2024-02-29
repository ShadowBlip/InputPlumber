use crate::input::capability::{Capability, Gamepad, GamepadAxis, GamepadButton};

use super::native::{InputValue, NativeEvent};

/// Actions represent all possible DBus event actions
#[derive(Debug, Clone)]
pub enum Action {
    None,
    Guide,
    Quick,
    Context,
    Option,
    Select,
    Accept,
    Back,
    ActOn,
    Left,
    Right,
    Up,
    Down,
    L1,
    L2,
    L3,
    R1,
    R2,
    R3,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::None => "none",
            Action::Guide => "ui_guide",
            Action::Quick => "ui_quick",
            Action::Context => "ui_context",
            Action::Option => "ui_option",
            Action::Select => "ui_select",
            Action::Accept => "ui_accept",
            Action::Back => "ui_back",
            Action::ActOn => "ui_action",
            Action::Left => "ui_left",
            Action::Right => "ui_right",
            Action::Up => "ui_up",
            Action::Down => "ui_down",
            Action::L1 => "ui_l1",
            Action::L2 => "ui_l2",
            Action::L3 => "ui_l3",
            Action::R1 => "ui_r1",
            Action::R2 => "ui_r2",
            Action::R3 => "ui_r3",
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

impl DBusEvent {
    /// Convert the [NativeEvent] into one or more [DBusEvent]
    pub fn from_native_event(item: NativeEvent) -> Vec<Self> {
        let mut events: Vec<Self> = Vec::new();
        let input_value = item.get_value();

        // Translate the event to dbus action(s)
        let actions = actions_from_capability(item.as_capability());

        // Create DBus events based on the type of input value
        for action in actions {
            let event = dbus_event_from_value(action, input_value.clone());
            if event.is_none() {
                continue;
            }
            events.push(event.unwrap());
        }

        events
    }
}

/// Returns an array of DBus event actions from the given event capability.
fn actions_from_capability(capability: Capability) -> Vec<Action> {
    match capability {
        Capability::None => vec![Action::None],
        Capability::NotImplemented => vec![Action::None],
        Capability::Sync => vec![Action::None],
        Capability::Gamepad(gamepad) => match gamepad {
            Gamepad::Button(btn) => match btn {
                GamepadButton::South => vec![Action::Accept],
                GamepadButton::East => vec![Action::Back],
                GamepadButton::North => vec![Action::Context],
                GamepadButton::West => vec![Action::ActOn],
                GamepadButton::Start => vec![Action::Option],
                GamepadButton::Select => vec![Action::Select],
                GamepadButton::Guide => vec![Action::Guide],
                GamepadButton::QuickAccess => vec![Action::Quick],
                GamepadButton::QuickAccess2 => vec![Action::None],
                GamepadButton::Keyboard => vec![Action::None],
                GamepadButton::Screenshot => vec![Action::None],
                GamepadButton::DPadUp => vec![Action::Up],
                GamepadButton::DPadDown => vec![Action::Down],
                GamepadButton::DPadLeft => vec![Action::Left],
                GamepadButton::DPadRight => vec![Action::Right],
                GamepadButton::LeftBumper => vec![Action::L1],
                GamepadButton::LeftTop => vec![Action::None],
                GamepadButton::LeftTrigger => vec![Action::L2],
                GamepadButton::LeftPaddle1 => vec![Action::None],
                GamepadButton::LeftPaddle2 => vec![Action::None],
                GamepadButton::LeftStick => vec![Action::L3],
                GamepadButton::LeftStickTouch => vec![Action::None],
                GamepadButton::LeftTouchpadTouch => vec![Action::None],
                GamepadButton::LeftTouchpadPress => vec![Action::None],
                GamepadButton::RightBumper => vec![Action::R1],
                GamepadButton::RightTop => vec![Action::None],
                GamepadButton::RightTrigger => vec![Action::R2],
                GamepadButton::RightPaddle1 => vec![Action::None],
                GamepadButton::RightPaddle2 => vec![Action::None],
                GamepadButton::RightStick => vec![Action::R3],
                GamepadButton::RightStickTouch => vec![Action::None],
                GamepadButton::RightTouchpadTouch => vec![Action::None],
                GamepadButton::RightTouchpadPress => vec![Action::None],
                GamepadButton::LeftPaddle3 => vec![Action::None],
                GamepadButton::RightPaddle3 => vec![Action::None],
            },
            Gamepad::Axis(axis) => match axis {
                GamepadAxis::LeftStick => {
                    vec![Action::Left, Action::Right, Action::Up, Action::Down]
                }
                GamepadAxis::Hat1 => {
                    vec![Action::Left, Action::Right, Action::Up, Action::Down]
                }
                GamepadAxis::Buttons(negative, positive) => {
                    let mut dpad_actions = vec![];
                    // Match negative axis buttons (up and left)
                    match negative {
                        GamepadButton::DPadUp => {
                            dpad_actions.push(Action::Up);
                        }
                        GamepadButton::DPadLeft => {
                            dpad_actions.push(Action::Left);
                        }
                        _ => (),
                    };
                    // Match positive axis buttons (down and right)
                    match positive {
                        GamepadButton::DPadDown => {
                            dpad_actions.push(Action::Down);
                        }
                        GamepadButton::DPadRight => {
                            dpad_actions.push(Action::Right);
                        }
                        _ => (),
                    }

                    dpad_actions
                }
                _ => vec![Action::None],
            },
            _ => vec![Action::None],
        },
        Capability::Mouse(_) => vec![Action::None],
        // TODO: Handle keyboard translation
        Capability::Keyboard(_) => vec![Action::None],
    }
}

/// Returns a DBus event from the given DBus event action and input value.
fn dbus_event_from_value(action: Action, input_value: InputValue) -> Option<DBusEvent> {
    let value = match input_value {
        InputValue::Bool(value) => {
            if value {
                Some(1.0)
            } else {
                Some(0.0)
            }
        }
        InputValue::Float(value) => Some(value),
        InputValue::Vector2 { x, y } => match action {
            // Left should be a negative value
            Action::Left => x.filter(|&x| x <= 0.0).map(|x| -x),
            // Right should be a positive value
            Action::Right => x.filter(|&x| x >= 0.0),
            // Up should be a negative value
            Action::Up => y.filter(|&y| y <= 0.0).map(|y| -y),
            // Down should be a positive value
            Action::Down => y.filter(|&y| y >= 0.0),
            _ => None,
        },
        InputValue::Vector3 { x, y, z } => None,
    };
    value?;

    Some(DBusEvent {
        action,
        value: value.unwrap(),
    })
}
