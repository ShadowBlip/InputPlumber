/// Events that can be emitted by the GPD Win Mini drivers
#[derive(Clone, Debug)]
pub enum Event {
    TouchAxis(TouchAxisEvent),
    TouchButton(TouchButtonEvent),
    GamepadButton(GamepadButtonEvent),
    Trigger(TriggerEvent),
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
pub struct TouchAxisEvent {
    pub index: u8,
    pub is_touching: bool,
    pub x: u16,
    pub y: u16,
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Trigger input contains non-negative integers
#[derive(Clone, Debug)]
pub struct TriggerInput {
    pub value: u8,
}

/// TouchButton events represent binary clicks
#[derive(Clone, Debug)]
pub enum TouchButtonEvent {
    /// Tap to click button
    Left(BinaryInput),
}

/// Trigger events contain values indicating how far a trigger is pulled
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    PadForce(TriggerInput),
}

/// GamepadButton events represent binary button presses
#[derive(Clone, Debug)]
pub enum GamepadButtonEvent {
    /// Tap to click button
    L4(BinaryInput),
    R4(BinaryInput),
}
