use std::mem;

/// Events that can be emitted by the Ultimate 2 controller
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
    Axis(AxisEvent),
    Inertia(InertialEvent),
    Trigger(TriggerEvent),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Event::Button(a), Event::Button(b)) => a.variant_eq(b),
            (Event::Axis(a), Event::Axis(b)) => a.variant_eq(b),
            (Event::Inertia(a), Event::Inertia(b)) => a.variant_eq(b),
            (Event::Trigger(a), Event::Trigger(b)) => a.variant_eq(b),
            _ => false,
        }
    }
}

/// Output events supported by the Ultimate 2 controller
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
    pub x: u8,
    pub y: u8,
}

/// Trigger input contains non-negative integars
#[derive(Clone, Debug, Default)]
pub struct TriggerInput {
    pub value: u8,
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
    /// Left shoulder button
    L1(BinaryInput),
    /// Right shoulder button
    R1(BinaryInput),
    /// View ⧉  button
    View(BinaryInput),
    /// Menu (☰) button
    Menu(BinaryInput),
    /// Guide button
    Guide(BinaryInput),
    /// Z-axis button on the left stick
    L2(BinaryInput),
    /// Z-axis button on the right stick
    R2(BinaryInput),
    /// DPad up
    DPadUp(BinaryInput),
    /// DPad right
    DPadRight(BinaryInput),
    /// DPad down
    DPadDown(BinaryInput),
    /// DPad left
    DPadLeft(BinaryInput),
    /// Paddle button on the left side
    L3(BinaryInput),
    /// Paddle button on the right side
    R3(BinaryInput),
    /// Small shoulder button on the left side
    L4(BinaryInput),
    /// Small shoulder button on the right side
    R4(BinaryInput),
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
            | Self::L1(input)
            | Self::R1(input)
            | Self::View(input)
            | Self::Menu(input)
            | Self::Guide(input)
            | Self::L2(input)
            | Self::R2(input)
            | Self::DPadUp(input)
            | Self::DPadRight(input)
            | Self::DPadDown(input)
            | Self::DPadLeft(input)
            | Self::L3(input)
            | Self::R3(input)
            | Self::L4(input)
            | Self::R4(input) => input.pressed,
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
