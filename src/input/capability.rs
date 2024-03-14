use std::{fmt, str::FromStr};

/// A capability describes what kind of input events an input device is capable
/// of emitting.
#[derive(Clone, Debug, PartialEq)]
pub enum Capability {
    /// Used to purposefully disable input capabilities
    None,
    /// Unknown or unimplemented input
    NotImplemented,
    /// Evdev syncronize event
    Sync,
    Gamepad(Gamepad),
    Mouse(Mouse),
    Keyboard(Keyboard),
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Capability::None => write!(f, "None"),
            Capability::NotImplemented => write!(f, "NotImplemented"),
            Capability::Sync => write!(f, "Sync"),
            Capability::Gamepad(_) => write!(f, "Gamepad"),
            Capability::Mouse(_) => write!(f, "Mouse"),
            Capability::Keyboard(_) => write!(f, "Keyboard"),
        }
    }
}

impl FromStr for Capability {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "None" => Ok(Capability::None),
            "NotImplemented" => Ok(Capability::NotImplemented),
            "Sync" => Ok(Capability::Sync),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Gamepad {
    /// Gamepad Buttons typically use binary input that represents button presses
    Button(GamepadButton),
    /// Gamepad Axes typically use (x, y) input that represents multi-axis input
    Axis(GamepadAxis),
    /// Gamepad Trigger typically uses a single unsigned integar value that represents
    /// how far a trigger has been pulled
    Trigger(GamepadTrigger),
    Accelerometer,
    Gyro,
}

impl fmt::Display for Gamepad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Gamepad::Button(_) => write!(f, "Button"),
            Gamepad::Axis(_) => write!(f, "Axis"),
            Gamepad::Trigger(_) => write!(f, "Trigger"),
            Gamepad::Accelerometer => write!(f, "Accelerometer"),
            Gamepad::Gyro => write!(f, "Gyro"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mouse {
    /// Represents (x, y) relative mouse motion
    Motion,
    /// Mouse Buttons are typically binary mouse input that represents button presses
    Button(MouseButton),
}

impl fmt::Display for Mouse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Mouse::Motion => write!(f, "Motion"),
            Mouse::Button(_) => write!(f, "Button"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Right mouse button
    Right,
    /// Middle mouse button
    Middle,
    /// Mouse wheel up
    WheelUp,
    /// Mouse wheen down
    WheelDown,
    /// Mouse wheel left
    WheelLeft,
    /// Mouse wheel right
    WheelRight,
    /// Extra mouse button, usually on the side of the mouse
    Extra1,
    /// Extra mouse button, usually on the side of the mouse
    Extra2,
}

impl fmt::Display for MouseButton {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MouseButton::Left => write!(f, "Left"),
            MouseButton::Right => write!(f, "Right"),
            MouseButton::Middle => write!(f, "Middle"),
            MouseButton::WheelUp => write!(f, "WheelUp"),
            MouseButton::WheelDown => write!(f, "WheelDown"),
            MouseButton::WheelLeft => write!(f, "WheelLeft"),
            MouseButton::WheelRight => write!(f, "WheelRight"),
            MouseButton::Extra1 => write!(f, "Extra1"),
            MouseButton::Extra2 => write!(f, "Extra2"),
        }
    }
}

/// Gamepad Buttons typically use binary input that represents button presses
#[derive(Clone, Debug, PartialEq)]
pub enum GamepadButton {
    /// South action, Sony Cross x, Xbox A, Nintendo B
    South,
    /// East action, Sony Circle ◯, Xbox B, Nintendo A
    East,
    /// North action, Sony Square □, Xbox X, Nintendo Y
    North,
    /// West action, Sony Triangle ∆, XBox Y, Nintendo X
    West,
    /// Start, Xbox Menu, Nintendo +, Steam Deck Hamburger Menu (☰)
    Start,
    /// Select, Sony Select, Xbox Back, Nintendo -
    Select,
    /// Guide button, Sony PS, Xbox Home, Steam Deck ⧉
    Guide,
    /// Base button, usually on the bottom right, Steam Quick Access Button (...)
    QuickAccess,
    /// Base button, usually on the bottom of the device
    QuickAccess2,
    /// Dedicated button for opening an on-screen keyboard
    Keyboard,
    /// Dedicated screenshot button
    Screenshot,
    /// Directional Pad up
    DPadUp,
    /// Directional Pad down
    DPadDown,
    /// Directional Pad left
    DPadLeft,
    /// Directional Pad right
    DPadRight,
    /// Left shoulder button, Sony L1, Xbox LB
    LeftBumper,
    /// Left top button on AyaNeo devices, inboard of left bumper
    LeftTop,
    /// Left trigger button, Deck binary sensor for left trigger
    LeftTrigger,
    /// Left back paddle button, Xbox P3, Steam Deck L4
    LeftPaddle1,
    /// Left back paddle button, Xbox P4, Steam Deck L5
    LeftPaddle2,
    /// Left back paddle button, No examples
    LeftPaddle3,
    /// Z-axis button on the left stick, Sony L3, Xbox LS
    LeftStick,
    /// Touch sensor for left stick
    LeftStickTouch,
    /// Touch binary sensor for the left touchpad
    LeftTouchpadTouch,
    /// Press binary sensor for the left touchpad
    LeftTouchpadPress,
    /// Right shoulder button, Sony R1, Xbox RB
    RightBumper,
    /// Right top button on AyaNeo devices, inboard of right bumper
    RightTop,
    /// Right trigger button, Deck binary sensor for right trigger
    RightTrigger,
    /// Right back paddle button, Xbox P1, Steam Deck R4
    RightPaddle1,
    /// Right back paddle button, Xbox P2, Steam Deck R5
    RightPaddle2,
    /// Right "side" paddle button, Legion Go M2
    RightPaddle3,
    /// Z-axis button on the right stick, Sony R3, Xbox RS
    RightStick,
    /// Touch binary sensor for right stick
    RightStickTouch,
    /// Touch binary sensor for the right touchpad
    RightTouchpadTouch,
    /// Press binary sensor for the right touchpad
    RightTouchpadPress,
}

impl fmt::Display for GamepadButton {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GamepadButton::South => write!(f, "South"),
            GamepadButton::East => write!(f, "East"),
            GamepadButton::North => write!(f, "North"),
            GamepadButton::West => write!(f, "West"),
            GamepadButton::Start => write!(f, "Start"),
            GamepadButton::Select => write!(f, "Select"),
            GamepadButton::Guide => write!(f, "Guide"),
            GamepadButton::QuickAccess => write!(f, "QuickAccess"),
            GamepadButton::QuickAccess2 => write!(f, "QuickAccess2"),
            GamepadButton::Keyboard => write!(f, "Keyboard"),
            GamepadButton::Screenshot => write!(f, "Screenshot"),
            GamepadButton::DPadUp => write!(f, "DPadUp"),
            GamepadButton::DPadDown => write!(f, "DPadDown"),
            GamepadButton::DPadLeft => write!(f, "DPadLeft"),
            GamepadButton::DPadRight => write!(f, "DPadRight"),
            GamepadButton::LeftBumper => write!(f, "LeftBumper"),
            GamepadButton::LeftTop => write!(f, "LeftTop"),
            GamepadButton::LeftTrigger => write!(f, "LeftTrigger"),
            GamepadButton::LeftPaddle1 => write!(f, "LeftPaddle1"),
            GamepadButton::LeftPaddle2 => write!(f, "LeftPaddle2"),
            GamepadButton::LeftPaddle3 => write!(f, "LeftPaddle3"),
            GamepadButton::LeftStick => write!(f, "LeftStick"),
            GamepadButton::LeftStickTouch => write!(f, "LeftStickTouch"),
            GamepadButton::LeftTouchpadTouch => write!(f, "LeftTouchpadTouch"),
            GamepadButton::LeftTouchpadPress => write!(f, "LeftTouchpadPress"),
            GamepadButton::RightBumper => write!(f, "RightBumper"),
            GamepadButton::RightTop => write!(f, "RightTop"),
            GamepadButton::RightTrigger => write!(f, "RightTrigger"),
            GamepadButton::RightPaddle1 => write!(f, "RightPaddle1"),
            GamepadButton::RightPaddle2 => write!(f, "RightPaddle2"),
            GamepadButton::RightPaddle3 => write!(f, "RightPaddle3"),
            GamepadButton::RightStick => write!(f, "RightStick"),
            GamepadButton::RightStickTouch => write!(f, "RightStickTouch"),
            GamepadButton::RightTouchpadTouch => write!(f, "RightTouchpadTouch"),
            GamepadButton::RightTouchpadPress => write!(f, "RightTouchpadPress"),
        }
    }
}

impl FromStr for GamepadButton {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "South" => Ok(GamepadButton::South),
            "East" => Ok(GamepadButton::East),
            "North" => Ok(GamepadButton::North),
            "West" => Ok(GamepadButton::West),
            "Start" => Ok(GamepadButton::Start),
            "Select" => Ok(GamepadButton::Select),
            "Guide" => Ok(GamepadButton::Guide),
            "QuickAccess" => Ok(GamepadButton::QuickAccess),
            "QuickAccess2" => Ok(GamepadButton::QuickAccess2),
            "Keyboard" => Ok(GamepadButton::Keyboard),
            "Screenshot" => Ok(GamepadButton::Screenshot),
            "DPadUp" => Ok(GamepadButton::DPadUp),
            "DPadDown" => Ok(GamepadButton::DPadDown),
            "DPadLeft" => Ok(GamepadButton::DPadLeft),
            "DPadRight" => Ok(GamepadButton::DPadRight),
            "LeftBumper" => Ok(GamepadButton::LeftBumper),
            "LeftTop" => Ok(GamepadButton::LeftTop),
            "LeftTrigger" => Ok(GamepadButton::LeftTrigger),
            "LeftPaddle1" => Ok(GamepadButton::LeftPaddle1),
            "LeftPaddle2" => Ok(GamepadButton::LeftPaddle2),
            "LeftPaddle3" => Ok(GamepadButton::LeftPaddle3),
            "LeftStick" => Ok(GamepadButton::LeftStick),
            "LeftStickTouch" => Ok(GamepadButton::LeftStickTouch),
            "LeftTouchpadTouch" => Ok(GamepadButton::LeftTouchpadTouch),
            "LeftTouchpadPress" => Ok(GamepadButton::LeftTouchpadPress),
            "RightBumper" => Ok(GamepadButton::RightBumper),
            "RightTop" => Ok(GamepadButton::RightTop),
            "RightTrigger" => Ok(GamepadButton::RightTrigger),
            "RightPaddle1" => Ok(GamepadButton::RightPaddle1),
            "RightPaddle2" => Ok(GamepadButton::RightPaddle2),
            "RightPaddle3" => Ok(GamepadButton::RightPaddle3),
            "RightStick" => Ok(GamepadButton::RightStick),
            "RightStickTouch" => Ok(GamepadButton::RightStickTouch),
            "RightTouchpadTouch" => Ok(GamepadButton::RightTouchpadTouch),
            "RightTouchpadPress" => Ok(GamepadButton::RightTouchpadPress),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GamepadAxis {
    LeftStick,
    RightStick,
    Hat1,
    Hat2,
    Hat3,
    /// Axis input from two binary button inputs
    Buttons(GamepadButton, GamepadButton),
}

impl fmt::Display for GamepadAxis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GamepadAxis::LeftStick => write!(f, "LeftStick"),
            GamepadAxis::RightStick => write!(f, "RightStick"),
            GamepadAxis::Hat1 => write!(f, "Hat1"),
            GamepadAxis::Hat2 => write!(f, "Hat2"),
            GamepadAxis::Hat3 => write!(f, "Hat3"),
            GamepadAxis::Buttons(_, _) => write!(f, "Buttons"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GamepadTrigger {
    LeftTrigger,
    LeftTouchpadForce,
    LeftStickForce,
    RightTrigger,
    RightTouchpadForce,
    RightStickForce,
}

impl fmt::Display for GamepadTrigger {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GamepadTrigger::LeftTrigger => write!(f, "LeftTrigger"),
            GamepadTrigger::LeftTouchpadForce => write!(f, "LeftTouchpadForce"),
            GamepadTrigger::LeftStickForce => write!(f, "LeftStickForce"),
            GamepadTrigger::RightTrigger => write!(f, "RightTrigger"),
            GamepadTrigger::RightTouchpadForce => write!(f, "RightTouchpadForce"),
            GamepadTrigger::RightStickForce => write!(f, "RightStickForce"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Keyboard {
    KeyEsc,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Key0,
    KeyMinus,
    KeyEqual,
    KeyBackspace,
    KeyTab,
    KeyQ,
    KeyW,
    KeyE,
    KeyR,
    KeyT,
    KeyY,
    KeyU,
    KeyI,
    KeyO,
    KeyP,
    KeyLeftBrace,
    KeyRightBrace,
    KeyEnter,
    KeyLeftCtrl,
    KeyA,
    KeyS,
    KeyD,
    KeyF,
    KeyG,
    KeyH,
    KeyJ,
    KeyK,
    KeyL,
    KeySemicolon,
    KeyApostrophe,
    KeyGrave,
    KeyLeftShift,
    KeyBackslash,
    KeyZ,
    KeyX,
    KeyC,
    KeyV,
    KeyB,
    KeyN,
    KeyM,
    KeyComma,
    KeyDot,
    KeySlash,
    KeyRightShift,
    KeyKpAsterisk,
    KeyLeftAlt,
    KeySpace,
    KeyCapslock,
    KeyF1,
    KeyF2,
    KeyF3,
    KeyF4,
    KeyF5,
    KeyF6,
    KeyF7,
    KeyF8,
    KeyF9,
    KeyF10,
    KeyNumlock,
    KeyScrollLock,
    KeyKp7,
    KeyKp8,
    KeyKp9,
    KeyKpMinus,
    KeyKp4,
    KeyKp5,
    KeyKp6,
    KeyKpPlus,
    KeyKp1,
    KeyKp2,
    KeyKp3,
    KeyKp0,
    KeyKpDot,
    KeyZenkakuhankaku,
    Key102nd,
    KeyF11,
    KeyF12,
    KeyRo,
    KeyKatakana,
    KeyHiragana,
    KeyHenkan,
    KeyKatakanaHiragana,
    KeyMuhenkan,
    KeyKpJpComma,
    KeyKpEnter,
    KeyRightCtrl,
    KeyKpSlash,
    KeySysrq,
    KeyRightAlt,
    KeyHome,
    KeyUp,
    KeyPageUp,
    KeyLeft,
    KeyRight,
    KeyEnd,
    KeyDown,
    KeyPageDown,
    KeyInsert,
    KeyDelete,
    KeyMute,
    KeyVolumeDown,
    KeyVolumeUp,
    KeyPower,
    KeyKpEqual,
    KeyPause,
    KeyKpComma,
    KeyHanja,
    KeyYen,
    KeyLeftMeta,
    KeyRightMeta,
    KeyCompose,
    KeyStop,
    KeyAgain,
    KeyProps,
    KeyUndo,
    KeyFront,
    KeyCopy,
    KeyOpen,
    KeyPaste,
    KeyFind,
    KeyCut,
    KeyHelp,
    KeyCalc,
    KeySleep,
    KeyWww,
    KeyBack,
    KeyForward,
    KeyEjectCD,
    KeyNextSong,
    KeyPlayPause,
    KeyPreviousSong,
    KeyStopCD,
    KeyRefresh,
    KeyEdit,
    KeyScrollUp,
    KeyScrollDown,
    KeyKpLeftParen,
    KeyKpRightParen,
    KeyF13,
    KeyF14,
    KeyF15,
    KeyF16,
    KeyF17,
    KeyF18,
    KeyF19,
    KeyF20,
    KeyF21,
    KeyF22,
    KeyF23,
    KeyF24,
    KeyProg1,
}

impl fmt::Display for Keyboard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Keyboard::KeyEsc => write!(f, "KeyEsc"),
            Keyboard::Key1 => write!(f, "Key1"),
            Keyboard::Key2 => write!(f, "Key2"),
            Keyboard::Key3 => write!(f, "Key3"),
            Keyboard::Key4 => write!(f, "Key4"),
            Keyboard::Key5 => write!(f, "Key5"),
            Keyboard::Key6 => write!(f, "Key6"),
            Keyboard::Key7 => write!(f, "Key7"),
            Keyboard::Key8 => write!(f, "Key8"),
            Keyboard::Key9 => write!(f, "Key9"),
            Keyboard::Key0 => write!(f, "Key0"),
            Keyboard::KeyMinus => write!(f, "KeyMinus"),
            Keyboard::KeyEqual => write!(f, "KeyEqual"),
            Keyboard::KeyBackspace => write!(f, "KeyBackspace"),
            Keyboard::KeyTab => write!(f, "KeyTab"),
            Keyboard::KeyQ => write!(f, "KeyQ"),
            Keyboard::KeyW => write!(f, "KeyW"),
            Keyboard::KeyE => write!(f, "KeyE"),
            Keyboard::KeyR => write!(f, "KeyR"),
            Keyboard::KeyT => write!(f, "KeyT"),
            Keyboard::KeyY => write!(f, "KeyY"),
            Keyboard::KeyU => write!(f, "KeyU"),
            Keyboard::KeyI => write!(f, "KeyI"),
            Keyboard::KeyO => write!(f, "KeyO"),
            Keyboard::KeyP => write!(f, "KeyP"),
            Keyboard::KeyLeftBrace => write!(f, "KeyLeftBrace"),
            Keyboard::KeyRightBrace => write!(f, "KeyRightBrace"),
            Keyboard::KeyEnter => write!(f, "KeyEnter"),
            Keyboard::KeyLeftCtrl => write!(f, "KeyLeftCtrl"),
            Keyboard::KeyA => write!(f, "KeyA"),
            Keyboard::KeyS => write!(f, "KeyS"),
            Keyboard::KeyD => write!(f, "KeyD"),
            Keyboard::KeyF => write!(f, "KeyF"),
            Keyboard::KeyG => write!(f, "KeyG"),
            Keyboard::KeyH => write!(f, "KeyH"),
            Keyboard::KeyJ => write!(f, "KeyJ"),
            Keyboard::KeyK => write!(f, "KeyK"),
            Keyboard::KeyL => write!(f, "KeyL"),
            Keyboard::KeySemicolon => write!(f, "KeySemicolon"),
            Keyboard::KeyApostrophe => write!(f, "KeyApostrophe"),
            Keyboard::KeyGrave => write!(f, "KeyGrave"),
            Keyboard::KeyLeftShift => write!(f, "KeyLeftShift"),
            Keyboard::KeyBackslash => write!(f, "KeyBackslash"),
            Keyboard::KeyZ => write!(f, "KeyZ"),
            Keyboard::KeyX => write!(f, "KeyX"),
            Keyboard::KeyC => write!(f, "KeyC"),
            Keyboard::KeyV => write!(f, "KeyV"),
            Keyboard::KeyB => write!(f, "KeyB"),
            Keyboard::KeyN => write!(f, "KeyN"),
            Keyboard::KeyM => write!(f, "KeyM"),
            Keyboard::KeyComma => write!(f, "KeyComma"),
            Keyboard::KeyDot => write!(f, "KeyDot"),
            Keyboard::KeySlash => write!(f, "KeySlash"),
            Keyboard::KeyRightShift => write!(f, "KeyRightShift"),
            Keyboard::KeyKpAsterisk => write!(f, "KeyKpAsterisk"),
            Keyboard::KeyLeftAlt => write!(f, "KeyLeftAlt"),
            Keyboard::KeySpace => write!(f, "KeySpace"),
            Keyboard::KeyCapslock => write!(f, "KeyCapslock"),
            Keyboard::KeyF1 => write!(f, "KeyF1"),
            Keyboard::KeyF2 => write!(f, "KeyF2"),
            Keyboard::KeyF3 => write!(f, "KeyF3"),
            Keyboard::KeyF4 => write!(f, "KeyF4"),
            Keyboard::KeyF5 => write!(f, "KeyF5"),
            Keyboard::KeyF6 => write!(f, "KeyF6"),
            Keyboard::KeyF7 => write!(f, "KeyF7"),
            Keyboard::KeyF8 => write!(f, "KeyF8"),
            Keyboard::KeyF9 => write!(f, "KeyF9"),
            Keyboard::KeyF10 => write!(f, "KeyF10"),
            Keyboard::KeyNumlock => write!(f, "KeyNumlock"),
            Keyboard::KeyScrollLock => write!(f, "KeyScrollLock"),
            Keyboard::KeyKp7 => write!(f, "KeyKp7"),
            Keyboard::KeyKp8 => write!(f, "KeyKp8"),
            Keyboard::KeyKp9 => write!(f, "KeyKp9"),
            Keyboard::KeyKpMinus => write!(f, "KeyKpMinus"),
            Keyboard::KeyKp4 => write!(f, "KeyKp4"),
            Keyboard::KeyKp5 => write!(f, "KeyKp5"),
            Keyboard::KeyKp6 => write!(f, "KeyKp6"),
            Keyboard::KeyKpPlus => write!(f, "KeyKpPlus"),
            Keyboard::KeyKp1 => write!(f, "KeyKp1"),
            Keyboard::KeyKp2 => write!(f, "KeyKp2"),
            Keyboard::KeyKp3 => write!(f, "KeyKp3"),
            Keyboard::KeyKp0 => write!(f, "KeyKp0"),
            Keyboard::KeyKpDot => write!(f, "KeyKpDot"),
            Keyboard::KeyZenkakuhankaku => write!(f, "KeyZenkakuhankaku"),
            Keyboard::Key102nd => write!(f, "Key102nd"),
            Keyboard::KeyF11 => write!(f, "KeyF11"),
            Keyboard::KeyF12 => write!(f, "KeyF12"),
            Keyboard::KeyRo => write!(f, "KeyRo"),
            Keyboard::KeyKatakana => write!(f, "KeyKatakana"),
            Keyboard::KeyHiragana => write!(f, "KeyHiragana"),
            Keyboard::KeyHenkan => write!(f, "KeyHenkan"),
            Keyboard::KeyKatakanaHiragana => write!(f, "KeyKatakanaHiragana"),
            Keyboard::KeyMuhenkan => write!(f, "KeyMuhenkan"),
            Keyboard::KeyKpJpComma => write!(f, "KeyKpJpComma"),
            Keyboard::KeyKpEnter => write!(f, "KeyKpEnter"),
            Keyboard::KeyRightCtrl => write!(f, "KeyRightCtrl"),
            Keyboard::KeyKpSlash => write!(f, "KeyKpSlash"),
            Keyboard::KeySysrq => write!(f, "KeySysrq"),
            Keyboard::KeyRightAlt => write!(f, "KeyRightAlt"),
            Keyboard::KeyHome => write!(f, "KeyHome"),
            Keyboard::KeyUp => write!(f, "KeyUp"),
            Keyboard::KeyPageUp => write!(f, "KeyPageUp"),
            Keyboard::KeyLeft => write!(f, "KeyLeft"),
            Keyboard::KeyRight => write!(f, "KeyRight"),
            Keyboard::KeyEnd => write!(f, "KeyEnd"),
            Keyboard::KeyDown => write!(f, "KeyDown"),
            Keyboard::KeyPageDown => write!(f, "KeyPageDown"),
            Keyboard::KeyInsert => write!(f, "KeyInsert"),
            Keyboard::KeyDelete => write!(f, "KeyDelete"),
            Keyboard::KeyMute => write!(f, "KeyMute"),
            Keyboard::KeyVolumeDown => write!(f, "KeyVolumeDown"),
            Keyboard::KeyVolumeUp => write!(f, "KeyVolumeUp"),
            Keyboard::KeyPower => write!(f, "KeyPower"),
            Keyboard::KeyKpEqual => write!(f, "KeyKpEqual"),
            Keyboard::KeyPause => write!(f, "KeyPause"),
            Keyboard::KeyKpComma => write!(f, "KeyKpComma"),
            Keyboard::KeyHanja => write!(f, "KeyHanja"),
            Keyboard::KeyYen => write!(f, "KeyYen"),
            Keyboard::KeyLeftMeta => write!(f, "KeyLeftMeta"),
            Keyboard::KeyRightMeta => write!(f, "KeyRightMeta"),
            Keyboard::KeyCompose => write!(f, "KeyCompose"),
            Keyboard::KeyStop => write!(f, "KeyStop"),
            Keyboard::KeyAgain => write!(f, "KeyAgain"),
            Keyboard::KeyProps => write!(f, "KeyProps"),
            Keyboard::KeyUndo => write!(f, "KeyUndo"),
            Keyboard::KeyFront => write!(f, "KeyFront"),
            Keyboard::KeyCopy => write!(f, "KeyCopy"),
            Keyboard::KeyOpen => write!(f, "KeyOpen"),
            Keyboard::KeyPaste => write!(f, "KeyPaste"),
            Keyboard::KeyFind => write!(f, "KeyFind"),
            Keyboard::KeyCut => write!(f, "KeyCut"),
            Keyboard::KeyHelp => write!(f, "KeyHelp"),
            Keyboard::KeyCalc => write!(f, "KeyCalc"),
            Keyboard::KeySleep => write!(f, "KeySleep"),
            Keyboard::KeyWww => write!(f, "KeyWww"),
            Keyboard::KeyBack => write!(f, "KeyBack"),
            Keyboard::KeyForward => write!(f, "KeyForward"),
            Keyboard::KeyEjectCD => write!(f, "KeyEjectCD"),
            Keyboard::KeyNextSong => write!(f, "KeyNextSong"),
            Keyboard::KeyPlayPause => write!(f, "KeyPlayPause"),
            Keyboard::KeyPreviousSong => write!(f, "KeyPreviousSong"),
            Keyboard::KeyStopCD => write!(f, "KeyStopCD"),
            Keyboard::KeyRefresh => write!(f, "KeyRefresh"),
            Keyboard::KeyEdit => write!(f, "KeyEdit"),
            Keyboard::KeyScrollUp => write!(f, "KeyScrollUp"),
            Keyboard::KeyScrollDown => write!(f, "KeyScrollDown"),
            Keyboard::KeyKpLeftParen => write!(f, "KeyKpLeftParen"),
            Keyboard::KeyKpRightParen => write!(f, "KeyKpRightParen"),
            Keyboard::KeyF13 => write!(f, "KeyF13"),
            Keyboard::KeyF14 => write!(f, "KeyF14"),
            Keyboard::KeyF15 => write!(f, "KeyF15"),
            Keyboard::KeyF16 => write!(f, "KeyF16"),
            Keyboard::KeyF17 => write!(f, "KeyF17"),
            Keyboard::KeyF18 => write!(f, "KeyF18"),
            Keyboard::KeyF19 => write!(f, "KeyF19"),
            Keyboard::KeyF20 => write!(f, "KeyF20"),
            Keyboard::KeyF21 => write!(f, "KeyF21"),
            Keyboard::KeyF22 => write!(f, "KeyF22"),
            Keyboard::KeyF23 => write!(f, "KeyF23"),
            Keyboard::KeyF24 => write!(f, "KeyF24"),
            Keyboard::KeyProg1 => write!(f, "KeyProg1"),
        }
    }
}

impl FromStr for Keyboard {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "KeyEsc" => Ok(Keyboard::KeyEsc),
            "Key1" => Ok(Keyboard::Key1),
            "Key2" => Ok(Keyboard::Key2),
            "Key3" => Ok(Keyboard::Key3),
            "Key4" => Ok(Keyboard::Key4),
            "Key5" => Ok(Keyboard::Key5),
            "Key6" => Ok(Keyboard::Key6),
            "Key7" => Ok(Keyboard::Key7),
            "Key8" => Ok(Keyboard::Key8),
            "Key9" => Ok(Keyboard::Key9),
            "Key0" => Ok(Keyboard::Key0),
            "KeyMinus" => Ok(Keyboard::KeyMinus),
            "KeyEqual" => Ok(Keyboard::KeyEqual),
            "KeyBackspace" => Ok(Keyboard::KeyBackspace),
            "KeyTab" => Ok(Keyboard::KeyTab),
            "KeyQ" => Ok(Keyboard::KeyQ),
            "KeyW" => Ok(Keyboard::KeyW),
            "KeyE" => Ok(Keyboard::KeyE),
            "KeyR" => Ok(Keyboard::KeyR),
            "KeyT" => Ok(Keyboard::KeyT),
            "KeyY" => Ok(Keyboard::KeyY),
            "KeyU" => Ok(Keyboard::KeyU),
            "KeyI" => Ok(Keyboard::KeyI),
            "KeyO" => Ok(Keyboard::KeyO),
            "KeyP" => Ok(Keyboard::KeyP),
            "KeyLeftBrace" => Ok(Keyboard::KeyLeftBrace),
            "KeyRightBrace" => Ok(Keyboard::KeyRightBrace),
            "KeyEnter" => Ok(Keyboard::KeyEnter),
            "KeyLeftCtrl" => Ok(Keyboard::KeyLeftCtrl),
            "KeyA" => Ok(Keyboard::KeyA),
            "KeyS" => Ok(Keyboard::KeyS),
            "KeyD" => Ok(Keyboard::KeyD),
            "KeyF" => Ok(Keyboard::KeyF),
            "KeyG" => Ok(Keyboard::KeyG),
            "KeyH" => Ok(Keyboard::KeyH),
            "KeyJ" => Ok(Keyboard::KeyJ),
            "KeyK" => Ok(Keyboard::KeyK),
            "KeyL" => Ok(Keyboard::KeyL),
            "KeySemicolon" => Ok(Keyboard::KeySemicolon),
            "KeyApostrophe" => Ok(Keyboard::KeyApostrophe),
            "KeyGrave" => Ok(Keyboard::KeyGrave),
            "KeyLeftShift" => Ok(Keyboard::KeyLeftShift),
            "KeyBackslash" => Ok(Keyboard::KeyBackslash),
            "KeyZ" => Ok(Keyboard::KeyZ),
            "KeyX" => Ok(Keyboard::KeyX),
            "KeyC" => Ok(Keyboard::KeyC),
            "KeyV" => Ok(Keyboard::KeyV),
            "KeyB" => Ok(Keyboard::KeyB),
            "KeyN" => Ok(Keyboard::KeyN),
            "KeyM" => Ok(Keyboard::KeyM),
            "KeyComma" => Ok(Keyboard::KeyComma),
            "KeyDot" => Ok(Keyboard::KeyDot),
            "KeySlash" => Ok(Keyboard::KeySlash),
            "KeyRightShift" => Ok(Keyboard::KeyRightShift),
            "KeyKpAsterisk" => Ok(Keyboard::KeyKpAsterisk),
            "KeyLeftAlt" => Ok(Keyboard::KeyLeftAlt),
            "KeySpace" => Ok(Keyboard::KeySpace),
            "KeyCapslock" => Ok(Keyboard::KeyCapslock),
            "KeyF1" => Ok(Keyboard::KeyF1),
            "KeyF2" => Ok(Keyboard::KeyF2),
            "KeyF3" => Ok(Keyboard::KeyF3),
            "KeyF4" => Ok(Keyboard::KeyF4),
            "KeyF5" => Ok(Keyboard::KeyF5),
            "KeyF6" => Ok(Keyboard::KeyF6),
            "KeyF7" => Ok(Keyboard::KeyF7),
            "KeyF8" => Ok(Keyboard::KeyF8),
            "KeyF9" => Ok(Keyboard::KeyF9),
            "KeyF10" => Ok(Keyboard::KeyF10),
            "KeyNumlock" => Ok(Keyboard::KeyNumlock),
            "KeyScrollLock" => Ok(Keyboard::KeyScrollLock),
            "KeyKp7" => Ok(Keyboard::KeyKp7),
            "KeyKp8" => Ok(Keyboard::KeyKp8),
            "KeyKp9" => Ok(Keyboard::KeyKp9),
            "KeyKpMinus" => Ok(Keyboard::KeyKpMinus),
            "KeyKp4" => Ok(Keyboard::KeyKp4),
            "KeyKp5" => Ok(Keyboard::KeyKp5),
            "KeyKp6" => Ok(Keyboard::KeyKp6),
            "KeyKpPlus" => Ok(Keyboard::KeyKpPlus),
            "KeyKp1" => Ok(Keyboard::KeyKp1),
            "KeyKp2" => Ok(Keyboard::KeyKp2),
            "KeyKp3" => Ok(Keyboard::KeyKp3),
            "KeyKp0" => Ok(Keyboard::KeyKp0),
            "KeyKpdot" => Ok(Keyboard::KeyKpDot),
            "KeyZenkakuhankaku" => Ok(Keyboard::KeyZenkakuhankaku),
            "Key102nd" => Ok(Keyboard::Key102nd),
            "KeyF11" => Ok(Keyboard::KeyF11),
            "KeyF12" => Ok(Keyboard::KeyF12),
            "KeyRo" => Ok(Keyboard::KeyRo),
            "KeyKatakana" => Ok(Keyboard::KeyKatakana),
            "KeyHiragana" => Ok(Keyboard::KeyHiragana),
            "KeyHenkan" => Ok(Keyboard::KeyHenkan),
            "KeyKatakanaHiragana" => Ok(Keyboard::KeyKatakanaHiragana),
            "KeyMuhenkan" => Ok(Keyboard::KeyMuhenkan),
            "KeyKpJpComma" => Ok(Keyboard::KeyKpJpComma),
            "KeyKpEnter" => Ok(Keyboard::KeyKpEnter),
            "KeyRightCtrl" => Ok(Keyboard::KeyRightCtrl),
            "KeyKpSlash" => Ok(Keyboard::KeyKpSlash),
            "KeySysrq" => Ok(Keyboard::KeySysrq),
            "KeyRightAlt" => Ok(Keyboard::KeyRightAlt),
            "KeyHome" => Ok(Keyboard::KeyHome),
            "KeyUp" => Ok(Keyboard::KeyUp),
            "KeyPageUp" => Ok(Keyboard::KeyPageUp),
            "KeyLeft" => Ok(Keyboard::KeyLeft),
            "KeyRight" => Ok(Keyboard::KeyRight),
            "KeyEnd" => Ok(Keyboard::KeyEnd),
            "KeyDown" => Ok(Keyboard::KeyDown),
            "KeyPageDown" => Ok(Keyboard::KeyPageDown),
            "KeyInsert" => Ok(Keyboard::KeyInsert),
            "KeyDelete" => Ok(Keyboard::KeyDelete),
            "KeyMute" => Ok(Keyboard::KeyMute),
            "KeyVolumeDown" => Ok(Keyboard::KeyVolumeDown),
            "KeyVolumeUp" => Ok(Keyboard::KeyVolumeUp),
            "KeyPower" => Ok(Keyboard::KeyPower),
            "KeyKpEqual" => Ok(Keyboard::KeyKpEqual),
            "KeyPause" => Ok(Keyboard::KeyPause),
            "KeyKpComma" => Ok(Keyboard::KeyKpComma),
            "KeyHanja" => Ok(Keyboard::KeyHanja),
            "KeyYen" => Ok(Keyboard::KeyYen),
            "KeyLeftMeta" => Ok(Keyboard::KeyLeftMeta),
            "KeyRightMeta" => Ok(Keyboard::KeyRightMeta),
            "KeyCompose" => Ok(Keyboard::KeyCompose),
            "KeyStop" => Ok(Keyboard::KeyStop),
            "KeyAgain" => Ok(Keyboard::KeyAgain),
            "KeyProps" => Ok(Keyboard::KeyProps),
            "KeyUndo" => Ok(Keyboard::KeyUndo),
            "KeyFront" => Ok(Keyboard::KeyFront),
            "KeyCopy" => Ok(Keyboard::KeyCopy),
            "KeyOpen" => Ok(Keyboard::KeyOpen),
            "KeyPaste" => Ok(Keyboard::KeyPaste),
            "KeyFind" => Ok(Keyboard::KeyFind),
            "KeyCut" => Ok(Keyboard::KeyCut),
            "KeyHelp" => Ok(Keyboard::KeyHelp),
            "KeyCalc" => Ok(Keyboard::KeyCalc),
            "KeySleep" => Ok(Keyboard::KeySleep),
            "KeyWww" => Ok(Keyboard::KeyWww),
            "KeyBack" => Ok(Keyboard::KeyBack),
            "KeyForward" => Ok(Keyboard::KeyForward),
            "KeyEjectCD" => Ok(Keyboard::KeyEjectCD),
            "KeyNextSong" => Ok(Keyboard::KeyNextSong),
            "KeyPlayPause" => Ok(Keyboard::KeyPlayPause),
            "KeyPreviousSong" => Ok(Keyboard::KeyPreviousSong),
            "KeyStopCD" => Ok(Keyboard::KeyStopCD),
            "KeyRefresh" => Ok(Keyboard::KeyRefresh),
            "KeyEdit" => Ok(Keyboard::KeyEdit),
            "KeyScrollUp" => Ok(Keyboard::KeyScrollUp),
            "KeyScrollDown" => Ok(Keyboard::KeyScrollDown),
            "KeyKpLeftParen" => Ok(Keyboard::KeyKpLeftParen),
            "KeyKpRightParen" => Ok(Keyboard::KeyKpRightParen),
            "KeyF13" => Ok(Keyboard::KeyF13),
            "KeyF14" => Ok(Keyboard::KeyF14),
            "KeyF15" => Ok(Keyboard::KeyF15),
            "KeyF16" => Ok(Keyboard::KeyF16),
            "KeyF17" => Ok(Keyboard::KeyF17),
            "KeyF18" => Ok(Keyboard::KeyF18),
            "KeyF19" => Ok(Keyboard::KeyF19),
            "KeyF20" => Ok(Keyboard::KeyF20),
            "KeyF21" => Ok(Keyboard::KeyF21),
            "KeyF22" => Ok(Keyboard::KeyF22),
            "KeyF23" => Ok(Keyboard::KeyF23),
            "KeyF24" => Ok(Keyboard::KeyF24),
            "KeyProg1" => Ok(Keyboard::KeyProg1),
            _ => Err(()),
        }
    }
}
