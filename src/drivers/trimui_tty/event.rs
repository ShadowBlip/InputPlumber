//! Decoded TrimUI MCU events. The driver emits one [`Event::Buttons`]
//! per frame with edge information, plus the side-selected raw stick axes.

/// Events decoded from TrimUI MCU frames.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    Buttons { buttons: u32, changed: u32 },
    Stick { x: u16, y: u16 },
}
