use std::collections::HashMap;

use evdev::{
    AbsInfo, AbsoluteAxisCode, AbsoluteAxisEvent, EventType, InputEvent, KeyCode, RelativeAxisCode,
};

use crate::input::capability::{
    Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Keyboard, Mouse, MouseButton,
    Touch, TouchButton, Touchpad,
};

use super::{native::NativeEvent, value::InputValue};

#[derive(Debug, Clone)]
pub struct EvdevEvent {
    event: InputEvent,
    abs_info: Option<AbsInfo>,
}

impl EvdevEvent {
    /// Returns the normalized value of the event expressed as an [InputValue].
    pub fn get_value(&self) -> InputValue {
        let normal_value = self.get_normalized_value();
        let event_type = self.event.event_type();
        let code = self.event.code();

        // Select the appropiate value type based on the type of input. E.g. Axis
        // values should be in the form of a [InputValue::Vector2].
        match event_type {
            EventType::KEY => {
                KeyCode::new(code);
                if normal_value > 0.5 {
                    InputValue::Bool(true)
                } else {
                    InputValue::Bool(false)
                }
            }
            EventType::ABSOLUTE => match AbsoluteAxisCode(code) {
                AbsoluteAxisCode::ABS_X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                AbsoluteAxisCode::ABS_RX => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_RY => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                AbsoluteAxisCode::ABS_HAT0X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_HAT0Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                AbsoluteAxisCode::ABS_HAT1X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_HAT1Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                AbsoluteAxisCode::ABS_HAT2X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_HAT2Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                AbsoluteAxisCode::ABS_HAT3X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                AbsoluteAxisCode::ABS_HAT3Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                _ => InputValue::Float(normal_value),
            },
            EventType::RELATIVE => match RelativeAxisCode(code) {
                RelativeAxisCode::REL_X => InputValue::Vector2 {
                    x: Some(normal_value),
                    y: None,
                },
                RelativeAxisCode::REL_Y => InputValue::Vector2 {
                    x: None,
                    y: Some(normal_value),
                },
                _ => InputValue::Float(normal_value),
            },

            _ => InputValue::Float(normal_value),
        }
    }

    /// Returns the normalized value of the event. This will be a value that
    /// ranges from -1.0 to 1.0 based on the minimum and maximum values.
    pub fn get_normalized_value(&self) -> f64 {
        let raw_value = self.event.value();

        // If this event has ABS info, normalize the value
        if let Some(info) = self.abs_info {
            let code = self.event.code();
            match AbsoluteAxisCode(code) {
                AbsoluteAxisCode::ABS_Z => normalize_unsigned_value(raw_value, info.maximum()),
                AbsoluteAxisCode::ABS_RZ => normalize_unsigned_value(raw_value, info.maximum()),
                _ => normalize_signed_value(raw_value, info.minimum(), info.maximum()),
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

    /// Returns the capability that this event fulfills. This is used to translate
    /// evdev events into [NativeEvent]s.
    pub fn as_capability(&self) -> Capability {
        let event_type = self.event.event_type();
        let code = self.event.code();
        match event_type {
            EventType::SYNCHRONIZATION => Capability::Sync,
            EventType::KEY => match KeyCode::new(code) {
                // Mouse Buttons
                KeyCode::BTN_LEFT => Capability::Mouse(Mouse::Button(MouseButton::Left)),
                KeyCode::BTN_RIGHT => Capability::Mouse(Mouse::Button(MouseButton::Right)),
                KeyCode::BTN_MIDDLE => Capability::Mouse(Mouse::Button(MouseButton::Middle)),
                KeyCode::BTN_SIDE => Capability::Mouse(Mouse::Button(MouseButton::Side)),
                KeyCode::BTN_EXTRA => Capability::Mouse(Mouse::Button(MouseButton::Extra)),
                // Gamepad Buttons
                KeyCode::BTN_SOUTH => Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                KeyCode::BTN_NORTH => Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
                KeyCode::BTN_WEST => Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
                KeyCode::BTN_EAST => Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
                KeyCode::BTN_TL => Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
                KeyCode::BTN_TR => Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                KeyCode::BTN_START => Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
                KeyCode::BTN_SELECT => Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
                KeyCode::BTN_MODE => Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                KeyCode::BTN_THUMBL => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick))
                }
                KeyCode::BTN_THUMBR => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick))
                }
                KeyCode::KEY_ESC => Capability::Keyboard(Keyboard::KeyEsc),
                KeyCode::KEY_1 => Capability::Keyboard(Keyboard::Key1),
                KeyCode::KEY_2 => Capability::Keyboard(Keyboard::Key2),
                KeyCode::KEY_3 => Capability::Keyboard(Keyboard::Key3),
                KeyCode::KEY_4 => Capability::Keyboard(Keyboard::Key4),
                KeyCode::KEY_5 => Capability::Keyboard(Keyboard::Key5),
                KeyCode::KEY_6 => Capability::Keyboard(Keyboard::Key6),
                KeyCode::KEY_7 => Capability::Keyboard(Keyboard::Key7),
                KeyCode::KEY_8 => Capability::Keyboard(Keyboard::Key8),
                KeyCode::KEY_9 => Capability::Keyboard(Keyboard::Key9),
                KeyCode::KEY_0 => Capability::Keyboard(Keyboard::Key0),
                KeyCode::KEY_MINUS => Capability::Keyboard(Keyboard::KeyMinus),
                KeyCode::KEY_EQUAL => Capability::Keyboard(Keyboard::KeyEqual),
                KeyCode::KEY_BACKSPACE => Capability::Keyboard(Keyboard::KeyBackspace),
                KeyCode::KEY_TAB => Capability::Keyboard(Keyboard::KeyTab),
                KeyCode::KEY_Q => Capability::Keyboard(Keyboard::KeyQ),
                KeyCode::KEY_W => Capability::Keyboard(Keyboard::KeyW),
                KeyCode::KEY_E => Capability::Keyboard(Keyboard::KeyE),
                KeyCode::KEY_R => Capability::Keyboard(Keyboard::KeyR),
                KeyCode::KEY_T => Capability::Keyboard(Keyboard::KeyT),
                KeyCode::KEY_Y => Capability::Keyboard(Keyboard::KeyY),
                KeyCode::KEY_U => Capability::Keyboard(Keyboard::KeyU),
                KeyCode::KEY_I => Capability::Keyboard(Keyboard::KeyI),
                KeyCode::KEY_O => Capability::Keyboard(Keyboard::KeyO),
                KeyCode::KEY_P => Capability::Keyboard(Keyboard::KeyP),
                KeyCode::KEY_LEFTBRACE => Capability::Keyboard(Keyboard::KeyLeftBrace),
                KeyCode::KEY_RIGHTBRACE => Capability::Keyboard(Keyboard::KeyRightBrace),
                KeyCode::KEY_ENTER => Capability::Keyboard(Keyboard::KeyEnter),
                KeyCode::KEY_LEFTCTRL => Capability::Keyboard(Keyboard::KeyLeftCtrl),
                KeyCode::KEY_A => Capability::Keyboard(Keyboard::KeyA),
                KeyCode::KEY_S => Capability::Keyboard(Keyboard::KeyS),
                KeyCode::KEY_D => Capability::Keyboard(Keyboard::KeyD),
                KeyCode::KEY_F => Capability::Keyboard(Keyboard::KeyF),
                KeyCode::KEY_G => Capability::Keyboard(Keyboard::KeyG),
                KeyCode::KEY_H => Capability::Keyboard(Keyboard::KeyH),
                KeyCode::KEY_J => Capability::Keyboard(Keyboard::KeyJ),
                KeyCode::KEY_K => Capability::Keyboard(Keyboard::KeyK),
                KeyCode::KEY_L => Capability::Keyboard(Keyboard::KeyL),
                KeyCode::KEY_SEMICOLON => Capability::Keyboard(Keyboard::KeySemicolon),
                KeyCode::KEY_APOSTROPHE => Capability::Keyboard(Keyboard::KeyApostrophe),
                KeyCode::KEY_GRAVE => Capability::Keyboard(Keyboard::KeyGrave),
                KeyCode::KEY_LEFTSHIFT => Capability::Keyboard(Keyboard::KeyLeftShift),
                KeyCode::KEY_BACKSLASH => Capability::Keyboard(Keyboard::KeyBackslash),
                KeyCode::KEY_Z => Capability::Keyboard(Keyboard::KeyZ),
                KeyCode::KEY_X => Capability::Keyboard(Keyboard::KeyX),
                KeyCode::KEY_C => Capability::Keyboard(Keyboard::KeyC),
                KeyCode::KEY_V => Capability::Keyboard(Keyboard::KeyV),
                KeyCode::KEY_B => Capability::Keyboard(Keyboard::KeyB),
                KeyCode::KEY_N => Capability::Keyboard(Keyboard::KeyN),
                KeyCode::KEY_M => Capability::Keyboard(Keyboard::KeyM),
                KeyCode::KEY_COMMA => Capability::Keyboard(Keyboard::KeyComma),
                KeyCode::KEY_DOT => Capability::Keyboard(Keyboard::KeyDot),
                KeyCode::KEY_SLASH => Capability::Keyboard(Keyboard::KeySlash),
                KeyCode::KEY_RIGHTSHIFT => Capability::Keyboard(Keyboard::KeyRightShift),
                KeyCode::KEY_KPASTERISK => Capability::Keyboard(Keyboard::KeyKpAsterisk),
                KeyCode::KEY_LEFTALT => Capability::Keyboard(Keyboard::KeyLeftAlt),
                KeyCode::KEY_SPACE => Capability::Keyboard(Keyboard::KeySpace),
                KeyCode::KEY_CAPSLOCK => Capability::Keyboard(Keyboard::KeyCapslock),
                KeyCode::KEY_F1 => Capability::Keyboard(Keyboard::KeyF1),
                KeyCode::KEY_F2 => Capability::Keyboard(Keyboard::KeyF2),
                KeyCode::KEY_F3 => Capability::Keyboard(Keyboard::KeyF3),
                KeyCode::KEY_F4 => Capability::Keyboard(Keyboard::KeyF4),
                KeyCode::KEY_F5 => Capability::Keyboard(Keyboard::KeyF5),
                KeyCode::KEY_F6 => Capability::Keyboard(Keyboard::KeyF6),
                KeyCode::KEY_F7 => Capability::Keyboard(Keyboard::KeyF7),
                KeyCode::KEY_F8 => Capability::Keyboard(Keyboard::KeyF8),
                KeyCode::KEY_F9 => Capability::Keyboard(Keyboard::KeyF9),
                KeyCode::KEY_F10 => Capability::Keyboard(Keyboard::KeyF10),
                KeyCode::KEY_NUMLOCK => Capability::Keyboard(Keyboard::KeyNumlock),
                KeyCode::KEY_SCROLLLOCK => Capability::Keyboard(Keyboard::KeyScrollLock),
                KeyCode::KEY_KP7 => Capability::Keyboard(Keyboard::KeyKp7),
                KeyCode::KEY_KP8 => Capability::Keyboard(Keyboard::KeyKp8),
                KeyCode::KEY_KP9 => Capability::Keyboard(Keyboard::KeyKp9),
                KeyCode::KEY_KPMINUS => Capability::Keyboard(Keyboard::KeyKpMinus),
                KeyCode::KEY_KP4 => Capability::Keyboard(Keyboard::KeyKp4),
                KeyCode::KEY_KP5 => Capability::Keyboard(Keyboard::KeyKp5),
                KeyCode::KEY_KP6 => Capability::Keyboard(Keyboard::KeyKp6),
                KeyCode::KEY_KPPLUS => Capability::Keyboard(Keyboard::KeyKpPlus),
                KeyCode::KEY_KP1 => Capability::Keyboard(Keyboard::KeyKp1),
                KeyCode::KEY_KP2 => Capability::Keyboard(Keyboard::KeyKp2),
                KeyCode::KEY_KP3 => Capability::Keyboard(Keyboard::KeyKp3),
                KeyCode::KEY_KP0 => Capability::Keyboard(Keyboard::KeyKp0),
                KeyCode::KEY_KPDOT => Capability::Keyboard(Keyboard::KeyKpDot),
                KeyCode::KEY_ZENKAKUHANKAKU => Capability::Keyboard(Keyboard::KeyZenkakuhankaku),
                KeyCode::KEY_102ND => Capability::Keyboard(Keyboard::Key102nd),
                KeyCode::KEY_F11 => Capability::Keyboard(Keyboard::KeyF11),
                KeyCode::KEY_F12 => Capability::Keyboard(Keyboard::KeyF12),
                KeyCode::KEY_RO => Capability::Keyboard(Keyboard::KeyRo),
                KeyCode::KEY_KATAKANA => Capability::Keyboard(Keyboard::KeyKatakana),
                KeyCode::KEY_HIRAGANA => Capability::Keyboard(Keyboard::KeyHiragana),
                KeyCode::KEY_HENKAN => Capability::Keyboard(Keyboard::KeyHenkan),
                KeyCode::KEY_KATAKANAHIRAGANA => {
                    Capability::Keyboard(Keyboard::KeyKatakanaHiragana)
                }
                KeyCode::KEY_MUHENKAN => Capability::Keyboard(Keyboard::KeyMuhenkan),
                KeyCode::KEY_KPJPCOMMA => Capability::Keyboard(Keyboard::KeyKpJpComma),
                KeyCode::KEY_KPENTER => Capability::Keyboard(Keyboard::KeyKpEnter),
                KeyCode::KEY_RIGHTCTRL => Capability::Keyboard(Keyboard::KeyRightCtrl),
                KeyCode::KEY_KPSLASH => Capability::Keyboard(Keyboard::KeyKpSlash),
                KeyCode::KEY_SYSRQ => Capability::Keyboard(Keyboard::KeySysrq),
                KeyCode::KEY_RIGHTALT => Capability::Keyboard(Keyboard::KeyRightAlt),
                KeyCode::KEY_LINEFEED => Capability::NotImplemented,
                KeyCode::KEY_HOME => Capability::Keyboard(Keyboard::KeyHome),
                KeyCode::KEY_UP => Capability::Keyboard(Keyboard::KeyUp),
                KeyCode::KEY_PAGEUP => Capability::Keyboard(Keyboard::KeyPageUp),
                KeyCode::KEY_LEFT => Capability::Keyboard(Keyboard::KeyLeft),
                KeyCode::KEY_RIGHT => Capability::Keyboard(Keyboard::KeyRight),
                KeyCode::KEY_END => Capability::Keyboard(Keyboard::KeyEnd),
                KeyCode::KEY_DOWN => Capability::Keyboard(Keyboard::KeyDown),
                KeyCode::KEY_PAGEDOWN => Capability::Keyboard(Keyboard::KeyPageDown),
                KeyCode::KEY_INSERT => Capability::Keyboard(Keyboard::KeyInsert),
                KeyCode::KEY_DELETE => Capability::Keyboard(Keyboard::KeyDelete),
                KeyCode::KEY_MACRO => Capability::NotImplemented,
                KeyCode::KEY_MUTE => Capability::Keyboard(Keyboard::KeyMute),
                KeyCode::KEY_VOLUMEDOWN => Capability::Keyboard(Keyboard::KeyVolumeDown),
                KeyCode::KEY_VOLUMEUP => Capability::Keyboard(Keyboard::KeyVolumeUp),
                KeyCode::KEY_POWER => Capability::Keyboard(Keyboard::KeyPower),
                KeyCode::KEY_KPEQUAL => Capability::Keyboard(Keyboard::KeyKpEqual),
                KeyCode::KEY_KPPLUSMINUS => Capability::NotImplemented,
                KeyCode::KEY_PAUSE => Capability::Keyboard(Keyboard::KeyPause),
                KeyCode::KEY_SCALE => Capability::NotImplemented,
                KeyCode::KEY_KPCOMMA => Capability::Keyboard(Keyboard::KeyKpComma),
                KeyCode::KEY_HANGEUL => Capability::NotImplemented,
                KeyCode::KEY_HANJA => Capability::Keyboard(Keyboard::KeyHanja),
                KeyCode::KEY_YEN => Capability::Keyboard(Keyboard::KeyYen),
                KeyCode::KEY_LEFTMETA => Capability::Keyboard(Keyboard::KeyLeftMeta),
                KeyCode::KEY_RIGHTMETA => Capability::Keyboard(Keyboard::KeyRightMeta),
                KeyCode::KEY_COMPOSE => Capability::Keyboard(Keyboard::KeyCompose),
                KeyCode::KEY_STOP => Capability::Keyboard(Keyboard::KeyStop),
                KeyCode::KEY_AGAIN => Capability::Keyboard(Keyboard::KeyAgain),
                KeyCode::KEY_PROPS => Capability::Keyboard(Keyboard::KeyProps),
                KeyCode::KEY_UNDO => Capability::Keyboard(Keyboard::KeyUndo),
                KeyCode::KEY_FRONT => Capability::Keyboard(Keyboard::KeyFront),
                KeyCode::KEY_COPY => Capability::Keyboard(Keyboard::KeyCopy),
                KeyCode::KEY_OPEN => Capability::Keyboard(Keyboard::KeyOpen),
                KeyCode::KEY_PASTE => Capability::Keyboard(Keyboard::KeyPaste),
                KeyCode::KEY_FIND => Capability::Keyboard(Keyboard::KeyFind),
                KeyCode::KEY_CUT => Capability::Keyboard(Keyboard::KeyCut),
                KeyCode::KEY_HELP => Capability::Keyboard(Keyboard::KeyHelp),
                KeyCode::KEY_MENU => Capability::NotImplemented,
                KeyCode::KEY_CALC => Capability::Keyboard(Keyboard::KeyCalc),
                KeyCode::KEY_SETUP => Capability::NotImplemented,
                KeyCode::KEY_SLEEP => Capability::Keyboard(Keyboard::KeySleep),
                KeyCode::KEY_WAKEUP => Capability::NotImplemented,
                KeyCode::KEY_FILE => Capability::NotImplemented,
                KeyCode::KEY_SENDFILE => Capability::NotImplemented,
                KeyCode::KEY_DELETEFILE => Capability::NotImplemented,
                KeyCode::KEY_XFER => Capability::NotImplemented,
                KeyCode::KEY_PROG1 => Capability::Keyboard(Keyboard::KeyProg1),
                KeyCode::KEY_PROG2 => Capability::NotImplemented,
                KeyCode::KEY_WWW => Capability::Keyboard(Keyboard::KeyWww),
                KeyCode::KEY_MSDOS => Capability::NotImplemented,
                KeyCode::KEY_COFFEE => Capability::NotImplemented,
                KeyCode::KEY_ROTATE_DISPLAY => Capability::NotImplemented,
                KeyCode::KEY_CYCLEWINDOWS => Capability::NotImplemented,
                KeyCode::KEY_MAIL => Capability::NotImplemented,
                KeyCode::KEY_BOOKMARKS => Capability::NotImplemented,
                KeyCode::KEY_COMPUTER => Capability::NotImplemented,
                KeyCode::KEY_BACK => Capability::Keyboard(Keyboard::KeyBack),
                KeyCode::KEY_FORWARD => Capability::Keyboard(Keyboard::KeyForward),
                KeyCode::KEY_CLOSECD => Capability::NotImplemented,
                KeyCode::KEY_EJECTCD => Capability::Keyboard(Keyboard::KeyEjectCD),
                KeyCode::KEY_EJECTCLOSECD => Capability::NotImplemented,
                KeyCode::KEY_NEXTSONG => Capability::Keyboard(Keyboard::KeyNextSong),
                KeyCode::KEY_PLAYPAUSE => Capability::Keyboard(Keyboard::KeyPlayPause),
                KeyCode::KEY_PREVIOUSSONG => Capability::Keyboard(Keyboard::KeyPreviousSong),
                KeyCode::KEY_STOPCD => Capability::Keyboard(Keyboard::KeyStopCD),
                KeyCode::KEY_RECORD => Capability::NotImplemented,
                KeyCode::KEY_REWIND => Capability::NotImplemented,
                KeyCode::KEY_PHONE => Capability::NotImplemented,
                KeyCode::KEY_ISO => Capability::NotImplemented,
                KeyCode::KEY_CONFIG => Capability::NotImplemented,
                KeyCode::KEY_HOMEPAGE => Capability::NotImplemented,
                KeyCode::KEY_REFRESH => Capability::Keyboard(Keyboard::KeyRefresh),
                KeyCode::KEY_EXIT => Capability::NotImplemented,
                KeyCode::KEY_MOVE => Capability::NotImplemented,
                KeyCode::KEY_EDIT => Capability::Keyboard(Keyboard::KeyEdit),
                KeyCode::KEY_SCROLLUP => Capability::Keyboard(Keyboard::KeyScrollUp),
                KeyCode::KEY_SCROLLDOWN => Capability::Keyboard(Keyboard::KeyScrollDown),
                KeyCode::KEY_KPLEFTPAREN => Capability::Keyboard(Keyboard::KeyKpLeftParen),
                KeyCode::KEY_KPRIGHTPAREN => Capability::Keyboard(Keyboard::KeyKpRightParen),
                KeyCode::KEY_NEW => Capability::NotImplemented,
                KeyCode::KEY_REDO => Capability::NotImplemented,
                KeyCode::KEY_F13 => Capability::Keyboard(Keyboard::KeyF13),
                KeyCode::KEY_F14 => Capability::Keyboard(Keyboard::KeyF14),
                KeyCode::KEY_F15 => Capability::Keyboard(Keyboard::KeyF15),
                KeyCode::KEY_F16 => Capability::Keyboard(Keyboard::KeyF16),
                KeyCode::KEY_F17 => Capability::Keyboard(Keyboard::KeyF17),
                KeyCode::KEY_F18 => Capability::Keyboard(Keyboard::KeyF18),
                KeyCode::KEY_F19 => Capability::Keyboard(Keyboard::KeyF19),
                KeyCode::KEY_F20 => Capability::Keyboard(Keyboard::KeyF20),
                KeyCode::KEY_F21 => Capability::Keyboard(Keyboard::KeyF21),
                KeyCode::KEY_F22 => Capability::Keyboard(Keyboard::KeyF22),
                KeyCode::KEY_F23 => Capability::Keyboard(Keyboard::KeyF23),
                KeyCode::KEY_F24 => Capability::Keyboard(Keyboard::KeyF24),
                KeyCode::KEY_PLAYCD => Capability::NotImplemented,
                KeyCode::KEY_PAUSECD => Capability::NotImplemented,
                KeyCode::KEY_PROG3 => Capability::NotImplemented,
                KeyCode::KEY_PROG4 => Capability::NotImplemented,
                KeyCode::KEY_DASHBOARD => Capability::NotImplemented,
                KeyCode::KEY_SUSPEND => Capability::NotImplemented,
                KeyCode::KEY_CLOSE => Capability::NotImplemented,
                KeyCode::KEY_PLAY => Capability::NotImplemented,
                KeyCode::KEY_FASTFORWARD => Capability::NotImplemented,
                KeyCode::KEY_BASSBOOST => Capability::NotImplemented,
                KeyCode::KEY_PRINT => Capability::NotImplemented,
                KeyCode::KEY_HP => Capability::NotImplemented,
                KeyCode::KEY_CAMERA => Capability::NotImplemented,
                KeyCode::KEY_SOUND => Capability::NotImplemented,
                KeyCode::KEY_QUESTION => Capability::NotImplemented,
                KeyCode::KEY_EMAIL => Capability::NotImplemented,
                KeyCode::KEY_CHAT => Capability::NotImplemented,
                KeyCode::KEY_SEARCH => Capability::NotImplemented,
                KeyCode::KEY_CONNECT => Capability::NotImplemented,
                KeyCode::KEY_FINANCE => Capability::NotImplemented,
                KeyCode::KEY_SPORT => Capability::NotImplemented,
                KeyCode::KEY_SHOP => Capability::NotImplemented,
                KeyCode::KEY_ALTERASE => Capability::NotImplemented,
                KeyCode::KEY_CANCEL => Capability::NotImplemented,
                KeyCode::KEY_BRIGHTNESSDOWN => Capability::NotImplemented,
                KeyCode::KEY_BRIGHTNESSUP => Capability::NotImplemented,
                KeyCode::KEY_MEDIA => Capability::NotImplemented,
                KeyCode::KEY_SWITCHVIDEOMODE => Capability::NotImplemented,
                KeyCode::KEY_KBDILLUMTOGGLE => Capability::NotImplemented,
                KeyCode::KEY_KBDILLUMDOWN => Capability::NotImplemented,
                KeyCode::KEY_KBDILLUMUP => Capability::NotImplemented,
                KeyCode::KEY_SEND => Capability::NotImplemented,
                KeyCode::KEY_REPLY => Capability::NotImplemented,
                KeyCode::KEY_FORWARDMAIL => Capability::NotImplemented,
                KeyCode::KEY_SAVE => Capability::NotImplemented,
                KeyCode::KEY_DOCUMENTS => Capability::NotImplemented,
                KeyCode::KEY_BATTERY => Capability::NotImplemented,
                KeyCode::KEY_BLUETOOTH => Capability::NotImplemented,
                KeyCode::KEY_WLAN => Capability::NotImplemented,
                KeyCode::KEY_UWB => Capability::NotImplemented,
                KeyCode::KEY_UNKNOWN => Capability::NotImplemented,
                KeyCode::KEY_VIDEO_NEXT => Capability::NotImplemented,
                KeyCode::KEY_VIDEO_PREV => Capability::NotImplemented,
                KeyCode::KEY_BRIGHTNESS_CYCLE => Capability::NotImplemented,
                KeyCode::KEY_BRIGHTNESS_AUTO => Capability::NotImplemented,
                KeyCode::KEY_DISPLAY_OFF => Capability::NotImplemented,
                KeyCode::KEY_WWAN => Capability::NotImplemented,
                KeyCode::KEY_RFKILL => Capability::NotImplemented,
                KeyCode::KEY_MICMUTE => Capability::NotImplemented,
                _ => Capability::NotImplemented,
            },
            EventType::ABSOLUTE => match AbsoluteAxisCode(code) {
                AbsoluteAxisCode::ABS_X => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
                }
                AbsoluteAxisCode::ABS_Y => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
                }
                AbsoluteAxisCode::ABS_RX => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
                }
                AbsoluteAxisCode::ABS_RY => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
                }
                AbsoluteAxisCode::ABS_Z => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
                }
                AbsoluteAxisCode::ABS_RZ => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
                }
                AbsoluteAxisCode::ABS_HAT0X => Capability::Gamepad(Gamepad::Axis(
                    GamepadAxis::Buttons(GamepadButton::DPadLeft, GamepadButton::DPadRight),
                )),
                AbsoluteAxisCode::ABS_HAT0Y => Capability::Gamepad(Gamepad::Axis(
                    GamepadAxis::Buttons(GamepadButton::DPadUp, GamepadButton::DPadDown),
                )),
                _ => Capability::NotImplemented,
            },
            EventType::RELATIVE => match RelativeAxisCode(code) {
                RelativeAxisCode::REL_X => Capability::Mouse(Mouse::Motion),
                RelativeAxisCode::REL_Y => Capability::Mouse(Mouse::Motion),
                RelativeAxisCode::REL_WHEEL => Capability::NotImplemented,
                _ => Capability::NotImplemented,
            },
            EventType::MISC => Capability::NotImplemented,
            EventType::SWITCH => Capability::NotImplemented,
            EventType::LED => Capability::NotImplemented,
            EventType::SOUND => Capability::NotImplemented,
            EventType::REPEAT => Capability::NotImplemented,
            EventType::FORCEFEEDBACK => Capability::NotImplemented,
            EventType::POWER => Capability::NotImplemented,
            EventType::FORCEFEEDBACKSTATUS => Capability::NotImplemented,
            EventType::UINPUT => Capability::NotImplemented,
            _ => Capability::NotImplemented,
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
    /// Translate a [NativeEvent] into one or more [EvdevEvent].
    pub fn from_native_event(
        event: NativeEvent,
        axis_map: HashMap<AbsoluteAxisCode, AbsInfo>,
    ) -> Vec<Self> {
        // Native events can be translated into one or more evdev events
        let mut events = Vec::new();

        // Determine the event type to use based on the capability
        let event_type = event_type_from_capability(event.as_capability());

        // If an event type cannot be determined, return an empty event
        let Some(event_type) = event_type else {
            //log::debug!("Unable to determine evdev event type");
            return events;
        };

        // Determine the event code(s) based on capability. Axis events can
        // translate to multiple events.
        let codes = event_codes_from_capability(event.as_capability());

        // Process each event code
        for code in codes {
            // Get the axis information if this is an ABS event.
            let axis_info = if event_type == EventType::ABSOLUTE {
                axis_map.get(&AbsoluteAxisCode(code)).copied()
            } else {
                None
            };

            // Get the axis direction if this this an ABS event and we need to
            // translate binary input into axis input. (e.g. DPad buttons)
            let axis_direction = if event_type == EventType::ABSOLUTE {
                Some(axis_direction_from_capability(event.as_capability()))
            } else {
                None
            };

            // Get the input value from the event and convert it into an evdev
            // input event.
            let value = event.get_value();
            let event = input_event_from_value(event_type, code, axis_info, axis_direction, value);
            if event.is_none() {
                continue;
            }

            events.push(EvdevEvent::from(event.unwrap()));
        }

        events
    }
}

/// Returns the event type responsible for handling the given input capability.
fn event_type_from_capability(capability: Capability) -> Option<EventType> {
    match capability {
        Capability::Sync => Some(EventType::SYNCHRONIZATION),
        Capability::Keyboard(_) => Some(EventType::KEY),
        Capability::Mouse(mouse) => match mouse {
            Mouse::Motion => Some(EventType::RELATIVE),
            Mouse::Button(_) => Some(EventType::KEY),
        },
        Capability::Gamepad(gamepad) => match gamepad {
            Gamepad::Button(button) => match button {
                GamepadButton::DPadUp => Some(EventType::ABSOLUTE),
                GamepadButton::DPadDown => Some(EventType::ABSOLUTE),
                GamepadButton::DPadLeft => Some(EventType::ABSOLUTE),
                GamepadButton::DPadRight => Some(EventType::ABSOLUTE),
                _ => Some(EventType::KEY),
            },
            Gamepad::Axis(_) => Some(EventType::ABSOLUTE),
            Gamepad::Trigger(_) => Some(EventType::ABSOLUTE),
            Gamepad::Accelerometer => None,
            Gamepad::Gyro => None,
        },
        _ => None,
    }
}

/// Returns the axis direction for the given input capability. This is primarily
/// used to translate direction binary button events into axis events.
fn axis_direction_from_capability(capability: Capability) -> AxisDirection {
    match capability {
        Capability::Gamepad(gamepad) => {
            if let Gamepad::Button(button) = gamepad {
                match button {
                    GamepadButton::DPadUp => AxisDirection::Negative,
                    GamepadButton::DPadDown => AxisDirection::Positive,
                    GamepadButton::DPadLeft => AxisDirection::Negative,
                    GamepadButton::DPadRight => AxisDirection::Positive,
                    _ => AxisDirection::None,
                }
            } else {
                AxisDirection::None
            }
        }
        _ => AxisDirection::None,
    }
}

/// Returns a list of event codes responsible for handling the given input capability.
/// This is typically used to translate a [NativeEvent] into an [EvdevEvent].
fn event_codes_from_capability(capability: Capability) -> Vec<u16> {
    match capability {
        Capability::None => vec![],
        Capability::NotImplemented => vec![],
        Capability::Sync => vec![0],
        Capability::DBus(_) => vec![],
        Capability::Gamepad(gamepad) => match gamepad {
            Gamepad::Button(btn) => match btn {
                GamepadButton::South => vec![KeyCode::BTN_SOUTH.0],
                GamepadButton::East => vec![KeyCode::BTN_EAST.0],
                GamepadButton::North => vec![KeyCode::BTN_NORTH.0],
                GamepadButton::West => vec![KeyCode::BTN_WEST.0],
                GamepadButton::LeftBumper => vec![KeyCode::BTN_TL.0],
                GamepadButton::RightBumper => vec![KeyCode::BTN_TR.0],
                GamepadButton::LeftTop => vec![],
                GamepadButton::RightTop => vec![],
                GamepadButton::Start => vec![KeyCode::BTN_START.0],
                GamepadButton::Select => vec![KeyCode::BTN_SELECT.0],
                GamepadButton::Guide => vec![KeyCode::BTN_MODE.0],
                GamepadButton::QuickAccess => vec![],
                GamepadButton::QuickAccess2 => vec![],
                GamepadButton::Keyboard => vec![],
                GamepadButton::Screenshot => vec![],
                GamepadButton::LeftStick => vec![KeyCode::BTN_THUMBL.0],
                GamepadButton::RightStick => vec![KeyCode::BTN_THUMBR.0],
                GamepadButton::DPadUp => vec![AbsoluteAxisCode::ABS_HAT0Y.0],
                GamepadButton::DPadDown => vec![AbsoluteAxisCode::ABS_HAT0Y.0],
                GamepadButton::DPadLeft => vec![AbsoluteAxisCode::ABS_HAT0X.0],
                GamepadButton::DPadRight => vec![AbsoluteAxisCode::ABS_HAT0X.0],
                GamepadButton::LeftTrigger => vec![KeyCode::BTN_TL2.0],
                GamepadButton::LeftPaddle1 => vec![],
                GamepadButton::LeftPaddle2 => vec![],
                GamepadButton::LeftStickTouch => vec![],
                GamepadButton::RightTrigger => vec![KeyCode::BTN_TR2.0],
                GamepadButton::RightPaddle1 => vec![],
                GamepadButton::RightPaddle2 => vec![],
                GamepadButton::RightStickTouch => vec![],
                GamepadButton::LeftPaddle3 => vec![],
                GamepadButton::RightPaddle3 => vec![],
            },
            Gamepad::Axis(axis) => match axis {
                GamepadAxis::LeftStick => {
                    vec![AbsoluteAxisCode::ABS_X.0, AbsoluteAxisCode::ABS_Y.0]
                }
                GamepadAxis::RightStick => {
                    vec![AbsoluteAxisCode::ABS_RX.0, AbsoluteAxisCode::ABS_RY.0]
                }
                GamepadAxis::Hat1 => {
                    vec![AbsoluteAxisCode::ABS_HAT0X.0, AbsoluteAxisCode::ABS_HAT0Y.0]
                }
                GamepadAxis::Hat2 => {
                    vec![AbsoluteAxisCode::ABS_HAT1X.0, AbsoluteAxisCode::ABS_HAT1Y.0]
                }
                GamepadAxis::Hat3 => {
                    vec![AbsoluteAxisCode::ABS_HAT2X.0, AbsoluteAxisCode::ABS_HAT2Y.0]
                }
                GamepadAxis::Buttons(_, _) => {
                    vec![AbsoluteAxisCode::ABS_HAT0X.0, AbsoluteAxisCode::ABS_HAT0Y.0]
                }
            },
            Gamepad::Trigger(trigg) => match trigg {
                GamepadTrigger::LeftTrigger => {
                    vec![AbsoluteAxisCode::ABS_Z.0]
                }
                GamepadTrigger::LeftTouchpadForce => vec![],
                GamepadTrigger::LeftStickForce => vec![],
                GamepadTrigger::RightTrigger => {
                    vec![AbsoluteAxisCode::ABS_RZ.0]
                }
                GamepadTrigger::RightTouchpadForce => vec![],
                GamepadTrigger::RightStickForce => vec![],
            },
            Gamepad::Accelerometer => vec![],
            Gamepad::Gyro => vec![],
        },
        Capability::Mouse(mouse) => match mouse {
            Mouse::Motion => vec![RelativeAxisCode::REL_X.0, RelativeAxisCode::REL_Y.0],
            Mouse::Button(button) => match button {
                MouseButton::Left => vec![KeyCode::BTN_LEFT.0],
                MouseButton::Right => vec![KeyCode::BTN_RIGHT.0],
                MouseButton::Middle => vec![KeyCode::BTN_MIDDLE.0],
                MouseButton::WheelUp => vec![],
                MouseButton::WheelDown => vec![],
                MouseButton::WheelLeft => vec![],
                MouseButton::WheelRight => vec![],
                MouseButton::Extra => vec![KeyCode::BTN_EXTRA.0],
                MouseButton::Side => vec![KeyCode::BTN_SIDE.0],
            },
        },
        Capability::Keyboard(key) => match key {
            Keyboard::KeyEsc => vec![KeyCode::KEY_ESC.0],
            Keyboard::Key1 => vec![KeyCode::KEY_1.0],
            Keyboard::Key2 => vec![KeyCode::KEY_2.0],
            Keyboard::Key3 => vec![KeyCode::KEY_3.0],
            Keyboard::Key4 => vec![KeyCode::KEY_4.0],
            Keyboard::Key5 => vec![KeyCode::KEY_5.0],
            Keyboard::Key6 => vec![KeyCode::KEY_6.0],
            Keyboard::Key7 => vec![KeyCode::KEY_7.0],
            Keyboard::Key8 => vec![KeyCode::KEY_8.0],
            Keyboard::Key9 => vec![KeyCode::KEY_9.0],
            Keyboard::Key0 => vec![KeyCode::KEY_0.0],
            Keyboard::KeyMinus => vec![KeyCode::KEY_MINUS.0],
            Keyboard::KeyEqual => vec![KeyCode::KEY_EQUAL.0],
            Keyboard::KeyBackspace => vec![KeyCode::KEY_BACKSPACE.0],
            Keyboard::KeyTab => vec![KeyCode::KEY_TAB.0],
            Keyboard::KeyQ => vec![KeyCode::KEY_Q.0],
            Keyboard::KeyW => vec![KeyCode::KEY_W.0],
            Keyboard::KeyE => vec![KeyCode::KEY_E.0],
            Keyboard::KeyR => vec![KeyCode::KEY_R.0],
            Keyboard::KeyT => vec![KeyCode::KEY_T.0],
            Keyboard::KeyY => vec![KeyCode::KEY_Y.0],
            Keyboard::KeyU => vec![KeyCode::KEY_U.0],
            Keyboard::KeyI => vec![KeyCode::KEY_I.0],
            Keyboard::KeyO => vec![KeyCode::KEY_O.0],
            Keyboard::KeyP => vec![KeyCode::KEY_P.0],
            Keyboard::KeyLeftBrace => vec![KeyCode::KEY_LEFTBRACE.0],
            Keyboard::KeyRightBrace => vec![KeyCode::KEY_RIGHTBRACE.0],
            Keyboard::KeyEnter => vec![KeyCode::KEY_ENTER.0],
            Keyboard::KeyLeftCtrl => vec![KeyCode::KEY_LEFTCTRL.0],
            Keyboard::KeyA => vec![KeyCode::KEY_A.0],
            Keyboard::KeyS => vec![KeyCode::KEY_S.0],
            Keyboard::KeyD => vec![KeyCode::KEY_D.0],
            Keyboard::KeyF => vec![KeyCode::KEY_F.0],
            Keyboard::KeyG => vec![KeyCode::KEY_G.0],
            Keyboard::KeyH => vec![KeyCode::KEY_H.0],
            Keyboard::KeyJ => vec![KeyCode::KEY_J.0],
            Keyboard::KeyK => vec![KeyCode::KEY_K.0],
            Keyboard::KeyL => vec![KeyCode::KEY_L.0],
            Keyboard::KeySemicolon => vec![KeyCode::KEY_SEMICOLON.0],
            Keyboard::KeyApostrophe => vec![KeyCode::KEY_APOSTROPHE.0],
            Keyboard::KeyGrave => vec![KeyCode::KEY_GRAVE.0],
            Keyboard::KeyLeftShift => vec![KeyCode::KEY_LEFTSHIFT.0],
            Keyboard::KeyBackslash => vec![KeyCode::KEY_BACKSLASH.0],
            Keyboard::KeyZ => vec![KeyCode::KEY_Z.0],
            Keyboard::KeyX => vec![KeyCode::KEY_X.0],
            Keyboard::KeyC => vec![KeyCode::KEY_C.0],
            Keyboard::KeyV => vec![KeyCode::KEY_V.0],
            Keyboard::KeyB => vec![KeyCode::KEY_B.0],
            Keyboard::KeyN => vec![KeyCode::KEY_N.0],
            Keyboard::KeyM => vec![KeyCode::KEY_M.0],
            Keyboard::KeyComma => vec![KeyCode::KEY_COMMA.0],
            Keyboard::KeyDot => vec![KeyCode::KEY_DOT.0],
            Keyboard::KeySlash => vec![KeyCode::KEY_SLASH.0],
            Keyboard::KeyRightShift => vec![KeyCode::KEY_RIGHTSHIFT.0],
            Keyboard::KeyKpAsterisk => vec![KeyCode::KEY_KPASTERISK.0],
            Keyboard::KeyLeftAlt => vec![KeyCode::KEY_LEFTALT.0],
            Keyboard::KeySpace => vec![KeyCode::KEY_SPACE.0],
            Keyboard::KeyCapslock => vec![KeyCode::KEY_CAPSLOCK.0],
            Keyboard::KeyF1 => vec![KeyCode::KEY_F1.0],
            Keyboard::KeyF2 => vec![KeyCode::KEY_F2.0],
            Keyboard::KeyF3 => vec![KeyCode::KEY_F3.0],
            Keyboard::KeyF4 => vec![KeyCode::KEY_F4.0],
            Keyboard::KeyF5 => vec![KeyCode::KEY_F5.0],
            Keyboard::KeyF6 => vec![KeyCode::KEY_F6.0],
            Keyboard::KeyF7 => vec![KeyCode::KEY_F7.0],
            Keyboard::KeyF8 => vec![KeyCode::KEY_F8.0],
            Keyboard::KeyF9 => vec![KeyCode::KEY_F9.0],
            Keyboard::KeyF10 => vec![KeyCode::KEY_F10.0],
            Keyboard::KeyNumlock => vec![KeyCode::KEY_NUMLOCK.0],
            Keyboard::KeyScrollLock => vec![KeyCode::KEY_SCROLLLOCK.0],
            Keyboard::KeyKp7 => vec![KeyCode::KEY_KP7.0],
            Keyboard::KeyKp8 => vec![KeyCode::KEY_KP8.0],
            Keyboard::KeyKp9 => vec![KeyCode::KEY_KP9.0],
            Keyboard::KeyKpMinus => vec![KeyCode::KEY_KPMINUS.0],
            Keyboard::KeyKp4 => vec![KeyCode::KEY_KP4.0],
            Keyboard::KeyKp5 => vec![KeyCode::KEY_KP5.0],
            Keyboard::KeyKp6 => vec![KeyCode::KEY_KP6.0],
            Keyboard::KeyKpPlus => vec![KeyCode::KEY_KPPLUS.0],
            Keyboard::KeyKp1 => vec![KeyCode::KEY_KP1.0],
            Keyboard::KeyKp2 => vec![KeyCode::KEY_KP2.0],
            Keyboard::KeyKp3 => vec![KeyCode::KEY_KP3.0],
            Keyboard::KeyKp0 => vec![KeyCode::KEY_KP0.0],
            Keyboard::KeyKpDot => vec![KeyCode::KEY_KPDOT.0],
            Keyboard::KeyZenkakuhankaku => vec![KeyCode::KEY_ZENKAKUHANKAKU.0],
            Keyboard::Key102nd => vec![KeyCode::KEY_102ND.0],
            Keyboard::KeyF11 => vec![KeyCode::KEY_F11.0],
            Keyboard::KeyF12 => vec![KeyCode::KEY_F12.0],
            Keyboard::KeyRo => vec![KeyCode::KEY_RO.0],
            Keyboard::KeyKatakana => vec![KeyCode::KEY_KATAKANA.0],
            Keyboard::KeyHiragana => vec![KeyCode::KEY_HIRAGANA.0],
            Keyboard::KeyHenkan => vec![KeyCode::KEY_HENKAN.0],
            Keyboard::KeyKatakanaHiragana => vec![KeyCode::KEY_KATAKANAHIRAGANA.0],
            Keyboard::KeyMuhenkan => vec![KeyCode::KEY_MUHENKAN.0],
            Keyboard::KeyKpJpComma => vec![KeyCode::KEY_KPJPCOMMA.0],
            Keyboard::KeyKpEnter => vec![KeyCode::KEY_KPENTER.0],
            Keyboard::KeyRightCtrl => vec![KeyCode::KEY_RIGHTCTRL.0],
            Keyboard::KeyKpSlash => vec![KeyCode::KEY_KPSLASH.0],
            Keyboard::KeySysrq => vec![KeyCode::KEY_SYSRQ.0],
            Keyboard::KeyRightAlt => vec![KeyCode::KEY_RIGHTALT.0],
            Keyboard::KeyHome => vec![KeyCode::KEY_HOME.0],
            Keyboard::KeyUp => vec![KeyCode::KEY_UP.0],
            Keyboard::KeyPageUp => vec![KeyCode::KEY_PAGEUP.0],
            Keyboard::KeyLeft => vec![KeyCode::KEY_LEFT.0],
            Keyboard::KeyRight => vec![KeyCode::KEY_RIGHT.0],
            Keyboard::KeyEnd => vec![KeyCode::KEY_END.0],
            Keyboard::KeyDown => vec![KeyCode::KEY_DOWN.0],
            Keyboard::KeyPageDown => vec![KeyCode::KEY_PAGEDOWN.0],
            Keyboard::KeyInsert => vec![KeyCode::KEY_INSERT.0],
            Keyboard::KeyDelete => vec![KeyCode::KEY_DELETE.0],
            Keyboard::KeyMute => vec![KeyCode::KEY_MUTE.0],
            Keyboard::KeyVolumeDown => vec![KeyCode::KEY_VOLUMEDOWN.0],
            Keyboard::KeyVolumeUp => vec![KeyCode::KEY_VOLUMEUP.0],
            Keyboard::KeyPower => vec![KeyCode::KEY_POWER.0],
            Keyboard::KeyKpEqual => vec![KeyCode::KEY_KPEQUAL.0],
            Keyboard::KeyPause => vec![KeyCode::KEY_PAUSE.0],
            Keyboard::KeyKpComma => vec![KeyCode::KEY_KPCOMMA.0],
            Keyboard::KeyHanja => vec![KeyCode::KEY_HANJA.0],
            Keyboard::KeyYen => vec![KeyCode::KEY_YEN.0],
            Keyboard::KeyLeftMeta => vec![KeyCode::KEY_LEFTMETA.0],
            Keyboard::KeyRightMeta => vec![KeyCode::KEY_RIGHTMETA.0],
            Keyboard::KeyCompose => vec![KeyCode::KEY_COMPOSE.0],
            Keyboard::KeyStop => vec![KeyCode::KEY_STOP.0],
            Keyboard::KeyAgain => vec![KeyCode::KEY_AGAIN.0],
            Keyboard::KeyProps => vec![KeyCode::KEY_PROPS.0],
            Keyboard::KeyUndo => vec![KeyCode::KEY_UNDO.0],
            Keyboard::KeyFront => vec![KeyCode::KEY_FRONT.0],
            Keyboard::KeyCopy => vec![KeyCode::KEY_COPY.0],
            Keyboard::KeyOpen => vec![KeyCode::KEY_OPEN.0],
            Keyboard::KeyPaste => vec![KeyCode::KEY_PASTE.0],
            Keyboard::KeyFind => vec![KeyCode::KEY_FIND.0],
            Keyboard::KeyCut => vec![KeyCode::KEY_CUT.0],
            Keyboard::KeyHelp => vec![KeyCode::KEY_HELP.0],
            Keyboard::KeyCalc => vec![KeyCode::KEY_CALC.0],
            Keyboard::KeySleep => vec![KeyCode::KEY_SLEEP.0],
            Keyboard::KeyWww => vec![KeyCode::KEY_WWW.0],
            Keyboard::KeyBack => vec![KeyCode::KEY_BACK.0],
            Keyboard::KeyForward => vec![KeyCode::KEY_FORWARD.0],
            Keyboard::KeyEjectCD => vec![KeyCode::KEY_EJECTCD.0],
            Keyboard::KeyNextSong => vec![KeyCode::KEY_NEXTSONG.0],
            Keyboard::KeyPlayPause => vec![KeyCode::KEY_PLAYPAUSE.0],
            Keyboard::KeyPreviousSong => vec![KeyCode::KEY_PREVIOUSSONG.0],
            Keyboard::KeyStopCD => vec![KeyCode::KEY_STOPCD.0],
            Keyboard::KeyRefresh => vec![KeyCode::KEY_REFRESH.0],
            Keyboard::KeyEdit => vec![KeyCode::KEY_EDIT.0],
            Keyboard::KeyScrollUp => vec![KeyCode::KEY_SCROLLUP.0],
            Keyboard::KeyScrollDown => vec![KeyCode::KEY_SCROLLDOWN.0],
            Keyboard::KeyKpLeftParen => vec![KeyCode::KEY_KPLEFTPAREN.0],
            Keyboard::KeyKpRightParen => vec![KeyCode::KEY_KPRIGHTPAREN.0],
            Keyboard::KeyF13 => vec![KeyCode::KEY_F13.0],
            Keyboard::KeyF14 => vec![KeyCode::KEY_F14.0],
            Keyboard::KeyF15 => vec![KeyCode::KEY_F15.0],
            Keyboard::KeyF16 => vec![KeyCode::KEY_F16.0],
            Keyboard::KeyF17 => vec![KeyCode::KEY_F17.0],
            Keyboard::KeyF18 => vec![KeyCode::KEY_F18.0],
            Keyboard::KeyF19 => vec![KeyCode::KEY_F19.0],
            Keyboard::KeyF20 => vec![KeyCode::KEY_F20.0],
            Keyboard::KeyF21 => vec![KeyCode::KEY_F21.0],
            Keyboard::KeyF22 => vec![KeyCode::KEY_F22.0],
            Keyboard::KeyF23 => vec![KeyCode::KEY_F23.0],
            Keyboard::KeyF24 => vec![KeyCode::KEY_F24.0],
            Keyboard::KeyProg1 => vec![KeyCode::KEY_PROG1.0],
        },
        Capability::Touchpad(touch) => match touch {
            Touchpad::LeftPad(action) => match action {
                Touch::Motion => vec![
                    AbsoluteAxisCode::ABS_MT_POSITION_X.0,
                    AbsoluteAxisCode::ABS_MT_POSITION_Y.0,
                ],
                Touch::Button(button) => match button {
                    TouchButton::Touch => vec![KeyCode::BTN_TOUCH.0],
                    TouchButton::Press => vec![KeyCode::BTN_LEFT.0],
                },
            },
            Touchpad::RightPad(action) => match action {
                Touch::Motion => vec![
                    AbsoluteAxisCode::ABS_MT_POSITION_X.0,
                    AbsoluteAxisCode::ABS_MT_POSITION_Y.0,
                ],
                Touch::Button(button) => match button {
                    TouchButton::Touch => vec![KeyCode::BTN_TOUCH.0],
                    TouchButton::Press => vec![KeyCode::BTN_LEFT.0],
                },
            },
            Touchpad::CenterPad(action) => match action {
                Touch::Motion => vec![
                    AbsoluteAxisCode::ABS_MT_POSITION_X.0,
                    AbsoluteAxisCode::ABS_MT_POSITION_Y.0,
                ],
                Touch::Button(button) => match button {
                    TouchButton::Touch => vec![KeyCode::BTN_TOUCH.0],
                    TouchButton::Press => vec![KeyCode::BTN_LEFT.0],
                },
            },
        },
    }
}

/// Returns a translated evdev input event from the given [InputValue].
fn input_event_from_value(
    event_type: EventType,
    code: u16,
    axis_info: Option<AbsInfo>,
    axis_direction: Option<AxisDirection>,
    input: InputValue,
) -> Option<InputEvent> {
    let value = match input {
        InputValue::None => None,
        InputValue::Bool(value) => {
            // Convert the binary input value into an integar
            let value = if value { 1 } else { 0 };

            // If this value is for an axis, we need to convert this value into
            // the minimum and maximum values for that axis depending on the
            // axis direction. This is typically done for DPad button input that
            // needs to be translated to an ABS_HAT axis input.
            if axis_info.is_some() && axis_direction.is_some() {
                let info = axis_info.unwrap();
                let direction = axis_direction.unwrap();
                match direction {
                    AxisDirection::None => Some(value),
                    AxisDirection::Positive => Some(info.maximum() * value),
                    AxisDirection::Negative => Some(info.minimum() * value),
                }
            } else {
                Some(value)
            }
        }
        InputValue::Float(value) => Some(denormalize_unsigned_value(value, axis_info)),
        InputValue::Vector2 { x, y } => {
            if let Some(axis_info) = axis_info {
                match AbsoluteAxisCode(code) {
                    AbsoluteAxisCode::ABS_X => denormalize_optional_signed_value(x, axis_info),
                    AbsoluteAxisCode::ABS_Y => denormalize_optional_signed_value(y, axis_info),
                    AbsoluteAxisCode::ABS_RX => denormalize_optional_signed_value(x, axis_info),
                    AbsoluteAxisCode::ABS_RY => denormalize_optional_signed_value(y, axis_info),
                    AbsoluteAxisCode::ABS_HAT0X => denormalize_optional_signed_value(x, axis_info),
                    AbsoluteAxisCode::ABS_HAT0Y => denormalize_optional_signed_value(y, axis_info),
                    AbsoluteAxisCode::ABS_HAT1X => denormalize_optional_signed_value(x, axis_info),
                    AbsoluteAxisCode::ABS_HAT1Y => denormalize_optional_signed_value(y, axis_info),
                    AbsoluteAxisCode::ABS_HAT2X => denormalize_optional_signed_value(x, axis_info),
                    AbsoluteAxisCode::ABS_HAT2Y => denormalize_optional_signed_value(y, axis_info),
                    AbsoluteAxisCode::ABS_HAT3X => denormalize_optional_signed_value(x, axis_info),
                    AbsoluteAxisCode::ABS_HAT3Y => denormalize_optional_signed_value(y, axis_info),
                    _ => todo!(),
                }
            } else {
                match event_type {
                    // Cannot denormalize value without axis info
                    EventType::ABSOLUTE => None,
                    EventType::RELATIVE => match RelativeAxisCode(code) {
                        RelativeAxisCode::REL_X => Some(x? as i32),
                        RelativeAxisCode::REL_Y => Some(y? as i32),
                        _ => None,
                    },
                    _ => None,
                }
            }
        }
        InputValue::Vector3 { x: _, y: _, z: _ } => todo!(),
        InputValue::Touch {
            index,
            is_touching: pressed,
            x,
            y,
        } => todo!(),
    };
    value?;

    Some(InputEvent::new(event_type.0, code, value.unwrap()))
}

/// Normalizes the given value from a real value with axis information into a range
/// between -1.0 - 1.0, indicating how far the axis has been pressed towards its
/// minimum and maximum values.
fn normalize_signed_value(raw_value: i32, min: i32, max: i32) -> f64 {
    let mid = (max + min) / 2;
    let event_value = raw_value - mid;

    let min = min as f64;
    let max = max as f64;
    let mid = mid as f64;
    let event_value = event_value as f64;

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
fn normalize_unsigned_value(raw_value: i32, max: i32) -> f64 {
    raw_value as f64 / max as f64
}

/// De-normalizes the given value from -1.0 - 1.0 into a real value based on the
/// minimum and maximum axis range from the given [AbsInfo].
fn denormalize_optional_signed_value(normal_value: Option<f64>, axis_info: AbsInfo) -> Option<i32> {
    normal_value?;
    let normal_value = normal_value.unwrap();
    Some(denormalize_signed_value(normal_value, axis_info))
}

/// De-normalizes the given value from -1.0 - 1.0 into a real value based on the
/// minimum and maximum axis range from the given [AbsInfo].
fn denormalize_signed_value(normal_value: f64, axis_info: AbsInfo) -> i32 {
    let mid = (axis_info.maximum() + axis_info.minimum()) / 2;
    let normal_value_abs = normal_value.abs();
    if normal_value >= 0.0 {
        let maximum = (axis_info.maximum() - mid) as f64;
        let value = normal_value * maximum + (mid as f64);
        value as i32
    } else {
        let minimum = (axis_info.minimum() - mid) as f64;
        let value = normal_value_abs * minimum + (mid as f64);
        value as i32
    }
}

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_unsigned_value(normal_value: f64, axis_info: Option<AbsInfo>) -> i32 {
    // TODO: this better
    let Some(axis_info) = axis_info else {
        return normal_value as i32;
    };
    (normal_value * axis_info.maximum() as f64).round() as i32
}

/// The AxisDirection is used to determine if a button value should be mapped
/// towards the maximum axis value or the minimum axis value. For example,
/// when mapping a BTN_UP to an ABS_HAT0Y, the converted value should be
/// positive, towards that axis's maximum value, whereas BTN_DOWN should
/// be Negative, towards that axis's minimum value.
enum AxisDirection {
    None,
    Positive,
    Negative,
}
