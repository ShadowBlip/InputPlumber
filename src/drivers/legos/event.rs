/// Events that can be emitted by the Legion Go controller
#[derive(Clone, Debug)]
pub enum Event {
    Axis(AxisEvent),
    Button(ButtonEvent),
    Inertia(InertialEvent),
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
#[derive(Clone, Debug)]
pub struct TouchAxisInput {
    pub index: u8,
    pub is_touching: bool,
    pub x: u16,
    pub y: u16,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
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

/// Trigger input contains non-negative integars
#[derive(Clone, Debug)]
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
