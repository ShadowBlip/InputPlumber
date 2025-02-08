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
    #[packed_field(bits = "23")]
    pub y1: bool,
    #[packed_field(bits = "22")]
    pub y2: bool,
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

    // byte 8
    #[packed_field(bits = "71")]
    pub rpad_touching: bool,
    // byte 9
    #[packed_field(bits = "79")]
    pub rpad_tap: bool,

    #[packed_field(bytes = "10")]
    pub touch_x: i8,
    #[packed_field(bytes = "11")]
    pub touch_y: i8,

    #[packed_field(bytes = "12")]
    pub a_trigger_l: u8,
    #[packed_field(bytes = "13")]
    pub a_trigger_r: u8,
}

impl XInputDataReport {
    /// Determines if the current report matches the bad data report.
    pub fn is_bad_data(&self) -> bool {
        *self == bad_data
    }
}

/// Signature of the bad data generated when grabbing the gyro device.
const bad_data: XInputDataReport = XInputDataReport {
    y1: false,
    y2: true,
    view: false,
    menu: false,
    a: false,
    b: true,
    x: false,
    y: false,
    lb: false,
    d_trigger_l: false,
    rb: false,
    d_trigger_r: false,
    legion: false,
    quick_access: true,
    thumb_l: false,
    thumb_r: false,
    up: false,
    down: false,
    left: false,
    right: false,
    l_stick_x: 2,
    l_stick_y: 10,
    r_stick_x: 0,
    r_stick_y: 0,
    rpad_touching: false,
    rpad_tap: true,
    touch_x: 0,
    touch_y: 16,
    a_trigger_l: 39,
    a_trigger_r: 240,
};

//InertialData
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "9")]
pub struct InertialInputDataReport {
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
