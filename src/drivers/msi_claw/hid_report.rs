//! Reference: https://gitlab.com/open-sd/opensd/-/blob/main/src/opensdd/drivers/gamepad/hid_reports.hpp
#![allow(warnings)]
use packed_struct::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ReportId {
    InputData = 0x01,
    RumbleData = 0x05,
}

impl ReportId {
    pub fn to_u8(&self) -> u8 {
        match self {
            &ReportId::InputData => ReportId::InputData as u8,
            &ReportId::RumbleData => ReportId::RumbleData as u8,
        }
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum Direction {
    Up = 0,
    UpRight = 1,
    Right = 2,
    DownRight = 3,
    Down = 4,
    DownLeft = 5,
    Left = 6,
    UpLeft = 7,
    #[default]
    None = 15,
}

impl Direction {
    /// Translates the raw hardware hat-switch value into 4 independent button states
    /// Returns: (Up, Right, Down, Left)
    pub fn button_states(&self) -> (bool, bool, bool, bool) {
        match *self {
            Self::Up => (true, false, false, false),
            Self::UpRight => (true, true, false, false),
            Self::Right => (false, true, false, false),
            Self::DownRight => (false, true, true, false),
            Self::Down => (false, false, true, false),
            Self::DownLeft => (false, false, true, true),
            Self::Left => (false, false, false, true),
            Self::UpLeft => (true, false, false, true),
            Self::None => (false, false, false, false),
        }
    }
}

// Dinput  Data
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct InputDataReport {
    // byte 0
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    // byte 1
    #[packed_field(bytes = "1")]
    pub l_stick_x: u8,
    // byte 2
    #[packed_field(bytes = "2")]
    pub l_stick_y: u8,
    // byte 3
    #[packed_field(bytes = "3")]
    pub r_stick_x: u8,
    // byte 4
    #[packed_field(bytes = "4")]
    pub r_stick_y: u8,

    // byte 5
    #[packed_field(bits = "40")]
    pub button_y: bool,
    #[packed_field(bits = "41")]
    pub button_b: bool,
    #[packed_field(bits = "42")]
    pub button_a: bool,
    #[packed_field(bits = "43")]
    pub button_x: bool,
    #[packed_field(bits = "44..=47", ty = "enum")]
    pub dpad_dir: Direction,

    // byte 6
    #[packed_field(bits = "48")]
    pub button_r3: bool,
    #[packed_field(bits = "49")]
    pub button_l3: bool,
    #[packed_field(bits = "50")]
    pub button_menu: bool,
    #[packed_field(bits = "51")]
    pub button_view: bool,
    #[packed_field(bits = "52")]
    pub unused_52: bool,
    #[packed_field(bits = "53")]
    pub unused_53: bool,
    #[packed_field(bits = "54")]
    pub button_rb: bool,
    #[packed_field(bits = "55")]
    pub button_lb: bool,

    // byte 8
    #[packed_field(bytes = "8")]
    pub trigger_l: u8,
    // byte 9
    #[packed_field(bytes = "9")]
    pub trigger_r: u8,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "11")]
pub struct RumbleOutputDataReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    #[packed_field(bytes = "1")]
    pub unk_1: u8,
    #[packed_field(bytes = "2")]
    pub unk_2: u8,
    #[packed_field(bytes = "3")]
    pub unk_3: u8,
    #[packed_field(bytes = "4")]
    pub weak_magnitude: u8,
    #[packed_field(bytes = "5")]
    pub strong_magnitude: u8,
}

impl Default for RumbleOutputDataReport {
    fn default() -> Self {
        Self {
            report_id: ReportId::RumbleData.to_u8(),
            unk_1: 0x01,
            unk_2: Default::default(),
            unk_3: Default::default(),
            weak_magnitude: Default::default(),
            strong_magnitude: Default::default(),
        }
    }
}
