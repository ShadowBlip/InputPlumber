use std::error::Error;

use super::hid_report::{
    calc_crc, ButtonId, ButtonMappingRequest, ButtonMappingResponse, HID_KEY_HOME, REPORT_SIZE,
};

/// Checksum of a frame captured from a Zotac Zone's command interface:
/// `e1 00 c5 3c b2 00 .. 00 fc 8d`. The sequence byte varied between captures
/// while the checksum did not, which is what places the sequence number
/// outside the checksummed range. The command byte (`0xB2`, an unsolicited
/// telemetry frame) isn't a [`Command`](super::hid_report::Command) this
/// module knows how to send, so this frame is built by hand rather than
/// through [`ButtonMappingRequest`].
#[tokio::test]
async fn crc_matches_device_frame() -> Result<(), Box<dyn Error>> {
    let mut frame = [0u8; REPORT_SIZE + 1];
    frame[1] = 0xE1; // header tag
    frame[3] = 0xC5; // sequence
    frame[4] = 0x3C; // payload size
    frame[5] = 0xB2; // command

    assert_eq!(calc_crc(&frame), 0xFC8D);

    Ok(())
}

#[tokio::test]
async fn mapping_command_is_framed_correctly() -> Result<(), Box<dyn Error>> {
    let report = ButtonMappingRequest::new(ButtonId::M2, HID_KEY_HOME).to_bytes()?;

    assert_eq!(report[0], 0x00, "hidraw report number");
    assert_eq!(report[1], 0xE1, "header tag");
    assert_eq!(report[4], 0x3C, "payload size");
    assert_eq!(report[5], 0xA1, "command");
    assert_eq!(report[6], ButtonId::M2 as u8, "button id");
    assert_eq!(report[13], HID_KEY_HOME, "key");

    Ok(())
}

#[tokio::test]
async fn mapping_status_ignores_other_commands() -> Result<(), Box<dyn Error>> {
    let mut response = [0u8; REPORT_SIZE];
    response[4] = 0xB2; // an unrelated command
    assert_eq!(ButtonMappingResponse::status(&response), None);

    response[4] = 0xA1; // CMD_SET_BUTTON_MAPPING
    response[6] = 0x00; // status: accepted
    assert_eq!(ButtonMappingResponse::status(&response), Some(0));

    Ok(())
}
