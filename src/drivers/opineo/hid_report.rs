//! Reference: https://gitlab.com/open-sd/opensd/-/blob/main/src/opensdd/drivers/gamepad/hid_reports.hpp
#![allow(warnings)]
use packed_struct::prelude::*;

/// Different report types
pub enum ReportType {
    TouchpadData = 0x04,
}

impl ReportType {
    pub fn to_u8(&self) -> u8 {
        match self {
            ReportType::TouchpadData => ReportType::TouchpadData as u8,
        }
    }
}

// TouchpadData
//
// # ReportID: 4 / Confidence: 1 | Tip Switch: 0 | Contact Id:  0 | # | X:      0 | Y:      0 | Scan Time:  40742 | Contact Count:    1 | Button: 0  0  0 | #
// E: 000000.000000 10 04 01 00 00 00 00 26 9f 01 00
//
// X Axis
// # ReportID: 4 / Confidence: 1 | Tip Switch: 0 | Contact Id:  0 | # | X:    512 | Y:      0 | Scan Time:  49251 | Contact Count:    1 | Button: 0  0  0 | #
// E: 000022.851877 10 04 01 00 02 00 00 63 c0 01 00
// # ReportID: 4 / Confidence: 1 | Tip Switch: 0 | Contact Id:  0 | # | X:    467 | Y:      0 | Scan Time:   2804 | Contact Count:    1 | Button: 0  0  0 | #
// E: 000455.355131 10 04 01 d3 01 00 00 f4 0a 01 00
//
// Y Axis
// # ReportID: 4 / Confidence: 1 | Tip Switch: 0 | Contact Id:  0 | # | X:      0 | Y:    512 | Scan Time:  11269 | Contact Count:    1 | Button: 0  0  0 | #
//E: 000054.609703 10 04 01 00 00 00 02 05 2c 01 00
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "10")]
pub struct TouchpadDataReport {
    // BYTE 0
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    // BYTE 1
    #[packed_field(bytes = "1")]
    pub confidence: u8,
    // BYTE 2-3
    #[packed_field(bytes = "2..=3", endian = "lsb")]
    pub touch_x: Integer<u16, packed_bits::Bits<16>>,
    // BYTE 4
    #[packed_field(bytes = "4")]
    pub unk_4: u8,
    // BYTE 5-6
    #[packed_field(bytes = "5..=6", endian = "lsb")]
    pub touch_y: Integer<u16, packed_bits::Bits<16>>,
    // BYTE 7-8
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub scan_time: Integer<u16, packed_bits::Bits<16>>,
    // BYTE 9
    #[packed_field(bytes = "9")]
    pub unk_9: u8,
}

impl Default for TouchpadDataReport {
    fn default() -> Self {
        Self {
            report_id: Default::default(),
            confidence: Default::default(),
            touch_x: Default::default(),
            unk_4: Default::default(),
            touch_y: Default::default(),
            scan_time: Default::default(),
            unk_9: Default::default(),
        }
    }
}
