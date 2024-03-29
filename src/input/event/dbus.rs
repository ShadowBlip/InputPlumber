use crate::input::capability::{Capability, Gamepad, GamepadAxis, GamepadButton, Keyboard};

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
    VolumeUp,
    VolumeDown,
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
            Action::VolumeUp => "ui_volume_up",
            Action::VolumeDown => "ui_volume_down",
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
        Capability::Keyboard(key) => match key {
            Keyboard::KeyEsc => vec![Action::Back],
            Keyboard::Key1 => vec![Action::None],
            Keyboard::Key2 => vec![Action::None],
            Keyboard::Key3 => vec![Action::None],
            Keyboard::Key4 => vec![Action::None],
            Keyboard::Key5 => vec![Action::None],
            Keyboard::Key6 => vec![Action::None],
            Keyboard::Key7 => vec![Action::None],
            Keyboard::Key8 => vec![Action::None],
            Keyboard::Key9 => vec![Action::None],
            Keyboard::Key0 => vec![Action::None],
            Keyboard::KeyMinus => vec![Action::None],
            Keyboard::KeyEqual => vec![Action::None],
            Keyboard::KeyBackspace => vec![Action::Back],
            Keyboard::KeyTab => vec![Action::None],
            Keyboard::KeyQ => vec![Action::None],
            Keyboard::KeyW => vec![Action::None],
            Keyboard::KeyE => vec![Action::None],
            Keyboard::KeyR => vec![Action::None],
            Keyboard::KeyT => vec![Action::None],
            Keyboard::KeyY => vec![Action::None],
            Keyboard::KeyU => vec![Action::None],
            Keyboard::KeyI => vec![Action::None],
            Keyboard::KeyO => vec![Action::None],
            Keyboard::KeyP => vec![Action::None],
            Keyboard::KeyLeftBrace => vec![Action::None],
            Keyboard::KeyRightBrace => vec![Action::None],
            Keyboard::KeyEnter => vec![Action::Accept],
            Keyboard::KeyLeftCtrl => vec![Action::None],
            Keyboard::KeyA => vec![Action::None],
            Keyboard::KeyS => vec![Action::None],
            Keyboard::KeyD => vec![Action::None],
            Keyboard::KeyF => vec![Action::None],
            Keyboard::KeyG => vec![Action::None],
            Keyboard::KeyH => vec![Action::None],
            Keyboard::KeyJ => vec![Action::None],
            Keyboard::KeyK => vec![Action::None],
            Keyboard::KeyL => vec![Action::None],
            Keyboard::KeySemicolon => vec![Action::None],
            Keyboard::KeyApostrophe => vec![Action::None],
            Keyboard::KeyGrave => vec![Action::None],
            Keyboard::KeyLeftShift => vec![Action::None],
            Keyboard::KeyBackslash => vec![Action::None],
            Keyboard::KeyZ => vec![Action::None],
            Keyboard::KeyX => vec![Action::None],
            Keyboard::KeyC => vec![Action::None],
            Keyboard::KeyV => vec![Action::None],
            Keyboard::KeyB => vec![Action::None],
            Keyboard::KeyN => vec![Action::None],
            Keyboard::KeyM => vec![Action::None],
            Keyboard::KeyComma => vec![Action::None],
            Keyboard::KeyDot => vec![Action::None],
            Keyboard::KeySlash => vec![Action::None],
            Keyboard::KeyRightShift => vec![Action::None],
            Keyboard::KeyKpAsterisk => vec![Action::None],
            Keyboard::KeyLeftAlt => vec![Action::None],
            Keyboard::KeySpace => vec![Action::None],
            Keyboard::KeyCapslock => vec![Action::None],
            Keyboard::KeyF1 => vec![Action::None],
            Keyboard::KeyF2 => vec![Action::None],
            Keyboard::KeyF3 => vec![Action::None],
            Keyboard::KeyF4 => vec![Action::None],
            Keyboard::KeyF5 => vec![Action::None],
            Keyboard::KeyF6 => vec![Action::None],
            Keyboard::KeyF7 => vec![Action::None],
            Keyboard::KeyF8 => vec![Action::None],
            Keyboard::KeyF9 => vec![Action::None],
            Keyboard::KeyF10 => vec![Action::None],
            Keyboard::KeyNumlock => vec![Action::None],
            Keyboard::KeyScrollLock => vec![Action::None],
            Keyboard::KeyKp7 => vec![Action::None],
            Keyboard::KeyKp8 => vec![Action::None],
            Keyboard::KeyKp9 => vec![Action::None],
            Keyboard::KeyKpMinus => vec![Action::None],
            Keyboard::KeyKp4 => vec![Action::None],
            Keyboard::KeyKp5 => vec![Action::None],
            Keyboard::KeyKp6 => vec![Action::None],
            Keyboard::KeyKpPlus => vec![Action::None],
            Keyboard::KeyKp1 => vec![Action::None],
            Keyboard::KeyKp2 => vec![Action::None],
            Keyboard::KeyKp3 => vec![Action::None],
            Keyboard::KeyKp0 => vec![Action::None],
            Keyboard::KeyKpDot => vec![Action::None],
            Keyboard::KeyZenkakuhankaku => vec![Action::None],
            Keyboard::Key102nd => vec![Action::None],
            Keyboard::KeyF11 => vec![Action::None],
            Keyboard::KeyF12 => vec![Action::None],
            Keyboard::KeyRo => vec![Action::None],
            Keyboard::KeyKatakana => vec![Action::None],
            Keyboard::KeyHiragana => vec![Action::None],
            Keyboard::KeyHenkan => vec![Action::None],
            Keyboard::KeyKatakanaHiragana => vec![Action::None],
            Keyboard::KeyMuhenkan => vec![Action::None],
            Keyboard::KeyKpJpComma => vec![Action::None],
            Keyboard::KeyKpEnter => vec![Action::None],
            Keyboard::KeyRightCtrl => vec![Action::None],
            Keyboard::KeyKpSlash => vec![Action::None],
            Keyboard::KeySysrq => vec![Action::None],
            Keyboard::KeyRightAlt => vec![Action::None],
            Keyboard::KeyHome => vec![Action::None],
            Keyboard::KeyUp => vec![Action::Up],
            Keyboard::KeyPageUp => vec![Action::None],
            Keyboard::KeyLeft => vec![Action::Left],
            Keyboard::KeyRight => vec![Action::Right],
            Keyboard::KeyEnd => vec![Action::None],
            Keyboard::KeyDown => vec![Action::Down],
            Keyboard::KeyPageDown => vec![Action::None],
            Keyboard::KeyInsert => vec![Action::None],
            Keyboard::KeyDelete => vec![Action::None],
            Keyboard::KeyMute => vec![Action::None],
            Keyboard::KeyVolumeDown => vec![Action::VolumeUp],
            Keyboard::KeyVolumeUp => vec![Action::VolumeDown],
            Keyboard::KeyPower => vec![Action::None],
            Keyboard::KeyKpEqual => vec![Action::None],
            Keyboard::KeyPause => vec![Action::None],
            Keyboard::KeyKpComma => vec![Action::None],
            Keyboard::KeyHanja => vec![Action::None],
            Keyboard::KeyYen => vec![Action::None],
            Keyboard::KeyLeftMeta => vec![Action::Guide],
            Keyboard::KeyRightMeta => vec![Action::Guide],
            Keyboard::KeyCompose => vec![Action::None],
            Keyboard::KeyStop => vec![Action::None],
            Keyboard::KeyAgain => vec![Action::None],
            Keyboard::KeyProps => vec![Action::None],
            Keyboard::KeyUndo => vec![Action::None],
            Keyboard::KeyFront => vec![Action::None],
            Keyboard::KeyCopy => vec![Action::None],
            Keyboard::KeyOpen => vec![Action::None],
            Keyboard::KeyPaste => vec![Action::None],
            Keyboard::KeyFind => vec![Action::None],
            Keyboard::KeyCut => vec![Action::None],
            Keyboard::KeyHelp => vec![Action::None],
            Keyboard::KeyCalc => vec![Action::None],
            Keyboard::KeySleep => vec![Action::None],
            Keyboard::KeyWww => vec![Action::None],
            Keyboard::KeyBack => vec![Action::None],
            Keyboard::KeyForward => vec![Action::None],
            Keyboard::KeyEjectCD => vec![Action::None],
            Keyboard::KeyNextSong => vec![Action::None],
            Keyboard::KeyPlayPause => vec![Action::None],
            Keyboard::KeyPreviousSong => vec![Action::None],
            Keyboard::KeyStopCD => vec![Action::None],
            Keyboard::KeyRefresh => vec![Action::None],
            Keyboard::KeyEdit => vec![Action::None],
            Keyboard::KeyScrollUp => vec![Action::None],
            Keyboard::KeyScrollDown => vec![Action::None],
            Keyboard::KeyKpLeftParen => vec![Action::None],
            Keyboard::KeyKpRightParen => vec![Action::None],
            Keyboard::KeyF13 => vec![Action::None],
            Keyboard::KeyF14 => vec![Action::None],
            Keyboard::KeyF15 => vec![Action::None],
            Keyboard::KeyF16 => vec![Action::None],
            Keyboard::KeyF17 => vec![Action::None],
            Keyboard::KeyF18 => vec![Action::None],
            Keyboard::KeyF19 => vec![Action::None],
            Keyboard::KeyF20 => vec![Action::None],
            Keyboard::KeyF21 => vec![Action::None],
            Keyboard::KeyF22 => vec![Action::None],
            Keyboard::KeyF23 => vec![Action::None],
            Keyboard::KeyF24 => vec![Action::None],
            Keyboard::KeyProg1 => vec![Action::None],
        },
    }
}

/// Returns a DBus event from the given DBus event action and input value.
fn dbus_event_from_value(action: Action, input_value: InputValue) -> Option<DBusEvent> {
    let value = match input_value {
        InputValue::None => None,
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
