//! Reference: https://gitlab.com/open-sd/opensd/-/blob/main/src/opensdd/drivers/gamepad/hid_reports.hpp
#![allow(warnings)]
use futures::FutureExt;
use packed_struct::prelude::*;

use std::fmt::Display;
use std::fmt::Formatter;
/// Different reports types
// When in some modes there's another report decriptor with the same ID
// as the touchpad whic is a keyboard with macros tied to different buttons.
// Not useful, I haven't enumerated this report here.
pub enum ReportType {
    TouchpadData = 0x01,
    XInputData = 0x04, // Always available and always has access to all buttons
}

impl ReportType {
    pub fn to_u8(&self) -> u8 {
        match self {
            ReportType::TouchpadData => ReportType::TouchpadData as u8,
            ReportType::XInputData => ReportType::XInputData as u8,
        }
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum DPadDirection {
    Up = 0,
    UpRight = 1,
    Right = 2,
    DownRight = 3,
    Down = 4,
    DownLeft = 5,
    Left = 6,
    UpLeft = 7,
    #[default]
    None = 8,
}

impl DPadDirection {
    pub fn as_bitflag(&self) -> u8 {
        match *self {
            Self::Up => 1,                      // 00000001
            Self::UpRight => 1 | 1 << 1,        // 00000011
            Self::Right => 1 << 1,              // 00000010
            Self::DownRight => 1 << 2 | 1 << 1, // 00000110
            Self::Down => 1 << 2,               // 00000100
            Self::DownLeft => 1 << 2 | 1 << 3,  // 00001100
            Self::Left => 1 << 3,               // 00001000
            Self::UpLeft => 1 | 1 << 3,         // 00001001
            Self::None => 0,                    // 00000000
        }
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum GamepadMode {
    XInput = 0x00,
    DInput,
    Fps,
    #[default]
    Unknown,
}

impl From<u8> for GamepadMode {
    fn from(value: u8) -> Self {
        match value {
            0x00 => Self::XInput,
            0x01 => Self::DInput,
            0x02 => Self::Fps,
            _ => Self::Unknown,
        }
    }
}

impl Display for GamepadMode {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let str = match *self {
            GamepadMode::XInput => "xinput".to_string(),
            GamepadMode::DInput => "dinput".to_string(),
            GamepadMode::Fps => "fps".to_string(),
            GamepadMode::Unknown => "unknown".to_string(),
        };
        write!(f, "{}", str)
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum ConnectedState {
    #[default]
    Unknown,
    Connecting,
    Attached,
    Detached,
}

impl From<u8> for ConnectedState {
    fn from(value: u8) -> Self {
        match value {
            0x02 => Self::Attached,
            0x03 => Self::Detached,
            _ => Self::Connecting,
        }
    }
}

impl Display for ConnectedState {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let str = match *self {
            ConnectedState::Unknown => "unknown".to_string(),
            ConnectedState::Connecting => "connecting".to_string(),
            ConnectedState::Attached => "attached".to_string(),
            ConnectedState::Detached => "detached".to_string(),
        };
        write!(f, "{}", str)
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "60")]
pub struct XInputDataReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    #[packed_field(bytes = "1")]
    pub report_size: u8,
    #[packed_field(bytes = "2")]
    pub hid_cmd: u8,
    #[packed_field(bytes = "3")]
    pub unk_3: u8,
    #[packed_field(bytes = "4")]
    pub unk_4: u8,
    #[packed_field(bytes = "5")]
    pub l_con_battery: u8,

    // BYTE 6
    //#[packed_field(bytes = "6")]
    //pub l_con_state_alt: u8,

    // BYTE 7
    #[packed_field(bytes = "7")]
    pub r_con_battery: u8,

    // BYTE 8
    //#[packed_field(bytes = "8")]
    //pub r_con_state_alt: u8,

    // BYTE 9
    #[packed_field(bytes = "9", ty = "enum")]
    pub gamepad_mode: GamepadMode,

    // BYTE 10
    #[packed_field(bytes = "10")]
    pub unk_10: u8,

    // BYTE 11
    #[packed_field(bytes = "11")]
    pub unk_11: u8,

    // BYTE 12
    #[packed_field(bytes = "12", ty = "enum")] // 96 - 103
    pub l_con_state: ConnectedState,

    // BYTE 13
    #[packed_field(bytes = "13", ty = "enum")] // 104 - 11
    pub r_con_state: ConnectedState,

    // BYTE 14
    #[packed_field(byte = "14", endian = "lsb")]
    pub l_stick_x: u8,
    #[packed_field(byte = "15", endian = "lsb")]
    pub l_stick_y: u8,
    #[packed_field(bytes = "16", endian = "lsb")]
    pub r_stick_x: u8,
    #[packed_field(bytes = "17", endian = "lsb")]
    pub r_stick_y: u8,

    // BYTE 18
    #[packed_field(bits = "144")]
    pub legion: bool,
    #[packed_field(bits = "145")]
    pub quick_access: bool,
    #[packed_field(bits = "146")]
    pub thumb_l: bool,
    #[packed_field(bits = "147")]
    pub thumb_r: bool,
    #[packed_field(bits = "148")]
    pub up: bool,
    #[packed_field(bits = "149")]
    pub down: bool,
    #[packed_field(bits = "150")]
    pub left: bool,
    #[packed_field(bits = "151")]
    pub right: bool,

    // BYTE 19
    #[packed_field(bits = "152")]
    pub a: bool,
    #[packed_field(bits = "153")]
    pub b: bool,
    #[packed_field(bits = "154")]
    pub x: bool,
    #[packed_field(bits = "155")]
    pub y: bool,
    #[packed_field(bits = "156")]
    pub lb: bool,
    #[packed_field(bits = "157")]
    pub d_trigger_l: bool,
    #[packed_field(bits = "158")]
    pub rb: bool,
    #[packed_field(bits = "159")]
    pub d_trigger_r: bool,

    // BYTE 20
    #[packed_field(bits = "160")]
    pub y1: bool,
    #[packed_field(bits = "161")]
    pub y2: bool,
    #[packed_field(bits = "162")]
    pub y3: bool,
    #[packed_field(bits = "163")]
    pub m1: bool,
    #[packed_field(bits = "164")]
    pub m2: bool,
    #[packed_field(bits = "165")]
    pub m3: bool,
    #[packed_field(bits = "166")]
    pub view: bool,
    #[packed_field(bits = "167")]
    pub menu: bool,

    // BYTE 21
    #[packed_field(bits = "168")]
    pub mouse_click: bool,
    #[packed_field(bits = "169")]
    pub show_desktop: bool,
    #[packed_field(bits = "170")]
    pub alt_tab: bool,
    #[packed_field(bits = "171")]
    pub unk_21_3: bool,
    #[packed_field(bits = "172")]
    pub unk_21_4: bool,
    #[packed_field(bits = "173")]
    pub unk_21_5: bool,
    #[packed_field(bits = "174")]
    pub unk_21_6: bool,
    #[packed_field(bits = "175")]
    pub unk_21_7: bool,
    #[packed_field(bytes = "22")]
    pub a_trigger_l: u8,
    #[packed_field(bytes = "23")]
    pub a_trigger_r: u8,

    #[packed_field(bytes = "24")]
    pub unk_23: u8,

    #[packed_field(bytes = "25", endian = "msb")]
    pub mouse_z: u8,

    #[packed_field(bytes = "26..=27", endian = "msb")]
    pub touch_x: u16,
    #[packed_field(bytes = "28..=29", endian = "msb")]
    pub touch_y: u16,

    #[packed_field(bytes = "30")]
    pub left_gyro_lq_x: u8,
    #[packed_field(bytes = "31")]
    pub left_gyro_lq_y: u8,
    #[packed_field(bytes = "32")]
    pub right_gyro_lq_x: u8,
    #[packed_field(bytes = "33")]
    pub right_gyro_lq_y: u8,
    #[packed_field(bytes = "34")]
    pub left_imu_timestamp: u8,
    #[packed_field(bytes = "35..=36", endian = "msb")]
    pub left_accel_x: i16,
    #[packed_field(bytes = "37..=38", endian = "msb")]
    pub left_accel_y: i16,
    #[packed_field(bytes = "39..=40", endian = "msb")]
    pub left_accel_z: i16,
    #[packed_field(bytes = "41..=42", endian = "msb")]
    pub left_gyro_x: i16,
    #[packed_field(bytes = "43..=44", endian = "msb")]
    pub left_gyro_y: i16,
    #[packed_field(bytes = "45..=46", endian = "msb")]
    pub left_gyro_z: i16,
    #[packed_field(bytes = "47")]
    pub right_imu_timestamp: u8,
    #[packed_field(bytes = "50..=51", endian = "msb")]
    pub right_accel_x: i16,
    #[packed_field(bytes = "48..=49", endian = "msb")]
    pub right_accel_y: i16,
    #[packed_field(bytes = "52..=53", endian = "msb")]
    pub right_accel_z: i16,
    #[packed_field(bytes = "56..=57", endian = "msb")]
    pub right_gyro_x: i16,
    #[packed_field(bytes = "54..=55", endian = "msb")]
    pub right_gyro_y: i16,
    #[packed_field(bytes = "58..=59", endian = "msb")]
    pub right_gyro_z: i16,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "20")]
pub struct TouchpadDataReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,

    // BYTE 1
    #[packed_field(bits = "13")]
    pub contact_id_0: bool,
    #[packed_field(bits = "14")]
    pub tip_switch_0: bool,
    #[packed_field(bits = "15")]
    pub confidence_0: bool,

    #[packed_field(bytes = "2..=3", endian = "lsb")]
    pub touch_x_0: u16,
    #[packed_field(bytes = "4..=5", endian = "lsb")]
    pub touch_y_0: u16,

    // BYTE 6
    #[packed_field(bits = "53")]
    pub contact_id_1: bool,
    #[packed_field(bits = "54")]
    pub tip_switch_1: bool,
    #[packed_field(bits = "55")]
    pub confidence_1: bool,

    // BYTES 7-8
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub touch_x_1: u16,

    // BYTES 9-10
    #[packed_field(bytes = "9..=10", endian = "lsb")]
    pub touch_y_1: u16,

    // BYTE 11
    #[packed_field(bits = "93")]
    pub contact_id_2: bool,
    #[packed_field(bits = "94")]
    pub tip_switch_2: bool,
    #[packed_field(bits = "95")]
    pub confidence_2: bool,

    // BYTES 12-13
    #[packed_field(bytes = "12..=13", endian = "lsb")]
    pub touch_x_2: u16,

    // BYTES 14-15
    #[packed_field(bytes = "14..=15", endian = "lsb")]
    pub touch_y_2: u16,

    // BYTES 16-17
    #[packed_field(bytes = "16..=17", endian = "lsb")]
    pub scan_time: u16,

    // BYTE 18
    #[packed_field(bytes = "18")]
    pub contact_count: u8,
}
