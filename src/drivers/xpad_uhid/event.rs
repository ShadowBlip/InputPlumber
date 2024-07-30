/// Events that can be emitted by the controller
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
    Axis(AxisEvent),
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
    pub x: u16,
    pub y: u16,
}

/// Trigger input contains non-negative integars
#[derive(Clone, Debug)]
pub struct TriggerInput {
    pub value: u16,
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
    /// Guide button
    Guide(BinaryInput),
    /// Z-axis button on the left stick
    ThumbL(BinaryInput),
    /// Z-axis button on the right stick
    ThumbR(BinaryInput),
    /// DPad up
    DPadUp(BinaryInput),
    /// DPad right
    DPadRight(BinaryInput),
    /// DPad down
    DPadDown(BinaryInput),
    /// DPad left
    DPadLeft(BinaryInput),
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
