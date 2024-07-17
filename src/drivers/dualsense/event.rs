/// Events that can be emitted by the DualSense controller
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
    Accelerometer(AccelerometerEvent),
    Axis(AxisEvent),
    Trigger(TriggerEvent),
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug)]
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

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
pub struct AxisInput {
    pub x: u8,
    pub y: u8,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
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

/// Trigger input contains non-negative integars
#[derive(Clone, Debug)]
pub struct TriggerInput {
    pub value: u8,
}

/// Trigger events contain positive values indicating how far a trigger is pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    L2(TriggerInput),
    R2(TriggerInput),
}

/// AccelerometerInput represents the state of the accelerometer (x, y, z) values
#[derive(Clone, Debug)]
pub struct AccelerometerInput {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

/// AccelerometerEvent has data from the accelerometer
#[derive(Clone, Debug)]
pub enum AccelerometerEvent {
    Accelerometer(AccelerometerInput),
    /// Pitch, yaw, roll
    Gyro(AccelerometerInput),
}
