use packed_struct::prelude::*;

use super::{STICK_DEADZONE, TRIGGER_DEADZONE};

/// Parsed state of one 8-byte MOGA XP5-i Plus input report.
///
/// The report has no report ID:
///
/// | byte | meaning                                        |
/// |------|------------------------------------------------|
/// | 0    | left stick X, signed int8                      |
/// | 1    | left stick Y, signed int8                      |
/// | 2    | right stick X, signed int8                     |
/// | 3    | right stick Y, signed int8                     |
/// | 4    | left trigger, unsigned int8                    |
/// | 5    | right trigger, unsigned int8                   |
/// | 6    | d-pad (up, right, down, left) and A, B, X, Y   |
/// | 7    | bumpers, stick clicks, home, view, start       |
#[derive(PackedStruct, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "8")]
pub struct MogaReport {
    #[packed_field(bytes = "0")]
    pub left_stick_x: i8,
    #[packed_field(bytes = "1")]
    pub left_stick_y: i8,
    #[packed_field(bytes = "2")]
    pub right_stick_x: i8,
    #[packed_field(bytes = "3")]
    pub right_stick_y: i8,
    #[packed_field(bytes = "4")]
    pub left_trigger: u8,
    #[packed_field(bytes = "5")]
    pub right_trigger: u8,
    #[packed_field(bits = "55")]
    pub dpad_up: bool,
    #[packed_field(bits = "54")]
    pub dpad_right: bool,
    #[packed_field(bits = "53")]
    pub dpad_down: bool,
    #[packed_field(bits = "52")]
    pub dpad_left: bool,
    #[packed_field(bits = "51")]
    pub a: bool,
    #[packed_field(bits = "50")]
    pub b: bool,
    #[packed_field(bits = "49")]
    pub x: bool,
    #[packed_field(bits = "48")]
    pub y: bool,
    #[packed_field(bits = "63")]
    pub lb: bool,
    #[packed_field(bits = "62")]
    pub rb: bool,
    #[packed_field(bits = "61")]
    pub l3: bool,
    #[packed_field(bits = "60")]
    pub r3: bool,
    #[packed_field(bits = "59")]
    pub guide: bool,
    #[packed_field(bits = "58")]
    pub select: bool,
    #[packed_field(bits = "57")]
    pub start: bool,
}

impl MogaReport {
    /// Unpack one input report and apply the stick and trigger deadzones.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let bytes: [u8; 8] = data.get(..8)?.try_into().ok()?;
        let mut report = Self::unpack(&bytes).ok()?;
        report.apply_deadzones();
        Some(report)
    }

    /// Zero centered stick values and resting trigger values.
    fn apply_deadzones(&mut self) {
        self.left_stick_x = deadzone_stick(self.left_stick_x);
        self.left_stick_y = deadzone_stick(self.left_stick_y);
        self.right_stick_x = deadzone_stick(self.right_stick_x);
        self.right_stick_y = deadzone_stick(self.right_stick_y);
        self.left_trigger = deadzone_trigger(self.left_trigger);
        self.right_trigger = deadzone_trigger(self.right_trigger);
    }
}

/// Apply the stick deadzone and rescale the remaining range to -127..127.
fn deadzone_stick(value: i8) -> i8 {
    let magnitude = value.unsigned_abs();
    if magnitude <= STICK_DEADZONE as u8 {
        return 0;
    }

    let scaled = (magnitude as i16 - STICK_DEADZONE as i16) * 127 / (127 - STICK_DEADZONE as i16);
    let scaled = scaled.min(127) as i8;

    if value < 0 {
        -scaled
    } else {
        scaled
    }
}

/// Apply the trigger deadzone and rescale the remaining range to 0..255.
fn deadzone_trigger(value: u8) -> u8 {
    if value <= TRIGGER_DEADZONE {
        return 0;
    }

    let scaled = (value - TRIGGER_DEADZONE) as u16 * 255 / (255 - TRIGGER_DEADZONE) as u16;
    scaled.min(255) as u8
}
