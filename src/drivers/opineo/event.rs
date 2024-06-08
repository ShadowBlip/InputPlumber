/// Events that can be emitted by the Steam Deck controller
#[derive(Clone, Debug)]
pub enum Event {
    TouchAxis(TouchAxisInput),
}

/// Axis input contain (x, y) coordinates
#[derive(Clone, Debug)]
pub struct TouchAxisInput {
    pub index: u8,
    pub is_touching: bool,
    pub x: u16,
    pub y: u16,
}

/// TouchAxisID tracks the sequential count of touch inputs
#[derive(Clone, Debug)]
pub struct TouchAxisID {
    pub value: u32,
}
