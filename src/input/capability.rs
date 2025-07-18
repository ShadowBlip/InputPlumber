use std::{fmt, str::FromStr};

use crate::config::capability_map::CapabilityConfig;

use super::event::dbus::Action;

/// A capability describes what kind of input events an input device is capable
/// of emitting.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Used to purposefully disable input capabilities
    None,
    /// Unknown or unimplemented input
    NotImplemented,
    /// Evdev syncronize event
    Sync,
    /// DBus is only implemented by DBus target devices
    DBus(Action),
    Gamepad(Gamepad),
    Mouse(Mouse),
    Keyboard(Keyboard),
    Touchpad(Touchpad),
    Touchscreen(Touch),
    InputLayer(InputLayer),
}

impl Capability {
    /// Helper function to determine if an event mapping output requires emulating
    /// a momentary press. This is required in such cases as relative->button
    /// mappings of similar. In the case of the Zotac Zone dials for example these
    /// emit only a `1` or a `-1` as do other devices like mouse wheels.
    pub fn is_momentary_translation(&self, target: &Capability) -> bool {
        if let Capability::Gamepad(Gamepad::Dial(_)) = self {
            matches!(
                target,
                Capability::Gamepad(Gamepad::Button(_))
                    | Capability::Mouse(Mouse::Button(_))
                    | Capability::Keyboard(_)
            )
        } else {
            false
        }
    }
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
            Capability::DBus(_) => write!(f, "DBus"),
            Capability::Touchpad(_) => write!(f, "Touchpad"),
            Capability::Touchscreen(_) => write!(f, "Touchscreen"),
            Capability::InputLayer(_) => write!(f, "InputLayer"),
        }
    }
}

impl FromStr for Capability {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        let Some((part, parts)) = parts.split_first() else {
            return Err(());
        };
        match *part {
            "None" => Ok(Capability::None),
            "NotImplemented" => Ok(Capability::NotImplemented),
            "Sync" => Ok(Capability::Sync),
            "Gamepad" => Ok(Capability::Gamepad(Gamepad::from_str(
                parts.join(":").as_str(),
            )?)),
            "Keyboard" => Ok(Capability::Keyboard(Keyboard::from_str(
                parts.join(":").as_str(),
            )?)),
            "Mouse" => Ok(Capability::Mouse(Mouse::from_str(
                parts.join(":").as_str(),
            )?)),
            "DBus" => Ok(Capability::DBus(Action::from_str(
                parts.join(":").as_str(),
            )?)),
            "Touchpad" => Ok(Capability::Touchpad(Touchpad::from_str(
                parts.join(":").as_str(),
            )?)),
            "Touchscreen" => Ok(Capability::Touchscreen(Touch::from_str(
                parts.join(":").as_str(),
            )?)),
            "InputLayer" => Ok(Capability::InputLayer(InputLayer::from_str(
                parts.join(":").as_str(),
            )?)),
            _ => Err(()),
        }
    }
}

impl From<CapabilityConfig> for Capability {
    fn from(value: CapabilityConfig) -> Self {
        // Gamepad
        if let Some(gamepad) = value.gamepad.as_ref() {
            // Axis
            if let Some(axis_config) = gamepad.axis.as_ref() {
                let axis = GamepadAxis::from_str(&axis_config.name);
                if axis.is_err() {
                    log::error!("Invalid or unimplemented axis: {}", axis_config.name);
                    return Capability::NotImplemented;
                }
                let axis = axis.unwrap();
                return Capability::Gamepad(Gamepad::Axis(axis));
            }

            // Button
            if let Some(button_string) = gamepad.button.clone() {
                let button = GamepadButton::from_str(&button_string);
                if button.is_err() {
                    log::error!("Invalid or unimplemented button: {button_string}");
                    return Capability::NotImplemented;
                }
                let button = button.unwrap();
                return Capability::Gamepad(Gamepad::Button(button));
            }

            // Trigger
            if let Some(trigger_capability) = gamepad.trigger.as_ref() {
                let trigger = GamepadTrigger::from_str(&trigger_capability.name);
                if trigger.is_err() {
                    log::error!(
                        "Invalid or unimplemented trigger: {}",
                        trigger_capability.name
                    );
                    return Capability::NotImplemented;
                }

                let trigger = trigger.unwrap();
                return Capability::Gamepad(Gamepad::Trigger(trigger));
            }

            // Gyro
            if let Some(gyro_capability) = gamepad.gyro.as_ref() {
                let gyro = Gamepad::from_str(&gyro_capability.name);
                if gyro.is_err() {
                    log::error!("Invalid or unimplemented gyro: {}", gyro_capability.name);
                    return Capability::NotImplemented;
                }

                return Capability::Gamepad(Gamepad::Gyro);
            }

            // Accelerometer
            if let Some(accelerometer_capability) = gamepad.accelerometer.as_ref() {
                let accelerometer = Gamepad::from_str(&accelerometer_capability.name);
                if accelerometer.is_err() {
                    log::error!(
                        "Invalid or unimplemented gyro: {}",
                        accelerometer_capability.name
                    );
                    return Capability::NotImplemented;
                }

                return Capability::Gamepad(Gamepad::Accelerometer);
            }

            // Dials/wheels
            if let Some(dial_config) = gamepad.dial.as_ref() {
                let dial = GamepadDial::from_str(&dial_config.name);
                if dial.is_err() {
                    log::error!("Invalid or unimplemented dial: {}", dial_config.name);
                    return Capability::NotImplemented;
                }
                let dial = dial.unwrap();
                return Capability::Gamepad(Gamepad::Dial(dial));
            }
        }

        // Keyboard
        if let Some(keyboard_string) = value.keyboard.as_ref() {
            let key = Keyboard::from_str(keyboard_string.as_str());
            if key.is_err() {
                log::error!("Invalid keyboard string: {keyboard_string}");
                return Capability::NotImplemented;
            }
            let key = key.unwrap();
            return Capability::Keyboard(key);
        }

        // Mouse
        if let Some(mouse) = value.mouse.as_ref() {
            // Motion
            if mouse.motion.is_some() {
                return Capability::Mouse(Mouse::Motion);
            }

            // Button
            if let Some(button_string) = mouse.button.as_ref() {
                let button = MouseButton::from_str(button_string);
                if button.is_err() {
                    log::error!("Invalid or unimplemented button: {button_string}");
                    return Capability::NotImplemented;
                }
                let button = button.unwrap();
                return Capability::Mouse(Mouse::Button(button));
            }
        }

        // DBus
        if let Some(action_string) = value.dbus.as_ref() {
            let action = Action::from_str(action_string);
            if action.is_err() {
                log::error!("Invalid or unimplemented dbus action: {action_string}");
                return Capability::NotImplemented;
            }
            let action = action.unwrap();
            return Capability::DBus(action);
        }

        // Touchpad
        if let Some(touchpad) = value.touchpad.as_ref() {
            let touch = {
                if touchpad.touch.motion.is_some() {
                    Touch::Motion
                } else if touchpad.touch.button.is_some() {
                    let button_string = touchpad.touch.button.as_ref().unwrap();
                    let button = TouchButton::from_str(button_string.as_str());
                    if button.is_err() {
                        log::error!("Invalid or unimplemented button: {button_string}");
                        return Capability::NotImplemented;
                    }
                    let button = button.unwrap();
                    Touch::Button(button)
                } else {
                    log::error!("Invalid or unimplemented touchpad config");
                    return Capability::NotImplemented;
                }
            };

            // TODO: Is there a better way to do this?
            match touchpad.name.as_str() {
                "LeftPad" => return Capability::Touchpad(Touchpad::LeftPad(touch)),
                "RightPad" => return Capability::Touchpad(Touchpad::RightPad(touch)),
                "CenterPad" => return Capability::Touchpad(Touchpad::CenterPad(touch)),
                _ => return Capability::NotImplemented,
            }
        }

        // Touchscreen
        if let Some(touch) = value.touchscreen.as_ref() {
            // Motion
            if touch.motion.is_some() {
                return Capability::Touchscreen(Touch::Motion);
            }

            // Button
            if let Some(button_string) = touch.button.as_ref() {
                let button = TouchButton::from_str(button_string);
                if button.is_err() {
                    log::error!("Invalid or unimplemented button: {button_string}");
                    return Capability::NotImplemented;
                }
                let button = button.unwrap();
                return Capability::Touchscreen(Touch::Button(button));
            }
        }

        // Layer activation
        if let Some(_) = value.layer.as_ref() {
            return Capability::InputLayer(InputLayer::Activate);
        }

        Capability::NotImplemented
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Gamepad {
    /// Gamepad Buttons typically use binary input that represents button presses
    Button(GamepadButton),
    /// Gamepad Axes typically use (x, y) input that represents multi-axis input
    Axis(GamepadAxis),
    /// Gamepad Trigger typically uses a single unsigned integar value that represents
    /// how far a trigger has been pulled
    Trigger(GamepadTrigger),
    /// Accelerometer events measure the current acceleration of a device. This is
    /// normally used to determine which way is "down" as there will be a constant
    /// acceleration towards the center of the earth at 9.8 meters per second.
    /// Typical will use (x, y, z) values normalized to meters per second.
    Accelerometer,
    /// Gyro events measure the angular velocity of a device measured
    /// with (x, y, z) values normalized to degrees per second.
    Gyro,
    /// Dials and wheels
    Dial(GamepadDial),
}

impl fmt::Display for Gamepad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Gamepad::Button(_) => write!(f, "Button"),
            Gamepad::Axis(_) => write!(f, "Axis"),
            Gamepad::Trigger(_) => write!(f, "Trigger"),
            Gamepad::Accelerometer => write!(f, "Accelerometer"),
            Gamepad::Gyro => write!(f, "Gyro"),
            Gamepad::Dial(_) => write!(f, "Dial"),
        }
    }
}

impl FromStr for Gamepad {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        let Some((part, parts)) = parts.split_first() else {
            return Err(());
        };
        match *part {
            "Button" => Ok(Gamepad::Button(GamepadButton::from_str(
                parts.join(":").as_str(),
            )?)),
            "Axis" => Ok(Gamepad::Axis(GamepadAxis::from_str(
                parts.join(":").as_str(),
            )?)),
            "Trigger" => Ok(Gamepad::Trigger(GamepadTrigger::from_str(
                parts.join(":").as_str(),
            )?)),
            "Accelerometer" => Ok(Gamepad::Accelerometer),
            "Gyro" => Ok(Gamepad::Gyro),
            "Dial" => Ok(Gamepad::Dial(GamepadDial::from_str(
                parts.join(":").as_str(),
            )?)),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

impl FromStr for Mouse {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        let Some((part, parts)) = parts.split_first() else {
            return Err(());
        };
        match *part {
            "Motion" => Ok(Mouse::Motion),
            "Button" => Ok(Mouse::Button(MouseButton::from_str(
                parts.join(":").as_str(),
            )?)),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    Extra,
    /// Extra mouse button, usually on the side of the mouse
    Side,
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
            MouseButton::Extra => write!(f, "Extra1"),
            MouseButton::Side => write!(f, "Extra2"),
        }
    }
}

impl FromStr for MouseButton {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Left" => Ok(MouseButton::Left),
            "Right" => Ok(MouseButton::Right),
            "Middle" => Ok(MouseButton::Middle),
            "WheelUp" => Ok(MouseButton::WheelUp),
            "WheelDown" => Ok(MouseButton::WheelDown),
            "WheelLeft" => Ok(MouseButton::WheelLeft),
            "WheelRight" => Ok(MouseButton::WheelRight),
            "Extra1" => Ok(MouseButton::Extra),
            "Extra2" => Ok(MouseButton::Side),
            _ => Err(()),
        }
    }
}

/// Gamepad Buttons typically use binary input that represents button presses
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    /// Dedicated mute button
    Mute,
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
}

impl fmt::Display for GamepadButton {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GamepadButton::DPadDown => write!(f, "DPadDown"),
            GamepadButton::DPadLeft => write!(f, "DPadLeft"),
            GamepadButton::DPadRight => write!(f, "DPadRight"),
            GamepadButton::DPadUp => write!(f, "DPadUp"),
            GamepadButton::East => write!(f, "East"),
            GamepadButton::Guide => write!(f, "Guide"),
            GamepadButton::Keyboard => write!(f, "Keyboard"),
            GamepadButton::LeftBumper => write!(f, "LeftBumper"),
            GamepadButton::LeftPaddle1 => write!(f, "LeftPaddle1"),
            GamepadButton::LeftPaddle2 => write!(f, "LeftPaddle2"),
            GamepadButton::LeftPaddle3 => write!(f, "LeftPaddle3"),
            GamepadButton::LeftStick => write!(f, "LeftStick"),
            GamepadButton::LeftStickTouch => write!(f, "LeftStickTouch"),
            GamepadButton::LeftTop => write!(f, "LeftTop"),
            GamepadButton::LeftTrigger => write!(f, "LeftTrigger"),
            GamepadButton::Mute => write!(f, "Mute"),
            GamepadButton::North => write!(f, "North"),
            GamepadButton::QuickAccess => write!(f, "QuickAccess"),
            GamepadButton::QuickAccess2 => write!(f, "QuickAccess2"),
            GamepadButton::RightBumper => write!(f, "RightBumper"),
            GamepadButton::RightPaddle1 => write!(f, "RightPaddle1"),
            GamepadButton::RightPaddle2 => write!(f, "RightPaddle2"),
            GamepadButton::RightPaddle3 => write!(f, "RightPaddle3"),
            GamepadButton::RightStick => write!(f, "RightStick"),
            GamepadButton::RightStickTouch => write!(f, "RightStickTouch"),
            GamepadButton::RightTop => write!(f, "RightTop"),
            GamepadButton::RightTrigger => write!(f, "RightTrigger"),
            GamepadButton::Screenshot => write!(f, "Screenshot"),
            GamepadButton::Select => write!(f, "Select"),
            GamepadButton::South => write!(f, "South"),
            GamepadButton::Start => write!(f, "Start"),
            GamepadButton::West => write!(f, "West"),
        }
    }
}

impl FromStr for GamepadButton {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "DPadDown" => Ok(GamepadButton::DPadDown),
            "DPadLeft" => Ok(GamepadButton::DPadLeft),
            "DPadRight" => Ok(GamepadButton::DPadRight),
            "DPadUp" => Ok(GamepadButton::DPadUp),
            "East" => Ok(GamepadButton::East),
            "Guide" => Ok(GamepadButton::Guide),
            "Keyboard" => Ok(GamepadButton::Keyboard),
            "LeftBumper" => Ok(GamepadButton::LeftBumper),
            "LeftPaddle1" => Ok(GamepadButton::LeftPaddle1),
            "LeftPaddle2" => Ok(GamepadButton::LeftPaddle2),
            "LeftPaddle3" => Ok(GamepadButton::LeftPaddle3),
            "LeftStick" => Ok(GamepadButton::LeftStick),
            "LeftStickTouch" => Ok(GamepadButton::LeftStickTouch),
            "LeftTop" => Ok(GamepadButton::LeftTop),
            "LeftTrigger" => Ok(GamepadButton::LeftTrigger),
            "Mute" => Ok(GamepadButton::Mute),
            "North" => Ok(GamepadButton::North),
            "QuickAccess" => Ok(GamepadButton::QuickAccess),
            "QuickAccess2" => Ok(GamepadButton::QuickAccess2),
            "RightBumper" => Ok(GamepadButton::RightBumper),
            "RightPaddle1" => Ok(GamepadButton::RightPaddle1),
            "RightPaddle2" => Ok(GamepadButton::RightPaddle2),
            "RightPaddle3" => Ok(GamepadButton::RightPaddle3),
            "RightStick" => Ok(GamepadButton::RightStick),
            "RightStickTouch" => Ok(GamepadButton::RightStickTouch),
            "RightTop" => Ok(GamepadButton::RightTop),
            "RightTrigger" => Ok(GamepadButton::RightTrigger),
            "Screenshot" => Ok(GamepadButton::Screenshot),
            "Select" => Ok(GamepadButton::Select),
            "South" => Ok(GamepadButton::South),
            "Start" => Ok(GamepadButton::Start),
            "West" => Ok(GamepadButton::West),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStick,
    RightStick,
    Hat0,
    Hat1,
    Hat2,
    Hat3,
}

impl fmt::Display for GamepadAxis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GamepadAxis::LeftStick => write!(f, "LeftStick"),
            GamepadAxis::RightStick => write!(f, "RightStick"),
            GamepadAxis::Hat0 => write!(f, "Hat0"),
            GamepadAxis::Hat1 => write!(f, "Hat1"),
            GamepadAxis::Hat2 => write!(f, "Hat2"),
            GamepadAxis::Hat3 => write!(f, "Hat3"),
        }
    }
}

impl FromStr for GamepadAxis {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "LeftStick" => Ok(GamepadAxis::LeftStick),
            "RightStick" => Ok(GamepadAxis::RightStick),
            "Hat0" => Ok(GamepadAxis::Hat0),
            "Hat1" => Ok(GamepadAxis::Hat1),
            "Hat2" => Ok(GamepadAxis::Hat2),
            "Hat3" => Ok(GamepadAxis::Hat3),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

impl FromStr for GamepadTrigger {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "LeftTrigger" => Ok(GamepadTrigger::LeftTrigger),
            "LeftTouchpadForce" => Ok(GamepadTrigger::LeftTouchpadForce),
            "LeftStickForce" => Ok(GamepadTrigger::LeftStickForce),
            "RightTrigger" => Ok(GamepadTrigger::RightTrigger),
            "RightTouchpadForce" => Ok(GamepadTrigger::RightTouchpadForce),
            "RightStickForce" => Ok(GamepadTrigger::RightStickForce),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GamepadDial {
    LeftStickDial,
    RightStickDial,
}

impl fmt::Display for GamepadDial {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GamepadDial::LeftStickDial => write!(f, "LeftStickDial"),
            GamepadDial::RightStickDial => write!(f, "RightStickDial"),
        }
    }
}

impl FromStr for GamepadDial {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "LeftStickDial" => Ok(GamepadDial::LeftStickDial),
            "RightStickDial" => Ok(GamepadDial::RightStickDial),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Keyboard {
    Key0,
    Key1,
    Key102nd,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    KeyA,
    KeyAgain,
    KeyApostrophe,
    KeyB,
    KeyBack,
    KeyBackslash,
    KeyBackspace,
    KeyBrightnessDown,
    KeyBrightnessUp,
    KeyC,
    KeyCalc,
    KeyCapslock,
    KeyComma,
    KeyCompose,
    KeyCopy,
    KeyCut,
    KeyD,
    KeyDelete,
    KeyDot,
    KeyDown,
    KeyE,
    KeyEdit,
    KeyEjectCD,
    KeyEnd,
    KeyEnter,
    KeyEqual,
    KeyEsc,
    KeyF,
    KeyF1,
    KeyF10,
    KeyF11,
    KeyF12,
    KeyF13,
    KeyF14,
    KeyF15,
    KeyF16,
    KeyF17,
    KeyF18,
    KeyF19,
    KeyF2,
    KeyF20,
    KeyF21,
    KeyF22,
    KeyF23,
    KeyF24,
    KeyF3,
    KeyF4,
    KeyF5,
    KeyF6,
    KeyF7,
    KeyF8,
    KeyF9,
    KeyFind,
    KeyForward,
    KeyFront,
    KeyG,
    KeyGrave,
    KeyH,
    KeyHanja,
    KeyHelp,
    KeyHenkan,
    KeyHiragana,
    KeyHome,
    KeyI,
    KeyInsert,
    KeyJ,
    KeyK,
    KeyKatakana,
    KeyKatakanaHiragana,
    KeyKp0,
    KeyKp1,
    KeyKp2,
    KeyKp3,
    KeyKp4,
    KeyKp5,
    KeyKp6,
    KeyKp7,
    KeyKp8,
    KeyKp9,
    KeyKpAsterisk,
    KeyKpComma,
    KeyKpDot,
    KeyKpEnter,
    KeyKpEqual,
    KeyKpJpComma,
    KeyKpLeftParen,
    KeyKpMinus,
    KeyKpPlus,
    KeyKpRightParen,
    KeyKpSlash,
    KeyL,
    KeyLeft,
    KeyLeftAlt,
    KeyLeftBrace,
    KeyLeftCtrl,
    KeyLeftMeta,
    KeyLeftShift,
    KeyM,
    KeyMinus,
    KeyMuhenkan,
    KeyMute,
    KeyN,
    KeyNextSong,
    KeyNumlock,
    KeyO,
    KeyOpen,
    KeyP,
    KeyPageDown,
    KeyPageUp,
    KeyPaste,
    KeyPause,
    KeyPlayPause,
    KeyPower,
    KeyPreviousSong,
    KeyProg1,
    KeyProps,
    KeyQ,
    KeyR,
    KeyRecord,
    KeyRefresh,
    KeyRight,
    KeyRightAlt,
    KeyRightBrace,
    KeyRightCtrl,
    KeyRightMeta,
    KeyRightShift,
    KeyRo,
    KeyS,
    KeyScrollDown,
    KeyScrollLock,
    KeyScrollUp,
    KeySemicolon,
    KeySlash,
    KeySleep,
    KeySpace,
    KeyStop,
    KeyStopCD,
    KeySysrq,
    KeyT,
    KeyTab,
    KeyU,
    KeyUndo,
    KeyUp,
    KeyV,
    KeyVolumeDown,
    KeyVolumeUp,
    KeyW,
    KeyWww,
    KeyX,
    KeyY,
    KeyYen,
    KeyZ,
    KeyZenkakuhankaku,
}

impl fmt::Display for Keyboard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Keyboard::Key0 => write!(f, "Key0"),
            Keyboard::Key1 => write!(f, "Key1"),
            Keyboard::Key102nd => write!(f, "Key102nd"),
            Keyboard::Key2 => write!(f, "Key2"),
            Keyboard::Key3 => write!(f, "Key3"),
            Keyboard::Key4 => write!(f, "Key4"),
            Keyboard::Key5 => write!(f, "Key5"),
            Keyboard::Key6 => write!(f, "Key6"),
            Keyboard::Key7 => write!(f, "Key7"),
            Keyboard::Key8 => write!(f, "Key8"),
            Keyboard::Key9 => write!(f, "Key9"),
            Keyboard::KeyA => write!(f, "KeyA"),
            Keyboard::KeyAgain => write!(f, "KeyAgain"),
            Keyboard::KeyApostrophe => write!(f, "KeyApostrophe"),
            Keyboard::KeyB => write!(f, "KeyB"),
            Keyboard::KeyBack => write!(f, "KeyBack"),
            Keyboard::KeyBackslash => write!(f, "KeyBackslash"),
            Keyboard::KeyBackspace => write!(f, "KeyBackspace"),
            Keyboard::KeyBrightnessDown => write!(f, "KeyBrightnessDown"),
            Keyboard::KeyBrightnessUp => write!(f, "KeyBrightnessUp"),
            Keyboard::KeyC => write!(f, "KeyC"),
            Keyboard::KeyCalc => write!(f, "KeyCalc"),
            Keyboard::KeyCapslock => write!(f, "KeyCapslock"),
            Keyboard::KeyComma => write!(f, "KeyComma"),
            Keyboard::KeyCompose => write!(f, "KeyCompose"),
            Keyboard::KeyCopy => write!(f, "KeyCopy"),
            Keyboard::KeyCut => write!(f, "KeyCut"),
            Keyboard::KeyD => write!(f, "KeyD"),
            Keyboard::KeyDelete => write!(f, "KeyDelete"),
            Keyboard::KeyDot => write!(f, "KeyDot"),
            Keyboard::KeyDown => write!(f, "KeyDown"),
            Keyboard::KeyE => write!(f, "KeyE"),
            Keyboard::KeyEdit => write!(f, "KeyEdit"),
            Keyboard::KeyEjectCD => write!(f, "KeyEjectCD"),
            Keyboard::KeyEnd => write!(f, "KeyEnd"),
            Keyboard::KeyEnter => write!(f, "KeyEnter"),
            Keyboard::KeyEqual => write!(f, "KeyEqual"),
            Keyboard::KeyEsc => write!(f, "KeyEsc"),
            Keyboard::KeyF => write!(f, "KeyF"),
            Keyboard::KeyF1 => write!(f, "KeyF1"),
            Keyboard::KeyF10 => write!(f, "KeyF10"),
            Keyboard::KeyF11 => write!(f, "KeyF11"),
            Keyboard::KeyF12 => write!(f, "KeyF12"),
            Keyboard::KeyF13 => write!(f, "KeyF13"),
            Keyboard::KeyF14 => write!(f, "KeyF14"),
            Keyboard::KeyF15 => write!(f, "KeyF15"),
            Keyboard::KeyF16 => write!(f, "KeyF16"),
            Keyboard::KeyF17 => write!(f, "KeyF17"),
            Keyboard::KeyF18 => write!(f, "KeyF18"),
            Keyboard::KeyF19 => write!(f, "KeyF19"),
            Keyboard::KeyF2 => write!(f, "KeyF2"),
            Keyboard::KeyF20 => write!(f, "KeyF20"),
            Keyboard::KeyF21 => write!(f, "KeyF21"),
            Keyboard::KeyF22 => write!(f, "KeyF22"),
            Keyboard::KeyF23 => write!(f, "KeyF23"),
            Keyboard::KeyF24 => write!(f, "KeyF24"),
            Keyboard::KeyF3 => write!(f, "KeyF3"),
            Keyboard::KeyF4 => write!(f, "KeyF4"),
            Keyboard::KeyF5 => write!(f, "KeyF5"),
            Keyboard::KeyF6 => write!(f, "KeyF6"),
            Keyboard::KeyF7 => write!(f, "KeyF7"),
            Keyboard::KeyF8 => write!(f, "KeyF8"),
            Keyboard::KeyF9 => write!(f, "KeyF9"),
            Keyboard::KeyFind => write!(f, "KeyFind"),
            Keyboard::KeyForward => write!(f, "KeyForward"),
            Keyboard::KeyFront => write!(f, "KeyFront"),
            Keyboard::KeyG => write!(f, "KeyG"),
            Keyboard::KeyGrave => write!(f, "KeyGrave"),
            Keyboard::KeyH => write!(f, "KeyH"),
            Keyboard::KeyHanja => write!(f, "KeyHanja"),
            Keyboard::KeyHelp => write!(f, "KeyHelp"),
            Keyboard::KeyHenkan => write!(f, "KeyHenkan"),
            Keyboard::KeyHiragana => write!(f, "KeyHiragana"),
            Keyboard::KeyHome => write!(f, "KeyHome"),
            Keyboard::KeyI => write!(f, "KeyI"),
            Keyboard::KeyInsert => write!(f, "KeyInsert"),
            Keyboard::KeyJ => write!(f, "KeyJ"),
            Keyboard::KeyK => write!(f, "KeyK"),
            Keyboard::KeyKatakana => write!(f, "KeyKatakana"),
            Keyboard::KeyKatakanaHiragana => write!(f, "KeyKatakanaHiragana"),
            Keyboard::KeyKp0 => write!(f, "KeyKp0"),
            Keyboard::KeyKp1 => write!(f, "KeyKp1"),
            Keyboard::KeyKp2 => write!(f, "KeyKp2"),
            Keyboard::KeyKp3 => write!(f, "KeyKp3"),
            Keyboard::KeyKp4 => write!(f, "KeyKp4"),
            Keyboard::KeyKp5 => write!(f, "KeyKp5"),
            Keyboard::KeyKp6 => write!(f, "KeyKp6"),
            Keyboard::KeyKp7 => write!(f, "KeyKp7"),
            Keyboard::KeyKp8 => write!(f, "KeyKp8"),
            Keyboard::KeyKp9 => write!(f, "KeyKp9"),
            Keyboard::KeyKpAsterisk => write!(f, "KeyKpAsterisk"),
            Keyboard::KeyKpComma => write!(f, "KeyKpComma"),
            Keyboard::KeyKpDot => write!(f, "KeyKpDot"),
            Keyboard::KeyKpEnter => write!(f, "KeyKpEnter"),
            Keyboard::KeyKpEqual => write!(f, "KeyKpEqual"),
            Keyboard::KeyKpJpComma => write!(f, "KeyKpJpComma"),
            Keyboard::KeyKpLeftParen => write!(f, "KeyKpLeftParen"),
            Keyboard::KeyKpMinus => write!(f, "KeyKpMinus"),
            Keyboard::KeyKpPlus => write!(f, "KeyKpPlus"),
            Keyboard::KeyKpRightParen => write!(f, "KeyKpRightParen"),
            Keyboard::KeyKpSlash => write!(f, "KeyKpSlash"),
            Keyboard::KeyL => write!(f, "KeyL"),
            Keyboard::KeyLeft => write!(f, "KeyLeft"),
            Keyboard::KeyLeftAlt => write!(f, "KeyLeftAlt"),
            Keyboard::KeyLeftBrace => write!(f, "KeyLeftBrace"),
            Keyboard::KeyLeftCtrl => write!(f, "KeyLeftCtrl"),
            Keyboard::KeyLeftMeta => write!(f, "KeyLeftMeta"),
            Keyboard::KeyLeftShift => write!(f, "KeyLeftShift"),
            Keyboard::KeyM => write!(f, "KeyM"),
            Keyboard::KeyMinus => write!(f, "KeyMinus"),
            Keyboard::KeyMuhenkan => write!(f, "KeyMuhenkan"),
            Keyboard::KeyMute => write!(f, "KeyMute"),
            Keyboard::KeyN => write!(f, "KeyN"),
            Keyboard::KeyNextSong => write!(f, "KeyNextSong"),
            Keyboard::KeyNumlock => write!(f, "KeyNumlock"),
            Keyboard::KeyO => write!(f, "KeyO"),
            Keyboard::KeyOpen => write!(f, "KeyOpen"),
            Keyboard::KeyP => write!(f, "KeyP"),
            Keyboard::KeyPageDown => write!(f, "KeyPageDown"),
            Keyboard::KeyPageUp => write!(f, "KeyPageUp"),
            Keyboard::KeyPaste => write!(f, "KeyPaste"),
            Keyboard::KeyPause => write!(f, "KeyPause"),
            Keyboard::KeyPlayPause => write!(f, "KeyPlayPause"),
            Keyboard::KeyPower => write!(f, "KeyPower"),
            Keyboard::KeyPreviousSong => write!(f, "KeyPreviousSong"),
            Keyboard::KeyProg1 => write!(f, "KeyProg1"),
            Keyboard::KeyProps => write!(f, "KeyProps"),
            Keyboard::KeyQ => write!(f, "KeyQ"),
            Keyboard::KeyR => write!(f, "KeyR"),
            Keyboard::KeyRecord => write!(f, "KeyRecord"),
            Keyboard::KeyRefresh => write!(f, "KeyRefresh"),
            Keyboard::KeyRight => write!(f, "KeyRight"),
            Keyboard::KeyRightAlt => write!(f, "KeyRightAlt"),
            Keyboard::KeyRightBrace => write!(f, "KeyRightBrace"),
            Keyboard::KeyRightCtrl => write!(f, "KeyRightCtrl"),
            Keyboard::KeyRightMeta => write!(f, "KeyRightMeta"),
            Keyboard::KeyRightShift => write!(f, "KeyRightShift"),
            Keyboard::KeyRo => write!(f, "KeyRo"),
            Keyboard::KeyS => write!(f, "KeyS"),
            Keyboard::KeyScrollDown => write!(f, "KeyScrollDown"),
            Keyboard::KeyScrollLock => write!(f, "KeyScrollLock"),
            Keyboard::KeyScrollUp => write!(f, "KeyScrollUp"),
            Keyboard::KeySemicolon => write!(f, "KeySemicolon"),
            Keyboard::KeySlash => write!(f, "KeySlash"),
            Keyboard::KeySleep => write!(f, "KeySleep"),
            Keyboard::KeySpace => write!(f, "KeySpace"),
            Keyboard::KeyStop => write!(f, "KeyStop"),
            Keyboard::KeyStopCD => write!(f, "KeyStopCD"),
            Keyboard::KeySysrq => write!(f, "KeySysrq"),
            Keyboard::KeyT => write!(f, "KeyT"),
            Keyboard::KeyTab => write!(f, "KeyTab"),
            Keyboard::KeyU => write!(f, "KeyU"),
            Keyboard::KeyUndo => write!(f, "KeyUndo"),
            Keyboard::KeyUp => write!(f, "KeyUp"),
            Keyboard::KeyV => write!(f, "KeyV"),
            Keyboard::KeyVolumeDown => write!(f, "KeyVolumeDown"),
            Keyboard::KeyVolumeUp => write!(f, "KeyVolumeUp"),
            Keyboard::KeyW => write!(f, "KeyW"),
            Keyboard::KeyWww => write!(f, "KeyWww"),
            Keyboard::KeyX => write!(f, "KeyX"),
            Keyboard::KeyY => write!(f, "KeyY"),
            Keyboard::KeyYen => write!(f, "KeyYen"),
            Keyboard::KeyZ => write!(f, "KeyZ"),
            Keyboard::KeyZenkakuhankaku => write!(f, "KeyZenkakuhankaku"),
        }
    }
}

impl FromStr for Keyboard {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Key0" => Ok(Keyboard::Key0),
            "Key1" => Ok(Keyboard::Key1),
            "Key102nd" => Ok(Keyboard::Key102nd),
            "Key2" => Ok(Keyboard::Key2),
            "Key3" => Ok(Keyboard::Key3),
            "Key4" => Ok(Keyboard::Key4),
            "Key5" => Ok(Keyboard::Key5),
            "Key6" => Ok(Keyboard::Key6),
            "Key7" => Ok(Keyboard::Key7),
            "Key8" => Ok(Keyboard::Key8),
            "Key9" => Ok(Keyboard::Key9),
            "KeyA" => Ok(Keyboard::KeyA),
            "KeyAgain" => Ok(Keyboard::KeyAgain),
            "KeyApostrophe" => Ok(Keyboard::KeyApostrophe),
            "KeyB" => Ok(Keyboard::KeyB),
            "KeyBack" => Ok(Keyboard::KeyBack),
            "KeyBackslash" => Ok(Keyboard::KeyBackslash),
            "KeyBackspace" => Ok(Keyboard::KeyBackspace),
            "KeyBrightnessDown" => Ok(Keyboard::KeyBrightnessDown),
            "KeyBrightnessUp" => Ok(Keyboard::KeyBrightnessUp),
            "KeyC" => Ok(Keyboard::KeyC),
            "KeyCalc" => Ok(Keyboard::KeyCalc),
            "KeyCapslock" => Ok(Keyboard::KeyCapslock),
            "KeyComma" => Ok(Keyboard::KeyComma),
            "KeyCompose" => Ok(Keyboard::KeyCompose),
            "KeyCopy" => Ok(Keyboard::KeyCopy),
            "KeyCut" => Ok(Keyboard::KeyCut),
            "KeyD" => Ok(Keyboard::KeyD),
            "KeyDelete" => Ok(Keyboard::KeyDelete),
            "KeyDot" => Ok(Keyboard::KeyDot),
            "KeyDown" => Ok(Keyboard::KeyDown),
            "KeyE" => Ok(Keyboard::KeyE),
            "KeyEdit" => Ok(Keyboard::KeyEdit),
            "KeyEjectCD" => Ok(Keyboard::KeyEjectCD),
            "KeyEnd" => Ok(Keyboard::KeyEnd),
            "KeyEnter" => Ok(Keyboard::KeyEnter),
            "KeyEqual" => Ok(Keyboard::KeyEqual),
            "KeyEsc" => Ok(Keyboard::KeyEsc),
            "KeyF" => Ok(Keyboard::KeyF),
            "KeyF1" => Ok(Keyboard::KeyF1),
            "KeyF10" => Ok(Keyboard::KeyF10),
            "KeyF11" => Ok(Keyboard::KeyF11),
            "KeyF12" => Ok(Keyboard::KeyF12),
            "KeyF13" => Ok(Keyboard::KeyF13),
            "KeyF14" => Ok(Keyboard::KeyF14),
            "KeyF15" => Ok(Keyboard::KeyF15),
            "KeyF16" => Ok(Keyboard::KeyF16),
            "KeyF17" => Ok(Keyboard::KeyF17),
            "KeyF18" => Ok(Keyboard::KeyF18),
            "KeyF19" => Ok(Keyboard::KeyF19),
            "KeyF2" => Ok(Keyboard::KeyF2),
            "KeyF20" => Ok(Keyboard::KeyF20),
            "KeyF21" => Ok(Keyboard::KeyF21),
            "KeyF22" => Ok(Keyboard::KeyF22),
            "KeyF23" => Ok(Keyboard::KeyF23),
            "KeyF24" => Ok(Keyboard::KeyF24),
            "KeyF3" => Ok(Keyboard::KeyF3),
            "KeyF4" => Ok(Keyboard::KeyF4),
            "KeyF5" => Ok(Keyboard::KeyF5),
            "KeyF6" => Ok(Keyboard::KeyF6),
            "KeyF7" => Ok(Keyboard::KeyF7),
            "KeyF8" => Ok(Keyboard::KeyF8),
            "KeyF9" => Ok(Keyboard::KeyF9),
            "KeyFind" => Ok(Keyboard::KeyFind),
            "KeyForward" => Ok(Keyboard::KeyForward),
            "KeyFront" => Ok(Keyboard::KeyFront),
            "KeyG" => Ok(Keyboard::KeyG),
            "KeyGrave" => Ok(Keyboard::KeyGrave),
            "KeyH" => Ok(Keyboard::KeyH),
            "KeyHanja" => Ok(Keyboard::KeyHanja),
            "KeyHelp" => Ok(Keyboard::KeyHelp),
            "KeyHenkan" => Ok(Keyboard::KeyHenkan),
            "KeyHiragana" => Ok(Keyboard::KeyHiragana),
            "KeyHome" => Ok(Keyboard::KeyHome),
            "KeyI" => Ok(Keyboard::KeyI),
            "KeyInsert" => Ok(Keyboard::KeyInsert),
            "KeyJ" => Ok(Keyboard::KeyJ),
            "KeyK" => Ok(Keyboard::KeyK),
            "KeyKatakana" => Ok(Keyboard::KeyKatakana),
            "KeyKatakanaHiragana" => Ok(Keyboard::KeyKatakanaHiragana),
            "KeyKp0" => Ok(Keyboard::KeyKp0),
            "KeyKp1" => Ok(Keyboard::KeyKp1),
            "KeyKp2" => Ok(Keyboard::KeyKp2),
            "KeyKp3" => Ok(Keyboard::KeyKp3),
            "KeyKp4" => Ok(Keyboard::KeyKp4),
            "KeyKp5" => Ok(Keyboard::KeyKp5),
            "KeyKp6" => Ok(Keyboard::KeyKp6),
            "KeyKp7" => Ok(Keyboard::KeyKp7),
            "KeyKp8" => Ok(Keyboard::KeyKp8),
            "KeyKp9" => Ok(Keyboard::KeyKp9),
            "KeyKpAsterisk" => Ok(Keyboard::KeyKpAsterisk),
            "KeyKpComma" => Ok(Keyboard::KeyKpComma),
            "KeyKpEnter" => Ok(Keyboard::KeyKpEnter),
            "KeyKpEqual" => Ok(Keyboard::KeyKpEqual),
            "KeyKpJpComma" => Ok(Keyboard::KeyKpJpComma),
            "KeyKpLeftParen" => Ok(Keyboard::KeyKpLeftParen),
            "KeyKpMinus" => Ok(Keyboard::KeyKpMinus),
            "KeyKpPlus" => Ok(Keyboard::KeyKpPlus),
            "KeyKpRightParen" => Ok(Keyboard::KeyKpRightParen),
            "KeyKpSlash" => Ok(Keyboard::KeyKpSlash),
            "KeyKpdot" => Ok(Keyboard::KeyKpDot),
            "KeyL" => Ok(Keyboard::KeyL),
            "KeyLeft" => Ok(Keyboard::KeyLeft),
            "KeyLeftAlt" => Ok(Keyboard::KeyLeftAlt),
            "KeyLeftBrace" => Ok(Keyboard::KeyLeftBrace),
            "KeyLeftCtrl" => Ok(Keyboard::KeyLeftCtrl),
            "KeyLeftMeta" => Ok(Keyboard::KeyLeftMeta),
            "KeyLeftShift" => Ok(Keyboard::KeyLeftShift),
            "KeyM" => Ok(Keyboard::KeyM),
            "KeyMinus" => Ok(Keyboard::KeyMinus),
            "KeyMuhenkan" => Ok(Keyboard::KeyMuhenkan),
            "KeyMute" => Ok(Keyboard::KeyMute),
            "KeyN" => Ok(Keyboard::KeyN),
            "KeyNextSong" => Ok(Keyboard::KeyNextSong),
            "KeyNumlock" => Ok(Keyboard::KeyNumlock),
            "KeyO" => Ok(Keyboard::KeyO),
            "KeyOpen" => Ok(Keyboard::KeyOpen),
            "KeyP" => Ok(Keyboard::KeyP),
            "KeyPageDown" => Ok(Keyboard::KeyPageDown),
            "KeyPageUp" => Ok(Keyboard::KeyPageUp),
            "KeyPaste" => Ok(Keyboard::KeyPaste),
            "KeyPause" => Ok(Keyboard::KeyPause),
            "KeyPlayPause" => Ok(Keyboard::KeyPlayPause),
            "KeyPower" => Ok(Keyboard::KeyPower),
            "KeyPreviousSong" => Ok(Keyboard::KeyPreviousSong),
            "KeyProg1" => Ok(Keyboard::KeyProg1),
            "KeyProps" => Ok(Keyboard::KeyProps),
            "KeyQ" => Ok(Keyboard::KeyQ),
            "KeyR" => Ok(Keyboard::KeyR),
            "KeyRecord" => Ok(Keyboard::KeyRecord),
            "KeyRefresh" => Ok(Keyboard::KeyRefresh),
            "KeyRight" => Ok(Keyboard::KeyRight),
            "KeyRightAlt" => Ok(Keyboard::KeyRightAlt),
            "KeyRightBrace" => Ok(Keyboard::KeyRightBrace),
            "KeyRightCtrl" => Ok(Keyboard::KeyRightCtrl),
            "KeyRightMeta" => Ok(Keyboard::KeyRightMeta),
            "KeyRightShift" => Ok(Keyboard::KeyRightShift),
            "KeyRo" => Ok(Keyboard::KeyRo),
            "KeyS" => Ok(Keyboard::KeyS),
            "KeyScrollDown" => Ok(Keyboard::KeyScrollDown),
            "KeyScrollLock" => Ok(Keyboard::KeyScrollLock),
            "KeyScrollUp" => Ok(Keyboard::KeyScrollUp),
            "KeySemicolon" => Ok(Keyboard::KeySemicolon),
            "KeySlash" => Ok(Keyboard::KeySlash),
            "KeySleep" => Ok(Keyboard::KeySleep),
            "KeySpace" => Ok(Keyboard::KeySpace),
            "KeyStop" => Ok(Keyboard::KeyStop),
            "KeyStopCD" => Ok(Keyboard::KeyStopCD),
            "KeySysrq" => Ok(Keyboard::KeySysrq),
            "KeyT" => Ok(Keyboard::KeyT),
            "KeyTab" => Ok(Keyboard::KeyTab),
            "KeyU" => Ok(Keyboard::KeyU),
            "KeyUndo" => Ok(Keyboard::KeyUndo),
            "KeyUp" => Ok(Keyboard::KeyUp),
            "KeyV" => Ok(Keyboard::KeyV),
            "KeyVolumeDown" => Ok(Keyboard::KeyVolumeDown),
            "KeyVolumeUp" => Ok(Keyboard::KeyVolumeUp),
            "KeyW" => Ok(Keyboard::KeyW),
            "KeyWww" => Ok(Keyboard::KeyWww),
            "KeyX" => Ok(Keyboard::KeyX),
            "KeyY" => Ok(Keyboard::KeyY),
            "KeyYen" => Ok(Keyboard::KeyYen),
            "KeyZ" => Ok(Keyboard::KeyZ),
            "KeyZenkakuhankaku" => Ok(Keyboard::KeyZenkakuhankaku),
            _ => Err(()),
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Touchpad {
    LeftPad(Touch),
    RightPad(Touch),
    CenterPad(Touch),
}

impl fmt::Display for Touchpad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Touchpad::LeftPad(_) => write!(f, "LeftPad"),
            Touchpad::RightPad(_) => write!(f, "RightPad"),
            Touchpad::CenterPad(_) => write!(f, "CenterPad"),
        }
    }
}

impl FromStr for Touchpad {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        let Some((part, parts)) = parts.split_first() else {
            return Err(());
        };
        match *part {
            "LeftPad" => Ok(Touchpad::LeftPad(Touch::from_str(
                parts.join(":").as_str(),
            )?)),
            "RightPad" => Ok(Touchpad::RightPad(Touch::from_str(
                parts.join(":").as_str(),
            )?)),
            "CenterPad" => Ok(Touchpad::CenterPad(Touch::from_str(
                parts.join(":").as_str(),
            )?)),

            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Touch {
    Motion,
    Button(TouchButton),
}

impl fmt::Display for Touch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Touch::Motion => write!(f, "Motion"),
            Touch::Button(_) => write!(f, "Button"),
        }
    }
}

impl FromStr for Touch {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        let Some((part, parts)) = parts.split_first() else {
            return Err(());
        };
        match *part {
            "Motion" => Ok(Touch::Motion),
            "Button" => Ok(Touch::Button(TouchButton::from_str(
                parts.join(":").as_str(),
            )?)),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TouchButton {
    Touch,
    Press,
}

impl fmt::Display for TouchButton {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TouchButton::Touch => write!(f, "Touch"),
            TouchButton::Press => write!(f, "Press"),
        }
    }
}

impl FromStr for TouchButton {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Touch" => Ok(TouchButton::Touch),
            "Press" => Ok(TouchButton::Press),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InputLayer {
    Activate,
}

impl fmt::Display for InputLayer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Activate => write!(f, "Activate"),
        }
    }
}

impl FromStr for InputLayer {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Activate" => Ok(Self::Activate),
            _ => Err(()),
        }
    }
}
