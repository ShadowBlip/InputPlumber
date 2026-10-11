use std::mem;

use super::hid_report::SetStatePackedOutputData;

/// Events that can be emitted by the DualSense controller
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
    Inertia(InertialEvent),
    Axis(AxisEvent),
    Trigger(TriggerEvent),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Event::Button(a), Event::Button(b)) => a.variant_eq(b),
            (Event::Inertia(a), Event::Inertia(b)) => a.variant_eq(b),
            (Event::Axis(a), Event::Axis(b)) => a.variant_eq(b),
            (Event::Trigger(a), Event::Trigger(b)) => a.variant_eq(b),
            _ => false,
        }
    }
}

/// Output events supported by the DualSense controller
#[derive(Clone, Debug)]
pub enum OutputEvent {
    /// Rumble the left and right haptic actuators, driving the adaptive
    /// triggers on the DS5.
    HapticRumble { left: u8, right: u8 },
    /// Write a full DualSense state report
    State(SetStatePackedOutputData),
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug, Default)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Button events represend binary inputs
#[derive(Clone, Debug)]
pub enum ButtonEvent {
    Cross(BinaryInput),
    Circle(BinaryInput),
    Square(BinaryInput),
    Triangle(BinaryInput),
    Create(BinaryInput),
    Options(BinaryInput),
    Guide(BinaryInput),
    Mute(BinaryInput),
    DPadDown(BinaryInput),
    DPadUp(BinaryInput),
    DPadLeft(BinaryInput),
    DPadRight(BinaryInput),
    L1(BinaryInput),
    L2(BinaryInput),
    L3(BinaryInput),
    L4(BinaryInput),
    L5(BinaryInput),
    R1(BinaryInput),
    R2(BinaryInput),
    R3(BinaryInput),
    R4(BinaryInput),
    R5(BinaryInput),
    PadPress(BinaryInput),
}

impl ButtonEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }

    /// Whether the button is pressed
    pub fn pressed(&self) -> bool {
        match self {
            Self::Cross(input)
            | Self::Circle(input)
            | Self::Square(input)
            | Self::Triangle(input)
            | Self::Create(input)
            | Self::Options(input)
            | Self::Guide(input)
            | Self::Mute(input)
            | Self::DPadDown(input)
            | Self::DPadUp(input)
            | Self::DPadLeft(input)
            | Self::DPadRight(input)
            | Self::L1(input)
            | Self::L2(input)
            | Self::L3(input)
            | Self::L4(input)
            | Self::L5(input)
            | Self::R1(input)
            | Self::R2(input)
            | Self::R3(input)
            | Self::R4(input)
            | Self::R5(input)
            | Self::PadPress(input) => input.pressed,
        }
    }
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug, Default)]
pub struct AxisInput {
    pub x: u8,
    pub y: u8,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug, Default)]
pub struct TouchAxisInput {
    pub index: u8,
    pub is_touching: bool,
    pub x: u16,
    pub y: u16,
}

/// Axis events are events that have (x, y) values
#[derive(Clone, Debug)]
pub enum AxisEvent {
    Pad(TouchAxisInput),
    LStick(AxisInput),
    RStick(AxisInput),
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

/// Trigger events contain positive values indicating how far a trigger is pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    L2(TriggerInput),
    R2(TriggerInput),
}

impl TriggerEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

/// InertialInput represents the state of the IMU (x, y, z) values, calibrated
/// to physical units by the driver
#[derive(Clone, Debug, Default)]
pub struct InertialInput {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// AccelerometerEvent has data from the accelerometer
#[derive(Clone, Debug)]
pub enum InertialEvent {
    Accelerometer(InertialInput),
    /// Pitch, yaw, roll
    Gyroscope(InertialInput),
}

impl InertialEvent {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}
