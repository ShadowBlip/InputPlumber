//! Reference: https://gitlab.com/open-sd/opensd/-/blob/main/src/opensdd/drivers/gamepad/hid_reports.hpp
#![allow(warnings)]
use futures::FutureExt;
use packed_struct::prelude::*;

use std::fmt::Display;
use std::fmt::Formatter;

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum ReportType {
    #[default]
    ButtonData = 0x1a,
    JoystickData = 0x1b,
    TakeoverAck = 0xef,
}

impl ReportType {
    pub fn to_u8(&self) -> u8 {
        match self {
            ReportType::JoystickData => ReportType::JoystickData as u8,
            ReportType::ButtonData => ReportType::ButtonData as u8,
            ReportType::TakeoverAck => ReportType::TakeoverAck as u8,
        }
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum ButtonId {
    #[default]
    None = 0x00,
    A = 0x01,
    B = 0x02,
    X = 0x03,
    Y = 0x04,
    LeftBumper = 0x05,
    RightBumper = 0x06,
    LeftTrigger = 0x07,
    RightTrigger = 0x08,
    Menu = 0x09,
    View = 0x0a,
    LeftStick = 0x0b,
    RightStick = 0x0c,
    DpadUp = 0x0d,
    DpadDown = 0x0e,
    DpadLeft = 0x0f,
    DpadRight = 0x10,
    Guide = 0x21,
    M1 = 0x22,
    M2 = 0x23,
    Keyboard = 0x24,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum ButtonStatus {
    #[default]
    Pressed = 0x01,
    Released = 0x02,
}

// Button Data
// [eb, 1a, 3f, 0f, 02, 0f, 00, 00, 00, 02, 00, 00, 00, 3f, 1a]
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "15")]
pub struct ButtonDataReport {
    #[packed_field(bytes = "0")]
    pub report_count: u8,
    #[packed_field(bytes = "1", ty = "enum")]
    pub report_type_head: ReportType, // always 1a
    #[packed_field(bytes = "2")]
    pub report_id_head: u8, // always 3f
    #[packed_field(bytes = "3", ty = "enum")]
    pub button_id: ButtonId,
    #[packed_field(bytes = "4")]
    pub mode: u8,
    #[packed_field(bytes = "5", ty = "enum")]
    pub value: ButtonId, // Remapped value
    #[packed_field(bytes = "6")]
    pub custom_1: u8,
    #[packed_field(bytes = "7")]
    pub custom_2: u8,
    #[packed_field(bytes = "8")]
    pub custom_3: u8,
    #[packed_field(bytes = "9", ty = "enum")]
    pub status: ButtonStatus,
    #[packed_field(bytes = "10")]
    pub reserved_10: u8,
    #[packed_field(bytes = "11")]
    pub reserved_11: u8,
    #[packed_field(bytes = "12")]
    pub reserved_12: u8,
    #[packed_field(bytes = "13")]
    pub report_type_footer: u8, // always 1a
    #[packed_field(bytes = "14")]
    pub report_id_footer: u8, // always 3f
}

// Joystick Data
// [01, 1b, 3f, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 3f, 1b]
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "15")]
pub struct JoystickDataReport {
    #[packed_field(bytes = "0")]
    pub report_count: u8,
    #[packed_field(bytes = "1", ty = "enum")]
    pub report_type_head: ReportType, // always 1b
    #[packed_field(bytes = "2")]
    pub report_id_head: u8, // always 3f
    #[packed_field(bytes = "3")]
    pub left_trigger: u8,
    #[packed_field(bytes = "4")]
    pub right_trigger: u8,
    #[packed_field(bytes = "5..=6", endian = "lsb")]
    pub left_stick_x: i16,
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub left_stick_y: i16,
    #[packed_field(bytes = "9..=10", endian = "lsb")]
    pub right_stick_x: i16,
    #[packed_field(bytes = "11..=12", endian = "lsb")]
    pub right_stick_y: i16,
    #[packed_field(bytes = "13")]
    pub report_type_footer: u8, // always 1b
    #[packed_field(bytes = "14")]
    pub report_id_footer: u8, // always 3f
}
