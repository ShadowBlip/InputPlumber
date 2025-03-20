//! Reference: https://gitlab.com/open-sd/opensd/-/blob/main/src/opensdd/drivers/gamepad/hid_reports.hpp
#![allow(warnings)]
use packed_struct::prelude::*;

/// Different reports types
pub enum InputReportType {
    AccelData = 0x01,
    GyroData = 0x02,
}

impl InputReportType {
    pub fn to_u8(&self) -> u8 {
        match self {
            InputReportType::AccelData => InputReportType::AccelData as u8,
            InputReportType::GyroData => InputReportType::GyroData as u8,
        }
    }
}

pub enum OutputReportType {
    RumbleData = 0x04,
}

impl OutputReportType {
    pub fn to_u8(&self) -> u8 {
        match self {
            &OutputReportType::RumbleData => OutputReportType::RumbleData as u8,
        }
    }
}

//XInputData
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "32")]
pub struct XInputDataReport {
    // byte 0
    #[packed_field(bits = "0")]
    pub right: bool,
    #[packed_field(bits = "1")]
    pub left: bool,
    #[packed_field(bits = "2")]
    pub down: bool,
    #[packed_field(bits = "3")]
    pub up: bool,
    #[packed_field(bits = "4")]
    pub thumb_r: bool,
    #[packed_field(bits = "5")]
    pub thumb_l: bool,
    #[packed_field(bits = "6")]
    pub quick_access: bool,
    #[packed_field(bits = "7")]
    pub legion: bool,
    // byte 1
    #[packed_field(bits = "8")]
    pub d_trigger_r: bool,
    #[packed_field(bits = "9")]
    pub rb: bool,
    #[packed_field(bits = "10")]
    pub d_trigger_l: bool,
    #[packed_field(bits = "11")]
    pub lb: bool,
    #[packed_field(bits = "12")]
    pub y: bool,
    #[packed_field(bits = "13")]
    pub x: bool,
    #[packed_field(bits = "14")]
    pub b: bool,
    #[packed_field(bits = "15")]
    pub a: bool,
    // byte 2
    #[packed_field(bits = "16")]
    pub menu: bool,
    #[packed_field(bits = "17")]
    pub view: bool,
    //byte 3
    #[packed_field(bits = "22")]
    pub y2: bool,
    #[packed_field(bits = "23")]
    pub y1: bool,
    // byte 4
    #[packed_field(bytes = "4")]
    pub l_stick_x: i8,
    // byte 5
    #[packed_field(bytes = "5")]
    pub l_stick_y: i8,
    // byte 6
    #[packed_field(bytes = "6")]
    pub r_stick_x: i8,
    // byte 7
    #[packed_field(bytes = "7")]
    pub r_stick_y: i8,
    // byte 8
    #[packed_field(bits = "71")] // Deprecated
    pub rpad_touching: bool,
    // byte 9
    #[packed_field(bits = "79")] // Deprecated
    pub rpad_tap: bool,
    // byte 10
    #[packed_field(bytes = "10")]
    pub touch_x: i8,
    // byte 11
    #[packed_field(bytes = "11")]
    pub touch_y: i8,
    // byte 12
    #[packed_field(bytes = "12")]
    pub a_trigger_l: u8,
    // byte 13
    #[packed_field(bytes = "13")]
    pub a_trigger_r: u8,
}

//InertialData
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "9")]
pub struct InertialDataReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    #[packed_field(bytes = "1")]
    pub sensor_state: u8,
    #[packed_field(bytes = "2")]
    pub sensor_event: u8,
    #[packed_field(bytes = "3..=4", endian = "lsb")]
    pub x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "5..=6", endian = "lsb")]
    pub y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub z: Integer<i16, packed_bits::Bits<16>>,
}

//TouchData
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "10")]
pub struct TouchpadDataReport {
    // byte 0
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    // byte 1
    #[packed_field(bits = "8..11")]
    pub contact_id: u8,
    #[packed_field(bits = "14")]
    pub tip_switch: bool,
    #[packed_field(bits = "15")]
    pub confidence: bool,
    //// byte 2 - 3
    #[packed_field(bytes = "2..=3", endian = "lsb")]
    pub touch_x: u16,
    // byte 4 - 5
    #[packed_field(bytes = "4..=5", endian = "lsb")]
    pub touch_y: u16,
    // bytes 6-7
    #[packed_field(bytes = "6..=7", endian = "lsb")]
    pub scan_time: u16,
    // byte 8
    #[packed_field(bytes = "8")]
    pub contact_count: u8,
    // byte 9
    #[packed_field(bits = "79")]
    pub pressed: bool,
}
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "9")]
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
    pub l_motor_speed: u8,
    #[packed_field(bytes = "5")]
    pub r_motor_speed: u8,
    #[packed_field(bytes = "6")]
    pub work_mode: u8,
    #[packed_field(bytes = "7")]
    pub l_motor_feature: u8,
    #[packed_field(bytes = "8")]
    pub r_motor_feature: u8,
}

impl Default for RumbleOutputDataReport {
    fn default() -> Self {
        Self {
            report_id: OutputReportType::RumbleData.to_u8(),
            unk_1: 0x00,
            unk_2: 0x08,
            unk_3: 0x00,
            l_motor_speed: 0x00,
            r_motor_speed: 0x00,
            work_mode: 0x00,
            l_motor_feature: 0x00,
            r_motor_feature: 0x00,
        }
    }
}
