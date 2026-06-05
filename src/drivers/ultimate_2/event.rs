/// Events that can be emitted by the controller
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
    Axis(AxisEvent),
    Inertia(InertialEvent),
    Trigger(TriggerEvent),
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
pub struct JoyAxisInput {
    pub x: u8,
    pub y: u8,
}

/// Trigger input contains non-negative integars
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
pub struct InertialInput {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

/// [InertialEvent] has data from the IMU
#[derive(Clone, Debug)]
pub enum InertialEvent {
    Accelerometer(InertialInput),
    Gyro(InertialInput),
}
