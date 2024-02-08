/// A capability describes what kind of input events an input device is capable
/// of emitting.
#[derive(Clone, Debug)]
pub enum Capability {
    None,
    Sync,
    Gamepad(Gamepad),
    Mouse(Mouse),
    Keyboard(Keyboard),
}

#[derive(Clone, Debug)]
pub enum Gamepad {
    Button(GamepadButton),
    Axis(GamepadAxis),
}

#[derive(Clone, Debug)]
pub enum Mouse {
    Motion,
    Button(MouseButton),
}

#[derive(Clone, Debug)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
    Extra1,
    Extra2,
}

#[derive(Clone, Debug)]
pub enum GamepadButton {
    South,
    East,
    North,
    West,
    LeftBumper,
    RightBumper,
    Start,
    Select,
    Guide,
    Base,
    LeftStick,
    RightStick,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

#[derive(Clone, Debug)]
pub enum GamepadAxis {
    LeftStick,
    RightStick,
    Hat1,
    Hat2,
}

#[derive(Clone, Debug)]
pub enum Keyboard {}
