//! Vendor configuration protocol spoken on the Zotac Zone's command interface.
//!
//! The frame layout, CRC and command codes were derived from the vendor kernel
//! driver at <https://github.com/OpenZotacZone/ZotacZone-Drivers>
//! (`driver/hid/zotac-zone-hid-config.c`, GPL-2.0-or-later, Copyright (c) 2025
//! Luke D. Jones), which InputPlumber may use under that license's "or later"
//! clause. Values were verified against live captures from a Zotac Zone
//! running firmware 1.3.9.

use packed_struct::prelude::*;

/// Size of a command frame, excluding the leading hidraw report number.
pub const REPORT_SIZE: usize = 64;

const HEADER_TAG: u8 = 0xE1;
const PAYLOAD_SIZE: u8 = 0x3C;

// The checksum covers the command and its payload, but not the framing bytes
// that precede them - notably not the sequence number. Indices below are
// absolute, counting the leading hidraw report number (always zero) as byte 0.
const CRC_START: usize = 0x05;
const CRC_END: usize = 0x3F;
const CRC_H_POS: usize = 0x3F;
const CRC_L_POS: usize = 0x40;

/// HID keyboard usage codes.
pub const HID_KEY_HOME: u8 = 0x4A;
pub const HID_KEY_END: u8 = 0x4D;

/// Commands understood by the configuration interface.
#[derive(PrimitiveEnum_u8, Debug, Clone, Copy, PartialEq)]
pub enum Command {
    SetButtonMapping = 0xA1,
}

/// Rear macro buttons that can be remapped over this protocol.
#[derive(PrimitiveEnum_u8, Debug, Clone, Copy, PartialEq)]
pub enum ButtonId {
    M1 = 0x01,
    M2 = 0x02,
}

impl ButtonId {
    /// Sequence number to stamp on this button's mapping command. The two
    /// buttons are always mapped together at startup, so a fixed, distinct
    /// number per button is enough to match each response to its own request
    /// on the shared command interface.
    fn sequence(self) -> u8 {
        match self {
            ButtonId::M2 => 0,
            ButtonId::M1 => 1,
        }
    }
}

/// A `CMD_SET_BUTTON_MAPPING` request binding a button to a single keyboard
/// key, clearing any gamepad, modifier and mouse bindings it previously had.
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "65")]
pub struct ButtonMappingRequest {
    #[packed_field(bytes = "1")]
    header_tag: u8,
    #[packed_field(bytes = "3")]
    sequence: u8,
    #[packed_field(bytes = "4")]
    payload_size: u8,
    #[packed_field(bytes = "5", ty = "enum")]
    command: Command,
    #[packed_field(bytes = "6", ty = "enum")]
    button_id: ButtonId,
    #[packed_field(bytes = "13")]
    key: u8,
}

impl ButtonMappingRequest {
    pub fn new(button_id: ButtonId, key: u8) -> Self {
        Self {
            header_tag: HEADER_TAG,
            sequence: button_id.sequence(),
            payload_size: PAYLOAD_SIZE,
            command: Command::SetButtonMapping,
            button_id,
            key,
        }
    }

    /// Packs this request into a hidraw write buffer with its checksum filled
    /// in. The command interface uses unnumbered reports, so the leading
    /// report number byte is always zero.
    pub fn to_bytes(self) -> Result<[u8; REPORT_SIZE + 1], PackingError> {
        let mut report = self.pack()?;
        let crc = calc_crc(&report);
        report[CRC_H_POS] = (crc >> 8) as u8;
        report[CRC_L_POS] = crc as u8;
        Ok(report)
    }
}

/// A `CMD_SET_BUTTON_MAPPING` response. Commands that carry a setting byte
/// report their status one byte later than the rest do.
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct ButtonMappingResponse {
    #[packed_field(bytes = "4", ty = "enum")]
    command: Command,
    #[packed_field(bytes = "6")]
    status: u8,
}

impl ButtonMappingResponse {
    /// The status a response reports, or `None` if `bytes` isn't an answer to
    /// a button mapping command. A status of zero means the device accepted
    /// it.
    pub fn status(bytes: &[u8]) -> Option<u8> {
        let mut frame = [0; REPORT_SIZE];
        let len = bytes.len().min(REPORT_SIZE);
        frame[..len].copy_from_slice(&bytes[..len]);

        Self::unpack(&frame).ok().map(|response| response.status)
    }
}

/// Checksum over the command and payload bytes of a request frame. Exposed
/// crate-internally so the frame layout can be verified against a captured
/// device frame in `hid_report_test.rs`, which isn't otherwise reachable
/// through the type-safe [ButtonMappingRequest] API (a raw capture's command
/// byte is not necessarily a [Command] this module knows about).
pub(crate) fn calc_crc(frame: &[u8; REPORT_SIZE + 1]) -> u16 {
    let mut crc: u16 = 0;

    for byte in &frame[CRC_START..CRC_END] {
        let h1 = (crc as u32 ^ *byte as u32) & 0xFF;
        let h2 = h1 & 0x0F;
        let h3 = (h2 << 4) ^ h1;
        let h4 = h3 >> 4;

        crc = (((((h3 << 1) ^ h4) << 4) ^ h2) << 3) as u16 ^ h4 as u16 ^ (crc >> 8);
    }

    crc
}
