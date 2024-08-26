/// Events that can be emitted by the Steam Deck controller
#[derive(Clone, Debug)]
pub enum Event {
    TouchAxis(TouchAxisEvent),
    TouchButton(TouchButtonEvent),
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

/// Button events represend binary inputs
#[derive(Clone, Debug)]
pub enum TouchButtonEvent {
    /// Tap to click button
    Left(BinaryInput),
}
