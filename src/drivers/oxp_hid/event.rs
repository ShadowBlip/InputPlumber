use std::mem;

/// Events that can be emitted by the OXP HID driver
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
}

impl Event {
    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Event::Button(a), Event::Button(b)) => a.variant_eq(b),
        }
    }
}

/// Binary input contains either pressed or unpressed
#[derive(Clone, Debug, Default)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// Button events from vendor HID report mode.
/// Only extra buttons (M1/M2/Keyboard/Guide) are reported — standard gamepad
/// buttons come through the Xbox gamepad's own evdev device.
#[derive(Clone, Debug)]
pub enum ButtonEvent {
    /// M1 back paddle
    M1(BinaryInput),
    /// M2 back paddle
    M2(BinaryInput),
    /// Keyboard button
    Keyboard(BinaryInput),
    /// Guide/Home button
    Guide(BinaryInput),
}

impl ButtonEvent {
    /// Whether the button is pressed
    pub fn pressed(&self) -> bool {
        match self {
            Self::M1(input) | Self::M2(input) | Self::Keyboard(input) | Self::Guide(input) => {
                input.pressed
            }
        }
    }

    /// Whether two events are the same type, ignoring payload data
    pub fn variant_eq(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}
