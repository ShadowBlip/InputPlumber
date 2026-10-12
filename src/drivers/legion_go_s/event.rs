use std::mem;

/// Events that can be emitted by the Legion Go S controllers
#[derive(Clone, Debug)]
pub enum Event {
    Axis(AxisEvent),
    Button(ButtonEvent),
    Inertia(InertialEvent),
    Trigger(TriggerEvent),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Event::Axis(a), Event::Axis(b)) => a.variant_eq(b),
            (Event::Button(a), Event::Button(b)) => a.variant_eq(b),
            (Event::Inertia(a), Event::Inertia(b)) => a.variant_eq(b),
            (Event::Trigger(a), Event::Trigger(b)) => a.variant_eq(b),
            _ => false,
        }
    }
}

/// Output events supported by the Legion Go S XInput controller
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
    /// Legion button on left controller
    Legion(BinaryInput),
    /// Quick Access button on right controller
    QuickAccess(BinaryInput),
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
    /// Binary sensor for left analog trigger
    DTriggerL(BinaryInput),
    /// Z-axis button on the left stick
    ThumbL(BinaryInput),
    /// Y1 left back paddle
    Y1(BinaryInput),
    /// Y2 right back paddle
    Y2(BinaryInput),
    /// Right shoulder button
    RB(BinaryInput),
    /// Binary sensor for right analog trigger
    DTriggerR(BinaryInput),
    /// Z-axis button on the right stick
    ThumbR(BinaryInput),
    /// Right touchpad click
    RPadPress(BinaryInput),
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug, Default)]
pub struct TouchAxisInput {
    pub index: u8,
    pub is_touching: bool,
    pub x: u16,
    pub y: u16,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug, Default)]
pub struct JoyAxisInput {
    pub x: i8,
    pub y: i8,
}

/// Axis events are events that have (x, y) values
#[derive(Clone, Debug)]
pub enum AxisEvent {
    Touchpad(TouchAxisInput),
    LStick(JoyAxisInput),
    RStick(JoyAxisInput),
}

/// [InertialInput] represents the state of the IMU (x, y, z) values
#[derive(Clone, Debug, Default)]
pub struct InertialInput {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

/// [InertialEvent] has data from the IMU
#[derive(Clone, Debug)]
pub enum InertialEvent {
    Accelerometer(InertialInput),
    Gyroscope(InertialInput),
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
    RpadForce(TriggerInput),
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
            | Self::Legion(input)
            | Self::QuickAccess(input)
            | Self::DPadDown(input)
            | Self::DPadUp(input)
            | Self::DPadLeft(input)
            | Self::DPadRight(input)
            | Self::LB(input)
            | Self::DTriggerL(input)
            | Self::ThumbL(input)
            | Self::Y1(input)
            | Self::Y2(input)
            | Self::RB(input)
            | Self::DTriggerR(input)
            | Self::ThumbR(input)
            | Self::RPadPress(input) => input.pressed,
        }
    }
}

impl AxisEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

impl InertialEvent {
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
