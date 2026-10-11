use std::mem;

/// Events that can be emitted by the controller
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
    Inertia(InertialEvent),
    Joystick(JoystickEvent),
    Trigger(TriggerEvent),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Event::Button(a), Event::Button(b)) => a.variant_eq(b),
            (Event::Inertia(a), Event::Inertia(b)) => a.variant_eq(b),
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
    /// Steam button
    Steam(BinaryInput),
    /// ... button
    Quick(BinaryInput),
    // Left Paddle 1
    L4(BinaryInput),
    // Right Paddle 1
    L5(BinaryInput),
    // Left Paddle 2
    R4(BinaryInput),
    // Right Paddle 2
    R5(BinaryInput),
    // C | Left Paddle 3
    M1(BinaryInput),
    // Z | Right Paddle 3
    M2(BinaryInput),
    /// Z-axis button on the left stick
    LSClick(BinaryInput),
    /// Z-axis button on the right stick
    RSClick(BinaryInput),
    // Digital TriggerEvent Left
    LTDigital(BinaryInput),
    // Digital TriggerEvent Right
    RTDigital(BinaryInput),
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
            | Self::X(input)
            | Self::B(input)
            | Self::Y(input)
            | Self::RB(input)
            | Self::LB(input)
            | Self::View(input)
            | Self::Menu(input)
            | Self::Steam(input)
            | Self::Quick(input)
            | Self::L4(input)
            | Self::L5(input)
            | Self::R4(input)
            | Self::R5(input)
            | Self::M1(input)
            | Self::M2(input)
            | Self::LSClick(input)
            | Self::RSClick(input)
            | Self::LTDigital(input)
            | Self::RTDigital(input)
            | Self::DPadUp(input)
            | Self::DPadRight(input)
            | Self::DPadDown(input)
            | Self::DPadLeft(input) => input.pressed,
        }
    }
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

impl InertialEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

/// [JoystickInput] is a double  (x, y) axis
#[derive(Clone, Debug, Default)]
pub struct JoystickInput {
    pub x: u8,
    pub y: u8,
}

/// [JoystickEvent] are events that have (x, y) values in the absolute domain indicating how far
/// left/right (x) and up/down (y) the joystick is off center.
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

/// [TriggerEvent] contains values indicating how far an analog trigger is pulled
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
