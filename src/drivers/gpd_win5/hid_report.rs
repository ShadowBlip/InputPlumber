use packed_struct::prelude::*;

// GPD Win 5 Vendor HID Report (Usage Page 0xFF00, VID 0x2f24, PID 0x0137)
//
// Idle report:  01 a5 00 5a ff 00 01 09 00 00 00 00
// Button press values:
//   BUF[8]  = 0x68 mode switch, 0x00 released
//   BUF[9]  = 0x69 left back,   0x00 released
//   BUF[10] = 0x6a right back,  0x00 released
//
/// HID report size (device sends 12 bytes per report)
pub const PACKET_SIZE: usize = 12;

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "12")]
pub struct GpdWin5ButtonReport {
    // BUF[0-7]: Header / padding (not used by this driver)
    #[packed_field(bytes = "0..=7")]
    pub header: [u8; 8],

    // BUF[8]: Mode switch / QuickAccess button (0x68 = pressed)
    #[packed_field(bytes = "8")]
    pub mode_switch: u8,

    // BUF[9]: Left back button (0x69 = pressed)
    #[packed_field(bytes = "9")]
    pub left_back: u8,

    // BUF[10]: Right back button (0x6a = pressed)
    #[packed_field(bytes = "10")]
    pub right_back: u8,

    // BUF[11]: Padding
    #[packed_field(bytes = "11")]
    pub _pad: u8,
}
