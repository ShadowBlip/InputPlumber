#![allow(warnings)]
use packed_struct::prelude::*;

/// Different report types
pub enum ReportType {
    TouchpadData = 0x01,
    MacroKeyboardData,
}

impl ReportType {
    pub const fn to_u8(&self) -> u8 {
        match self {
            ReportType::TouchpadData => ReportType::TouchpadData as u8,
            ReportType::MacroKeyboardData => ReportType::MacroKeyboardData as u8,
        }
    }
}

// TouchpadData
//
// Top left
// # ReportID: 1 / Confidence: 1 | Tip Switch: 1 | # | Contact Id:   0 | X:      0 | Y:      7
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0 | Contact Count:    1 | Button: 0  0  0 | # | Scan Time:  15975
// E: 000138.861602 30 01 03 00 00 07 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 67 3e
// Top right
// # ReportID: 1 / Confidence: 1 | Tip Switch: 0 | # | Contact Id:   0 | X:   2559 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0 | Contact Count:    1 | Button: 0  0  0 | # | Scan Time:   6725
// E: 000164.504191 30 01 01 ff 09 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 45 1a
// Bottom left
// # ReportID: 1 / Confidence: 1 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:   1535
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0 | Contact Count:    1 | Button: 0  0  0 | # | Scan Time:  57905
// E: 000189.649370 30 01 01 00 00 ff 05 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 31 e2
// Bottom right
// # ReportID: 1 / Confidence: 1 | Tip Switch: 0 | # | Contact Id:   0 | X:   2559 | Y:   1535
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0
// #             | Confidence: 0 | Tip Switch: 0 | # | Contact Id:   0 | X:      0 | Y:      0 | Contact Count:    1 | Button: 0  0  0 | # | Scan Time:  43411
// E: 000201.428741 30 01 01 ff 09 ff 05 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 93 a9
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "30")]
pub struct TouchpadDataReport {
    // BYTE 0
    #[packed_field(bytes = "0")]
    pub report_id: u8,

    // BYTE 1-5: Finger 0
    #[packed_field(bits = "8..=11")]
    pub contact_id0: u8,
    // 12 and 13 are padding
    #[packed_field(bits = "14")]
    pub tip_switch0: bool,
    #[packed_field(bits = "15")]
    pub confidence0: bool,
    #[packed_field(bytes = "2..=3", endian = "lsb")]
    pub touch_x0: u16,
    #[packed_field(bytes = "4..=5", endian = "lsb")]
    pub touch_y0: u16,

    // BYTE 6-10: Finger 1
    #[packed_field(bits = "48..=51")]
    pub contact_id1: u8,
    // 52 and 53 are padding
    #[packed_field(bits = "54")]
    pub tip_switch1: bool,
    #[packed_field(bits = "55")]
    pub confidence1: bool,
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub touch_x1: u16,
    #[packed_field(bytes = "9..=10", endian = "lsb")]
    pub touch_y1: u16,

    // BYTE 11-15: Finger 2
    #[packed_field(bits = "88..=91")]
    pub contact_id2: u8,
    // 92 and 93 are padding
    #[packed_field(bits = "94")]
    pub tip_switch2: bool,
    #[packed_field(bits = "95")]
    pub confidence2: bool,
    #[packed_field(bytes = "12..=13", endian = "lsb")]
    pub touch_x2: u16,
    #[packed_field(bytes = "14..=15", endian = "lsb")]
    pub touch_y2: u16,

    // BYTE 16-20: Finger 3
    #[packed_field(bits = "128..=131")]
    pub contact_id3: u8,
    // 132 and 133 are padding
    #[packed_field(bits = "134")]
    pub tip_switch3: bool,
    #[packed_field(bits = "135")]
    pub confidence3: bool,
    #[packed_field(bytes = "17..=18", endian = "lsb")]
    pub touch_x3: u16,
    #[packed_field(bytes = "19..=20", endian = "lsb")]
    pub touch_y3: u16,

    // BYTE 21-25: Finger 4
    #[packed_field(bits = "168..=171")]
    pub contact_id4: u8,
    // 172 and 173 are padding
    #[packed_field(bits = "174")]
    pub tip_switch4: bool,
    #[packed_field(bits = "175")]
    pub confidence4: bool,
    #[packed_field(bytes = "22..=23", endian = "lsb")]
    pub touch_x4: u16,
    #[packed_field(bytes = "24..=25", endian = "lsb")]
    pub touch_y4: u16,

    // BYTE 26: Contact count
    #[packed_field(bytes = "26")]
    pub contact_count: u8,

    // BYTE 27: Buttons (unused)
    #[packed_field(bytes = "27")]
    pub _buttons: u8,

    // BYTE 28-29: Scan time
    #[packed_field(bytes = "28..=29", endian = "lsb")]
    pub scan_time: u16,
}

// MacroKeyboardData
//
// L4 only
//
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000000.000000 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['0x70000', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000000.295742 8 00 00 00 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000000.315681 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['0x70000', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000000.611655 8 00 00 00 00 00 00 00 00
//
// R4 only
//
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000009.024553 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['0x70000', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000009.319799 8 00 00 00 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000009.339826 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['0x70000', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000009.635850 8 00 00 00 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000009.656072 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['0x70000', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000009.952133 8 00 00 00 00 00 00 00 00

// L4, L4+R4, release

// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000032.764316 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['0x70000', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.061207 8 00 00 00 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.080626 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', 'Pause', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.292776 8 00 00 46 48 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.376570 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', 'PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.396706 8 00 00 48 46 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.588814 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', 'Pause', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.608325 8 00 00 46 48 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.692449 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', 'PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.713268 8 00 00 48 46 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.904337 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', 'Pause', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000033.924432 8 00 00 46 48 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000034.008374 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', 'PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000034.028301 8 00 00 48 46 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000034.220623 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', 'Pause', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000034.240262 8 00 00 46 48 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000034.324254 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', 'PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000034.344298 8 00 00 48 46 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000034.536343 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['0x70000', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000034.640707 8 00 00 00 00 00 00 00 00

// R4, L4+R4, release

// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000042.388506 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['0x70000', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000042.685124 8 00 00 00 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000042.704761 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', 'PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000042.992783 8 00 00 48 46 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.000912 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', 'Pause', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.020553 8 00 00 46 48 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.288683 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', 'PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.308784 8 00 00 48 46 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.316482 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', 'Pause', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.336461 8 00 00 46 48 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.604794 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', 'PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.624486 8 00 00 48 46 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.632943 8 00 00 46 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['PrintScreen', 'Pause', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.652791 8 00 00 46 48 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.920985 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['0x70000', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.948797 8 00 00 00 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['Pause', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000043.968795 8 00 00 48 00 00 00 00 00
// #  LeftControl: 0 | LeftShift: 0 | LeftAlt: 0 | Left GUI: 0 | RightControl: 0 | RightShift: 0 | RightAlt: 0 | Right GUI: 0 | # |Keyboard ['0x70000', '0x70000', '0x70000', '0x70000', '0x70000', '0x70000']
// E: 000044.264845 8 00 00 00 00 00 00 00 00
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "8")]
pub struct MacroKeyboardDataReport {
    #[packed_field(bytes = "0")]
    pub modifiers: u8,
    #[packed_field(bytes = "1")]
    pub reserved: u8,
    #[packed_field(bytes = "2..=7")]
    pub keys: [u8; 6],
}

impl MacroKeyboardDataReport {
    pub fn has_key(&self, code: u8) -> bool {
        self.keys.contains(&code)
    }
}
