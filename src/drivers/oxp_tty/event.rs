/// Events that can be emitted by the Legion Go controller
#[derive(Clone, Debug)]
pub enum Event {
    GamepadButton(GamepadButtonEvent),
    Axis(AxisEvent),
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
pub struct JoyAxisInput {
    pub x: i16,
    pub y: i16,
}

/// Trigger input contains non-negative integars
#[derive(Clone, Debug)]
pub struct TriggerInput {
    pub value: u8,
}

/// Button events represend binary inputs
#[derive(Clone, Debug)]
pub enum GamepadButtonEvent {
    /// No button (Not a real button, placeholder
    /// as buttons may be mapped to nothing in the report)
    None,
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
    /// Right shoulder button
    RB(BinaryInput),
    /// Binary sensor for left analog trigger
    TriggerL(BinaryInput),
    /// Binary sensor for right analog trigger
    TriggerR(BinaryInput),
    /// Z-axis button on the left stick
    ThumbL(BinaryInput),
    /// Z-axis button on the right stick
    ThumbR(BinaryInput),
    /// M1 on the top left of the controller
    M1(BinaryInput),
    /// M2 on the top right of the controller
    M2(BinaryInput),
    /// Keyboard button (X1/G1) devices
    Keyboard(BinaryInput),
}

/// Axis events are events that have (x, y) values
#[derive(Clone, Debug)]
pub enum AxisEvent {
    LStick(JoyAxisInput),
    RStick(JoyAxisInput),
    TriggerL(TriggerInput),
    TriggerR(TriggerInput),
}
