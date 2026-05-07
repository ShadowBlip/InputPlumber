/// Events that can be emitted by the OXP HID driver
#[derive(Clone, Debug)]
pub enum Event {
    Button(ButtonEvent),
}

/// Binary input contains either pressed or unpressed
#[derive(Clone, Debug)]
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
