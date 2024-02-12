/// A capability describes what kind of input events an input device is capable
/// of emitting.
#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub enum Mouse {
    /// Represents (x, y) relative mouse motion
    Motion,
    /// Mouse Buttons are typically binary mouse input that represents button presses
    Button(MouseButton),
}

#[derive(Clone, Debug)]
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

/// Gamepad Buttons typically use binary input that represents button presses
#[derive(Clone, Debug)]
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
    Base,
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
    /// Left trigger button, Deck binary sensor for left trigger
    LeftTrigger,
    /// Left back paddle button, Xbox P3, Steam Deck L4
    LeftPaddle1,
    /// Left back paddle button, Xbox P4, Steam Deck L5
    LeftPaddle2,
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
    /// Right trigger button, Deck binary sensor for right trigger
    RightTrigger,
    /// Right back paddle button, Xbox P1, Steam Deck R4
    RightPaddle1,
    /// Right back paddle button, Xbox P2, Steam Deck R5
    RightPaddle2,
    /// Z-axis button on the right stick, Sony R3, Xbox RS
    RightStick,
    /// Touch binary sensor for right stick
    RightStickTouch,
    /// Touch binary sensor for the right touchpad
    RightTouchpadTouch,
    /// Press binary sensor for the right touchpad
    RightTouchpadPress,
}

#[derive(Clone, Debug)]
pub enum GamepadAxis {
    LeftStick,
    RightStick,
    Hat1,
    Hat2,
    Hat3,
}

#[derive(Clone, Debug)]
pub enum GamepadTrigger {
    LeftTrigger,
    LeftTouchpadForce,
    LeftStickForce,
    RightTrigger,
    RightTouchpadForce,
    RightStickForce,
}

#[derive(Clone, Debug)]
pub enum Keyboard {}
