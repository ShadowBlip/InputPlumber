/// Events that can be emitted by the GPD Win 5 HID button driver
#[derive(Clone, Debug)]
pub enum Event {
    GamepadButton(GamepadButtonEvent),
}

/// Binary input contain either pressed or unpressed
#[derive(Clone, Debug)]
pub struct BinaryInput {
    pub pressed: bool,
}

/// GamepadButton events represent binary button presses
#[derive(Clone, Debug)]
pub enum GamepadButtonEvent {
    /// Quick Access / Mode switch button (short press)
    QuickAccess(BinaryInput),
    /// Left back button (L4)
    L4(BinaryInput),
    /// Right back button (R4)
    R4(BinaryInput),
}
