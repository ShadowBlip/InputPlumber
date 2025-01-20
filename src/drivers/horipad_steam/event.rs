/// Events that can be emitted by the controller
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
    Inertia(InertialEvent),
    Joystick(JoystickEvent),
    Trigger(TriggerEvent),
}

/// [BinaryInput] contains either pressed or unpressed
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
    // M1 below the d-pad
    M1(BinaryInput),
    // M2 below the right stick
    M2(BinaryInput),
    // L4 behind
    L4(BinaryInput),
    // R4 behind
    R4(BinaryInput),
    /// Z-axis button on the left stick
    LSClick(BinaryInput),
    /// Z-axis button on the right stick
    RSClick(BinaryInput),
    // Capacitive touch on the left stick
    LSTouch(BinaryInput),
    // Capacitive touch on the right stick
    RSTouch(BinaryInput),
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

/// [JoystickInput] is a double  (x, y) axis
#[derive(Clone, Debug)]
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

/// [TriggerInput] is a single (z) axis
#[derive(Clone, Debug)]
pub struct TriggerInput {
    pub value: u8,
}

/// [TriggerEvent] contains values indicating how far an analog trigger is pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    LTAnalog(TriggerInput),
    RTAnalog(TriggerInput),
}
