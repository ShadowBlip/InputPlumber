use packed_struct::prelude::*;

/// Button IDs emitted by the OXP vendor HID report mode.
/// Covers all IDs that appear in B4 mapping commands.
#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum ButtonId {
    #[default]
    None = 0x00,
    A = 0x01,
    B = 0x02,
    X = 0x03,
    Y = 0x04,
    LB = 0x05,
    RB = 0x06,
    LT = 0x07,
    RT = 0x08,
    Start = 0x09,
    Back = 0x0a,
    L3 = 0x0b,
    R3 = 0x0c,
    DUp = 0x0d,
    DDown = 0x0e,
    DLeft = 0x0f,
    DRight = 0x10,
    Guide = 0x21,
    M1 = 0x22,
    M2 = 0x23,
    Keyboard = 0x24,
}

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
    pub pressed: bool,
    #[packed_field(bytes = "13..=61")]
    pub _reserved_13_61: [u8; 49],
    #[packed_field(bytes = "62")]
    pub frame_foot: u8,
    #[packed_field(bytes = "63")]
    pub cid_foot: u8,
}
