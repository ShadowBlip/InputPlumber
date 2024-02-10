/// Events that can be emitted by the Steam Deck controller
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
    /// A Button
    A(BinaryInput),
    /// X Button
    X(BinaryInput),
    /// B Button
    B(BinaryInput),
    /// Y Button
    Y(BinaryInput),
    /// Hamburger (☰) button located above right stick
    Menu(BinaryInput),
    /// Overlapping square ⧉  button located above left stick
    Options(BinaryInput),
    /// STEAM button below left trackpad
    Steam(BinaryInput),
    /// Quick Access (...) button below right trackpad
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
    L1(BinaryInput),
    /// Binary sensor for left analog trigger
    L2(BinaryInput),
    /// Z-axis button on the left stick
    L3(BinaryInput),
    /// L4 on the back of the deck
    L4(BinaryInput),
    /// L5 on the back of the deck
    L5(BinaryInput),
    /// Right shoulder button
    R1(BinaryInput),
    /// Binary sensor for right analog trigger
    R2(BinaryInput),
    /// Z-axis button on the right stick
    R3(BinaryInput),
    /// R4 on the back of the deck
    R4(BinaryInput),
    /// R5 on the back of the deck
    R5(BinaryInput),
    /// Binary "touch" sensor for right trackpad
    RPadTouch(BinaryInput),
    /// Binary "touch" sensor for left trackpad
    LPadTouch(BinaryInput),
    /// Binary "press" sensor for right trackpad
    RPadPress(BinaryInput),
    /// Binary "press" sensor for left trackpad
    LPadPress(BinaryInput),
    /// Binary touch sensors on the right control stick
    RStickTouch(BinaryInput),
    /// Binary touch sensors on the left control stick
    LStickTouch(BinaryInput),
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
pub struct AxisInput {
    pub x: i16,
    pub y: i16,
}

/// Axis events are events that have (x, y) values
#[derive(Clone, Debug)]
pub enum AxisEvent {
    LPad(AxisInput),
    RPad(AxisInput),
    LStick(AxisInput),
    RStick(AxisInput),
}

/// Trigger input contains non-negative integars
#[derive(Clone, Debug)]
pub struct TriggerInput {
    pub value: u16,
}

/// Trigger events contain positive values indicating how far a trigger is pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    LTrigger(TriggerInput),
    RTrigger(TriggerInput),
    LPadForce(TriggerInput),
    RPadForce(TriggerInput),
    LStickForce(TriggerInput),
    RStickForce(TriggerInput),
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
    Attitude(AccelerometerInput),
}
