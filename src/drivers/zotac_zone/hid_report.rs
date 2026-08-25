//! Vendor configuration protocol spoken on the Zotac Zone's command interface.
//!
//! The frame layout, CRC and command codes were derived from the vendor kernel
//! driver at <https://github.com/OpenZotacZone/ZotacZone-Drivers>
//! (`driver/hid/zotac-zone-hid-config.c`, GPL-2.0-or-later, Copyright (c) 2025
//! Luke D. Jones), which InputPlumber may use under that license's "or later"
//! clause. Values were verified against live captures from a Zotac Zone
//! running firmware 1.3.9.

/// Size of a command frame, excluding the leading hidraw report number.
pub const REPORT_SIZE: usize = 64;

const HEADER_TAG: u8 = 0xE1;
const PAYLOAD_SIZE: u8 = 0x3C;

// Byte offsets within a command frame.
const HEADER_TAG_POS: usize = 0x00;
const RESERVED_POS: usize = 0x01;
const SEQUENCE_POS: usize = 0x02;
const PAYLOADSIZE_POS: usize = 0x03;
const COMMAND_POS: usize = 0x04;
const CRC_H_POS: usize = 0x3E;
const CRC_L_POS: usize = 0x3F;

// The checksum covers the command and its payload, but not the framing bytes
// that precede them - notably not the sequence number.
const CRC_START: usize = 0x04;
const CRC_END: usize = 0x3E;

const CMD_SET_BUTTON_MAPPING: u8 = 0xA1;

// Offsets of a button mapping payload, relative to the start of the frame. The
// bytes in between hold gamepad button, modifier key and mouse button
// bindings, all of which are left cleared here.
const MAP_SOURCE_POS: usize = 0x05;
const MAP_KEYBOARD_POS: usize = 0x0C;

/// Status byte of a `CMD_SET_BUTTON_MAPPING` response. Commands that carry a
/// setting byte report their status one byte later than the rest do.
const MAP_STATUS_POS: usize = 0x06;

/// Button ids used by the mapping commands.
pub const BUTTON_M1: u8 = 0x01;
pub const BUTTON_M2: u8 = 0x02;

/// HID keyboard usage codes.
pub const HID_KEY_HOME: u8 = 0x4A;
pub const HID_KEY_END: u8 = 0x4D;

/// Checksum over the command and payload bytes of a frame.
fn calc_crc(frame: &[u8; REPORT_SIZE]) -> u16 {
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

/// Build a command frame with the framing bytes filled in.
fn build_frame(sequence: u8, command: u8) -> [u8; REPORT_SIZE] {
    let mut frame = [0; REPORT_SIZE];
    frame[HEADER_TAG_POS] = HEADER_TAG;
    frame[RESERVED_POS] = 0x00;
    frame[SEQUENCE_POS] = sequence;
    frame[PAYLOADSIZE_POS] = PAYLOAD_SIZE;
    frame[COMMAND_POS] = command;
    frame
}

/// Append the checksum to a frame and prefix the hidraw report number. The
/// command interface uses unnumbered reports, so that number is always zero.
fn finish_frame(mut frame: [u8; REPORT_SIZE]) -> [u8; REPORT_SIZE + 1] {
    let crc = calc_crc(&frame);
    frame[CRC_H_POS] = (crc >> 8) as u8;
    frame[CRC_L_POS] = crc as u8;

    let mut report = [0; REPORT_SIZE + 1];
    report[1..].copy_from_slice(&frame);
    report
}

/// Build a command binding `button_id` to a single keyboard `key`, clearing any
/// gamepad, modifier and mouse bindings it previously had.
pub fn set_keyboard_mapping(sequence: u8, button_id: u8, key: u8) -> [u8; REPORT_SIZE + 1] {
    let mut frame = build_frame(sequence, CMD_SET_BUTTON_MAPPING);
    frame[MAP_SOURCE_POS] = button_id;
    frame[MAP_KEYBOARD_POS] = key;
    finish_frame(frame)
}

/// The status a button mapping response reports, or `None` if `response` isn't
/// an answer to a button mapping command. A status of zero means the device
/// accepted it.
pub fn mapping_status(response: &[u8]) -> Option<u8> {
    if response.get(COMMAND_POS) != Some(&CMD_SET_BUTTON_MAPPING) {
        return None;
    }
    response.get(MAP_STATUS_POS).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checksum of a frame captured from a Zotac Zone's command interface:
    /// `e1 00 c5 3c b2 00 .. 00 fc 8d`. The sequence byte varied between
    /// captures while the checksum did not, which is what places the sequence
    /// number outside the checksummed range.
    #[test]
    fn crc_matches_device_frame() {
        let mut frame = [0; REPORT_SIZE];
        frame[HEADER_TAG_POS] = HEADER_TAG;
        frame[SEQUENCE_POS] = 0xC5;
        frame[PAYLOADSIZE_POS] = PAYLOAD_SIZE;
        frame[COMMAND_POS] = 0xB2;

        assert_eq!(calc_crc(&frame), 0xFC8D);
    }

    #[test]
    fn mapping_command_is_framed_correctly() {
        let report = set_keyboard_mapping(0, BUTTON_M2, HID_KEY_HOME);

        assert_eq!(report[0], 0x00, "hidraw report number");

        let frame = &report[1..];
        assert_eq!(frame[HEADER_TAG_POS], HEADER_TAG);
        assert_eq!(frame[PAYLOADSIZE_POS], PAYLOAD_SIZE);
        assert_eq!(frame[COMMAND_POS], CMD_SET_BUTTON_MAPPING);
        assert_eq!(frame[MAP_SOURCE_POS], BUTTON_M2);
        assert_eq!(frame[MAP_KEYBOARD_POS], HID_KEY_HOME);
    }

    #[test]
    fn mapping_status_ignores_other_commands() {
        let mut response = [0; REPORT_SIZE];
        response[COMMAND_POS] = 0xB2;
        assert_eq!(mapping_status(&response), None);

        response[COMMAND_POS] = CMD_SET_BUTTON_MAPPING;
        response[MAP_STATUS_POS] = 0x00;
        assert_eq!(mapping_status(&response), Some(0));
    }
}
