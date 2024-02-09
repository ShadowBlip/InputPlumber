//! Reference: https://gitlab.com/open-sd/opensd/-/blob/main/src/opensdd/drivers/gamepad/hid_reports.hpp
use packed_struct::prelude::*;

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0")]
pub struct PackedInputDataReport {
    // byte 0-3
    #[packed_field(bytes = "0")]
    pub major_ver: u8, // Major version? Always 0x01
    #[packed_field(bytes = "1")]
    pub minor_ver: u8, // Minor version? Always 0x00
    #[packed_field(bytes = "2")]
    pub report_type: u8, // Report type? Always 0x09
    #[packed_field(bytes = "3")]
    pub report_size: u8, // Actual data length of report in bytes.  Always 64 for input reports.
    // byte 4-7
    #[packed_field(bytes = "4..=7", endian = "lsb")]
    pub frame: Integer<u32, packed_bits::Bits<32>>, // Input frame counter?
    // byte 8
    #[packed_field(bits = "64")]
    pub r2: bool, // Binary sensor for analog triggers
    #[packed_field(bits = "65")]
    pub l2: bool,
    #[packed_field(bits = "66")]
    pub r1: bool, // Shoulder buttons
    #[packed_field(bits = "67")]
    pub l1: bool,
    #[packed_field(bits = "68")]
    pub y: bool, // Button cluster
    #[packed_field(bits = "69")]
    pub b: bool,
    #[packed_field(bits = "70")]
    pub x: bool,
    #[packed_field(bits = "71")]
    pub a: bool,
    // byte 9
    #[packed_field(bits = "72")]
    pub up: bool, // Directional Pad buttons
    #[packed_field(bits = "73")]
    pub right: bool,
    #[packed_field(bits = "74")]
    pub left: bool,
    #[packed_field(bits = "75")]
    pub down: bool,
    #[packed_field(bits = "76")]
    pub options: bool, // Overlapping square ⧉  button located above left stick
    #[packed_field(bits = "77")]
    pub steam: bool, // STEAM button below left trackpad
    #[packed_field(bits = "78")]
    pub menu: bool, // Hamburger (☰) button located above right stick
    #[packed_field(bits = "79")]
    pub l5: bool, // L5 & R5 on the back of the deck
    // byte 10
    #[packed_field(bits = "80")]
    pub r5: bool,
    #[packed_field(bits = "81")]
    pub l_pad_press: bool, // Binary "press" sensor for trackpads
    #[packed_field(bits = "82")]
    pub r_pad_press: bool,
    #[packed_field(bits = "83")]
    pub l_pad_touch: bool, // Binary "touch" sensor for trackpads
    #[packed_field(bits = "84")]
    pub r_pad_touch: bool,
    #[packed_field(bits = "85")]
    pub _unk3: bool,
    #[packed_field(bits = "86")]
    pub l3: bool, // Z-axis button on the left stick
    #[packed_field(bits = "87")]
    pub _unk4: bool,
    // byte 11
    #[packed_field(bits = "88")]
    pub _unk5: bool,
    #[packed_field(bits = "89")]
    pub _unk6: bool,
    #[packed_field(bits = "90")]
    pub r3: bool, // Z-axis button on the right stick
    #[packed_field(bits = "91")]
    pub _unk7: bool,
    #[packed_field(bits = "92")]
    pub _unk8: bool,
    #[packed_field(bits = "93")]
    pub _unk9: bool,
    #[packed_field(bits = "94")]
    pub _unk10: bool,
    #[packed_field(bits = "95")]
    pub _unk11: bool,
    // byte 12
    #[packed_field(bits = "96")]
    pub _unk12: bool,
    #[packed_field(bits = "97")]
    pub _unk13: bool,
    #[packed_field(bits = "98")]
    pub _unk14: bool,
    #[packed_field(bits = "99")]
    pub _unk15: bool,
    #[packed_field(bits = "100")]
    pub _unk16: bool,
    #[packed_field(bits = "101")]
    pub _unk17: bool,
    #[packed_field(bits = "102")]
    pub _unk18: bool,
    #[packed_field(bits = "103")]
    pub _unk19: bool,
    // byte 13
    #[packed_field(bits = "104")]
    pub _unk20: bool,
    #[packed_field(bits = "105")]
    pub l4: bool, // L4 & R4 on the back of the deck
    #[packed_field(bits = "106")]
    pub r4: bool,
    #[packed_field(bits = "107")]
    pub _unk21: bool,
    #[packed_field(bits = "108")]
    pub _unk22: bool,
    #[packed_field(bits = "109")]
    pub _unk23: bool,
    #[packed_field(bits = "110")]
    pub l_stick_touch: bool, // Binary touch sensors on the stick controls
    #[packed_field(bits = "111")]
    pub r_stick_touch: bool,
    // byte 14
    #[packed_field(bits = "112")]
    pub _unk24: bool,
    #[packed_field(bits = "113")]
    pub _unk25: bool,
    #[packed_field(bits = "114")]
    pub quick_access: bool, // Quick Access (...) button below right trackpad
    #[packed_field(bits = "115")]
    pub _unk26: bool,
    #[packed_field(bits = "116")]
    pub _unk27: bool,
    #[packed_field(bits = "117")]
    pub _unk28: bool,
    #[packed_field(bits = "118")]
    pub _unk29: bool,
    #[packed_field(bits = "119")]
    pub _unk30: bool,
    // byte 15
    #[packed_field(bytes = "15")]
    pub _unk31: u8, // Not sure, maybe padding or just unused
    // byte 16-23
    #[packed_field(bytes = "16..=17", endian = "lsb")]
    pub l_pad_x: Integer<i16, packed_bits::Bits<16>>, // Trackpad touch coordinates
    #[packed_field(bytes = "18..=19", endian = "lsb")]
    pub l_pad_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "20..=21", endian = "lsb")]
    pub r_pad_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "22..=23", endian = "lsb")]
    pub r_pad_y: Integer<i16, packed_bits::Bits<16>>,
    // byte 24-29
    #[packed_field(bytes = "24..=25", endian = "lsb")]
    pub accel_x: Integer<i16, packed_bits::Bits<16>>, // Accelerometers I think.  Needs more testing.
    #[packed_field(bytes = "26..=27", endian = "lsb")]
    pub accel_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "28..=29", endian = "lsb")]
    pub accel_z: Integer<i16, packed_bits::Bits<16>>,
    // byte 30-35
    #[packed_field(bytes = "30..=31", endian = "lsb")]
    pub pitch: Integer<i16, packed_bits::Bits<16>>, // Attitude (?)  Needs more testing
    #[packed_field(bytes = "32..=33", endian = "lsb")]
    pub yaw: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "34..=35", endian = "lsb")]
    pub roll: Integer<i16, packed_bits::Bits<16>>,
    // byte 36-43
    #[packed_field(bytes = "36..=37", endian = "lsb")]
    pub _gyro0: Integer<i16, packed_bits::Bits<16>>, // Not sure what these are...
    #[packed_field(bytes = "38..=39", endian = "lsb")]
    pub _gyro1: Integer<i16, packed_bits::Bits<16>>, // Seems like they might be additional gyros for extra precision (?)
    #[packed_field(bytes = "40..=41", endian = "lsb")]
    pub _gyro2: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "42..=43", endian = "lsb")]
    pub _gyro3: Integer<i16, packed_bits::Bits<16>>,
    // byte 44-47
    #[packed_field(bytes = "44..=45", endian = "lsb")]
    pub l_trigg: Integer<u16, packed_bits::Bits<16>>, // Pressure sensors for L2 & R2 triggers
    #[packed_field(bytes = "46..=47", endian = "lsb")]
    pub r_trigg: Integer<u16, packed_bits::Bits<16>>,
    // byte 48-55
    #[packed_field(bytes = "48..=49", endian = "lsb")]
    pub l_stick_x: Integer<i16, packed_bits::Bits<16>>, // Analogue thumbsticks
    #[packed_field(bytes = "50..=51", endian = "lsb")]
    pub l_stick_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "52..=53", endian = "lsb")]
    pub r_stick_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "54..=55", endian = "lsb")]
    pub r_stick_y: Integer<i16, packed_bits::Bits<16>>,
    // byte 56-59
    #[packed_field(bytes = "56..=57", endian = "lsb")]
    pub l_pad_force: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "58..=59", endian = "lsb")]
    pub r_pad_force: Integer<u16, packed_bits::Bits<16>>,
    // byte 60-63
    #[packed_field(bytes = "60..=61", endian = "lsb")]
    pub l_stick_force: Integer<u16, packed_bits::Bits<16>>, // Thumbstick capacitive sensors
    #[packed_field(bytes = "62..=63", endian = "lsb")]
    pub r_stick_force: Integer<u16, packed_bits::Bits<16>>,
    // 64 Bytes total
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0")]
pub struct PackedFeedbackReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    #[packed_field(bytes = "1")]
    pub report_size: u8,
    #[packed_field(bytes = "2")]
    pub side: u8,
    #[packed_field(bytes = "3..=4", endian = "lsb")]
    pub amplitude: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "5..=6", endian = "lsb")]
    pub period: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub count: Integer<u16, packed_bits::Bits<16>>,
}
