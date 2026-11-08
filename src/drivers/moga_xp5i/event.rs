use std::mem;

/// Events that can be emitted by the controller
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
    Joystick(JoystickEvent),
    Trigger(TriggerEvent),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Event::Button(a), Event::Button(b)) => a.variant_eq(b),
            (Event::Joystick(a), Event::Joystick(b)) => a.variant_eq(b),
            (Event::Trigger(a), Event::Trigger(b)) => a.variant_eq(b),
            _ => false,
        }
    }
}

/// [BinaryInput] contains either pressed or unpressed
#[derive(Clone, Debug, Default)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Button events represent binary inputs
#[derive(Clone, Debug)]
pub enum ButtonEvent {
    /// A button
    A(BinaryInput),
    /// B button
    B(BinaryInput),
    /// X button
    X(BinaryInput),
    /// Y button
    Y(BinaryInput),
    /// Left shoulder button
    LB(BinaryInput),
    /// Right shoulder button
    RB(BinaryInput),
    /// View / Options button
    View(BinaryInput),
    /// Menu / Start button
    Menu(BinaryInput),
    /// Home / Guide button
    Guide(BinaryInput),
    /// Left stick click
    LSClick(BinaryInput),
    /// Right stick click
    RSClick(BinaryInput),
    /// DPad up
    DPadUp(BinaryInput),
    /// DPad right
    DPadRight(BinaryInput),
    /// DPad down
    DPadDown(BinaryInput),
    /// DPad left
    DPadLeft(BinaryInput),
}

impl ButtonEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }

    /// Whether the button is pressed
    pub fn pressed(&self) -> bool {
        match self {
            Self::A(input)
            | Self::B(input)
            | Self::X(input)
            | Self::Y(input)
            | Self::LB(input)
            | Self::RB(input)
            | Self::View(input)
            | Self::Menu(input)
            | Self::Guide(input)
            | Self::LSClick(input)
            | Self::RSClick(input)
            | Self::DPadUp(input)
            | Self::DPadRight(input)
            | Self::DPadDown(input)
            | Self::DPadLeft(input) => input.pressed,
        }
    }
}

/// [JoystickInput] is a double (x, y) axis. The MFi report uses signed int8
/// values in the range -127..127.
#[derive(Clone, Debug, Default)]
pub struct JoystickInput {
    pub x: i8,
    pub y: i8,
}

/// [JoystickEvent] are events that have (x, y) values in the absolute domain
/// indicating how far left/right (x) and up/down (y) the joystick is off
/// center.
#[derive(Clone, Debug)]
pub enum JoystickEvent {
    LStick(JoystickInput),
    RStick(JoystickInput),
}

impl JoystickEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

/// [TriggerInput] is a single (z) axis
#[derive(Clone, Debug, Default)]
pub struct TriggerInput {
    pub value: u8,
}

/// [TriggerEvent] contains values indicating how far an analog trigger is
/// pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    LTAnalog(TriggerInput),
    RTAnalog(TriggerInput),
}

impl TriggerEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}
