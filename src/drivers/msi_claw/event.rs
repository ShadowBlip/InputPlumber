/// Events that can be emitted by the Legion Go controller
#[derive(Clone, Debug)]
pub enum Event {
    Axis(AxisEvent),
    Button(ButtonEvent),
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
    /// Z-axis button on the left stick
    ThumbL(BinaryInput),
    /// Right shoulder button
    RB(BinaryInput),
    /// Z-axis button on the right stick
    ThumbR(BinaryInput),
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
pub struct JoyAxisInput {
    pub x: u8,
    pub y: u8,
}

/// Axis events are events that have (x, y) values
#[derive(Clone, Debug)]
pub enum AxisEvent {
    LStick(JoyAxisInput),
    RStick(JoyAxisInput),
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
}
