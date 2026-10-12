use std::mem;

/// Events that can be emitted by the MSI Claw controller
#[derive(Clone, Debug)]
pub enum Event {
    Axis(AxisEvent),
    Button(ButtonEvent),
    Trigger(TriggerEvent),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Event::Axis(a), Event::Axis(b)) => a.variant_eq(b),
            (Event::Button(a), Event::Button(b)) => a.variant_eq(b),
            (Event::Trigger(a), Event::Trigger(b)) => a.variant_eq(b),
            _ => false,
        }
    }
}

/// Output events supported by the MSI Claw controller
#[derive(Clone, Debug)]
pub enum OutputEvent {
    Rumble { weak: u8, strong: u8 },
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug, Default)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Button events represend binary inputs
#[derive(Clone, Debug)]
pub enum ButtonEvent {
    /// A Button
    A(BinaryInput),
    /// X Button
    X(BinaryInput),
    /// B Button
    B(BinaryInput),
    /// Y Button
    Y(BinaryInput),
    /// Hamburger (☰) button
    Menu(BinaryInput),
    /// Overlapping square ⧉  button
    View(BinaryInput),
    /// DPad down
    DPadDown(BinaryInput),
    /// DPad up
    DPadUp(BinaryInput),
    /// DPad left
    DPadLeft(BinaryInput),
    /// DPad right
    DPadRight(BinaryInput),
    /// Left shoulder button
    LB(BinaryInput),
    /// Z-axis button on the left stick
    ThumbL(BinaryInput),
    /// Right shoulder button
    RB(BinaryInput),
    /// Z-axis button on the right stick
    ThumbR(BinaryInput),
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
            | Self::X(input)
            | Self::B(input)
            | Self::Y(input)
            | Self::Menu(input)
            | Self::View(input)
            | Self::DPadDown(input)
            | Self::DPadUp(input)
            | Self::DPadLeft(input)
            | Self::DPadRight(input)
            | Self::LB(input)
            | Self::ThumbL(input)
            | Self::RB(input)
            | Self::ThumbR(input) => input.pressed,
        }
    }
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug, Default)]
pub struct JoyAxisInput {
    pub x: u8,
    pub y: u8,
}

/// Axis events are events that have (x, y) values
#[derive(Clone, Debug)]
pub enum AxisEvent {
    LStick(JoyAxisInput),
    RStick(JoyAxisInput),
}

impl AxisEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

/// Trigger input contains non-negative integars
#[derive(Clone, Debug, Default)]
pub struct TriggerInput {
    pub value: u8,
}

/// Trigger events contain values indicating how far a trigger is pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    ATriggerL(TriggerInput),
    ATriggerR(TriggerInput),
}

impl TriggerEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}
