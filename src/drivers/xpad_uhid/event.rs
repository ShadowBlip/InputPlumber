use std::mem;

/// Events that can be emitted by the XPad UHID controller
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
    Axis(AxisEvent),
    Trigger(TriggerEvent),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Event::Button(a), Event::Button(b)) => a.variant_eq(b),
            (Event::Axis(a), Event::Axis(b)) => a.variant_eq(b),
            (Event::Trigger(a), Event::Trigger(b)) => a.variant_eq(b),
            _ => false,
        }
    }
}

/// Output events supported by the XPad controller
#[derive(Clone, Debug)]
pub enum OutputEvent {
    Rumble { weak: u8, strong: u8 },
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug, Default)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug, Default)]
pub struct JoyAxisInput {
    pub x: u16,
    pub y: u16,
}

/// Trigger input contains non-negative integars
#[derive(Clone, Debug, Default)]
pub struct TriggerInput {
    pub value: u16,
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
    /// Right shoulder button
    RB(BinaryInput),
    /// Left shoulder button
    LB(BinaryInput),
    /// View ⧉  button
    View(BinaryInput),
    /// Menu (☰) button
    Menu(BinaryInput),
    /// Guide button
    Guide(BinaryInput),
    /// Z-axis button on the left stick
    ThumbL(BinaryInput),
    /// Z-axis button on the right stick
    ThumbR(BinaryInput),
    /// DPad up
    DPadUp(BinaryInput),
    /// DPad right
    DPadRight(BinaryInput),
    /// DPad down
    DPadDown(BinaryInput),
    /// DPad left
    DPadLeft(BinaryInput),
}

/// Axis events are events that have (x, y) values
#[derive(Clone, Debug)]
pub enum AxisEvent {
    LStick(JoyAxisInput),
    RStick(JoyAxisInput),
}

/// Trigger events contain values indicating how far a trigger is pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    TriggerL(TriggerInput),
    TriggerR(TriggerInput),
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
            | Self::RB(input)
            | Self::LB(input)
            | Self::View(input)
            | Self::Menu(input)
            | Self::Guide(input)
            | Self::ThumbL(input)
            | Self::ThumbR(input)
            | Self::DPadUp(input)
            | Self::DPadRight(input)
            | Self::DPadDown(input)
            | Self::DPadLeft(input) => input.pressed,
        }
    }
}

impl AxisEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

impl TriggerEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}
