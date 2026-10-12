//! Decoded TrimUI MCU events
#[derive(Clone, Copy, Debug)]
pub enum Event {
    Buttons { buttons: u32, changed: u32 },
    Stick { x: u16, y: u16 },
}
