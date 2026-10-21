use packed_struct::prelude::*;

use super::RUMBLE_REPORT_ID;

/*
 * Extended rumble output report for the ROG Ally X gamepad interface. This is
 * the Xbox wireless controller rumble block with a different report ID and
 * two additional trigger motor magnitude fields.
 * Magnitudes range 0-100 and duration and delay are in 10ms units.
 */
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0")]
pub struct PackedRumbleExtendedOutputReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    #[packed_field(bytes = "1")]
    pub enable: u8,
    #[packed_field(bytes = "2")]
    pub magnitude_left_trigger: u8,
    #[packed_field(bytes = "3")]
    pub magnitude_right_trigger: u8,
    #[packed_field(bytes = "4")]
    pub magnitude_left: u8,
    #[packed_field(bytes = "5")]
    pub magnitude_right: u8,
    #[packed_field(bytes = "6")]
    pub duration_10ms: u8,
    #[packed_field(bytes = "7")]
    pub delay_10ms: u8,
    #[packed_field(bytes = "8")]
    pub loop_count: u8,
}

impl PackedRumbleExtendedOutputReport {
    /// All actuators off, matching the kernel force feedback off report
    pub fn off() -> Self {
        Self {
            report_id: RUMBLE_REPORT_ID,
            enable: 0x0f,
            magnitude_left_trigger: 0,
            magnitude_right_trigger: 0,
            magnitude_left: 0,
            magnitude_right: 0,
            duration_10ms: 0xff,
            delay_10ms: 0,
            loop_count: 0xeb,
        }
    }
}
