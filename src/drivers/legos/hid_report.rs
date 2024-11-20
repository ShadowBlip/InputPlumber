//! Reference: https://gitlab.com/open-sd/opensd/-/blob/main/src/opensdd/drivers/gamepad/hid_reports.hpp
#![allow(warnings)]
use packed_struct::prelude::*;

/// Different reports types
// When in some modes there's another report decriptor with the same ID
// as the touchpad whic is a keyboard with macros tied to different buttons.
// Not useful, I haven't enumerated this report here.
pub enum ReportType {
    XInputData = 0x00,   // Always available and always has access to all buttons
}

impl ReportType {
    pub fn to_u8(&self) -> u8 {
        match self {
            ReportType::XInputData => ReportType::XInputData as u8,
        }
    }
}
//XInputData
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "32")]
pub struct XInputDataReport {
    #[packed_field(bits = "23")]
    pub y1: bool,
    #[packed_field(bits = "22")]
    pub y3: bool,
    #[packed_field(bits = "17")]
    pub view: bool,
    #[packed_field(bits = "16")]
    pub menu: bool,
    #[packed_field(bits = "15")]
    pub a: bool,
    #[packed_field(bits = "14")]
    pub b: bool,
    #[packed_field(bits = "13")]
    pub x: bool,
    #[packed_field(bits = "12")]
    pub y: bool,
    #[packed_field(bits = "11")]
    pub lb: bool,
    #[packed_field(bits = "10")]
    pub d_trigger_l: bool,
    #[packed_field(bits = "9")]
    pub rb: bool,
    #[packed_field(bits = "8")]
    pub d_trigger_r: bool,
    #[packed_field(bits = "7")]
    pub legion: bool,
    #[packed_field(bits = "6")]
    pub quick_access: bool,
    #[packed_field(bits = "5")]
    pub thumb_l: bool,
    #[packed_field(bits = "4")]
    pub thumb_r: bool,
    #[packed_field(bits = "3")]
    pub up: bool,
    #[packed_field(bits = "2")]
    pub down: bool,
    #[packed_field(bits = "1")]
    pub left: bool,
    #[packed_field(bits = "0")]
    pub right: bool,

    #[packed_field(bytes = "4")]
    pub l_stick_x: i8,
    #[packed_field(bytes = "5")]
    pub l_stick_y: i8,
    #[packed_field(bytes = "6")]
    pub r_stick_x: i8,
    #[packed_field(bytes = "7")]
    pub r_stick_y: i8,

    #[packed_field(bytes = "12")]
    pub a_trigger_l: u8,
    #[packed_field(bytes = "13")]
    pub a_trigger_r: u8,
}
