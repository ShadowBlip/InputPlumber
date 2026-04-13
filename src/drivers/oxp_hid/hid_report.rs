use packed_struct::prelude::*;

pub use crate::drivers::oxp_tty::ButtonId;

/// OXP HID 64-byte input report.
/// Frame: [cid, 0x3F, 0x01, ...payload..., 0x3F, cid]
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct InputDataReport {
    #[packed_field(bytes = "0")]
    pub cid: u8,
    #[packed_field(bytes = "1")]
    pub frame_head: u8,
    #[packed_field(bytes = "2")]
    pub _reserved_2: u8,
    #[packed_field(bytes = "3")]
    pub pkt_type: u8,
    #[packed_field(bytes = "4")]
    pub _reserved_4: u8,
    #[packed_field(bytes = "5")]
    pub flag: u8,
    #[packed_field(bytes = "6", ty = "enum")]
    pub btn: ButtonId,
    #[packed_field(bytes = "7")]
    pub func_code: u8,
    #[packed_field(bytes = "8..=11")]
    pub _reserved_8_11: [u8; 4],
    #[packed_field(bytes = "12")]
    pub press_state: u8,
    #[packed_field(bytes = "13..=61")]
    pub _reserved_13_61: [u8; 49],
    #[packed_field(bytes = "62")]
    pub frame_foot: u8,
    #[packed_field(bytes = "63")]
    pub cid_foot: u8,
}
