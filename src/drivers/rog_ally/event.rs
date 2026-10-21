/// Output events supported by the ROG Ally X controller
#[derive(Clone, Debug)]
pub enum OutputEvent {
    /// Extended rumble magnitudes in the device 0-100 range, in report order
    RumbleExtended {
        left_trigger: u8,
        right_trigger: u8,
        left_motor: u8,
        right_motor: u8,
    },
}
