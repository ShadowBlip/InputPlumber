use packed_struct::prelude::*;

// GPD Win 5 Extended Keyboard HID Report
//
// The new firmware sends an extended HID report where the first 8 bytes
// are the standard HID keyboard report, and bytes 8-11 contain the
// hidden button states:
//
// BUF[0]   - Modifiers (standard keyboard)
// BUF[1]   - Reserved
// BUF[2-7] - Standard keyboard Keys[6]
// BUF[8]   - Mode switch button (short press) trigger value
// BUF[9]   - Left back button trigger value
// BUF[10]  - Right back button trigger value
// BUF[11]  - Reserved
//
// TODO: Verify with actual firmware data capture:
// - Exact trigger values (assumed non-zero = pressed)
// - Actual report size (may be larger than 12 bytes)

/// Report size for the extended keyboard data
pub const REPORT_SIZE: usize = 12;

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "12")]
pub struct GpdWin5ButtonReport {
    // BUF[0]: Standard keyboard modifiers
    #[packed_field(bytes = "0")]
    pub modifiers: u8,

    // BUF[1]: Reserved
    #[packed_field(bytes = "1")]
    pub reserved: u8,

    // BUF[2-7]: Standard keyboard keys (not used by this driver)
    #[packed_field(bytes = "2..=7")]
    pub keys: [u8; 6],

    // BUF[8]: Mode switch / QuickAccess button trigger value
    #[packed_field(bytes = "8")]
    pub mode_switch: u8,

    // BUF[9]: Left back button trigger value
    #[packed_field(bytes = "9")]
    pub left_back: u8,

    // BUF[10]: Right back button trigger value
    #[packed_field(bytes = "10")]
    pub right_back: u8,

    // BUF[11]: Reserved
    #[packed_field(bytes = "11")]
    pub reserved2: u8,
}
