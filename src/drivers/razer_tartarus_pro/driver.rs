use super::event::Event;
use crate::udev::device::UdevDevice;
use hidapi::HidDevice;
use std::{error::Error, ffi::CString};

pub const VID: u16 = 0x1532;
pub const PID: u16 = 0x0244;

const RAZER_HID_ARRAY_SIZE: usize = 91;
const MESSAGE_DATA_PAYLOAD: usize = 24;
const RAZER_FEATURE_ENDPOINT: i32 = 2;

pub struct Driver {
    device: HidDevice,
    razer_message_id: u8,
}

impl Driver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let hidrawpath = udevice.devnode();
        let cs_path = CString::new(hidrawpath.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;
        let info = device.get_device_info()?;
        let mut razer_message_id = 0 as u8;
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{hidrawpath}' is not a Razer Tartarus Pro").into());
        }
        if info.interface_number() == RAZER_FEATURE_ENDPOINT {
            // Check if we can get serial number
            let packet = Self::razer_report(0x0, 0x82, 0x16, &[0x0], &mut razer_message_id);
            let output = Self::transaction(packet, &device).unwrap();
            log::info!(
                "Tartarus Serial Number: {:?}",
                String::from_utf8(output).unwrap().trim_end_matches("\0")
            );
        }

        Ok(Self {
            device,
            razer_message_id,
        })
    }

    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let events = Vec::new();
        Ok(events)
    }

    // Rust's USBHID implementation enforces the use of Windows-ism of needing a dud zeroth byte
    // for unused report ID, so +1 the buffer size. 91 now for Razer, our window is [1..91].
    // Ignore [0]. The XOR is always the penultimate byte with the final byte being 0.
    // The first byte is a status byte which for PC->DEV is 0. Replies always start with 0x2.
    // The next 4 bytes are used in other Razer devices, but not the Tartarus so we leave them 0.

    fn razer_report(
        cmd_class: i32,
        cmd_id: i32,
        data_size: i32,
        payload: &[u8],
        id: &mut u8,
    ) -> [u8; RAZER_HID_ARRAY_SIZE] {
        let mut message: [u8; RAZER_HID_ARRAY_SIZE] = [0; RAZER_HID_ARRAY_SIZE];
        message[1] = 0; // Always 0 when sending to the device
        message[2] = *id as u8;
        message[6] = data_size as u8;
        message[7] = cmd_class as u8;
        message[8] = cmd_id as u8;
        message[9..(9 + payload.len())].copy_from_slice(&payload);

        // Generate checksum
        let mut xorval = 0;
        for refr in message[6..89].iter() {
            xorval ^= refr;
        }

        message[89] = xorval as u8;

        // Increment next transaction id
        *id = id.wrapping_add(1);
        return message;
    }

    // This validates any received reports against the feature request and provides the payload
    // as a vector for further processing.
    fn razer_decapsulate(
        request: &[u8],
        response: &[u8],
    ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let mut is_valid = true;
        is_valid = (response[1] == 2) && is_valid;
        is_valid = response[3..5].iter().all(|&b| b == 0) && is_valid;
        is_valid = (response[6] <= 0x50) && is_valid;
        is_valid = (response[7] == request[7]) && is_valid;
        is_valid = (response[8] == request[8]) && is_valid;

        // Validate the checksum
        let mut xorval = 0;
        for refr in response[6..89].iter() {
            xorval ^= refr;
        }
        is_valid = (response[89] == xorval) && is_valid;
        if !is_valid {
            log::error!("Invalid Tartarus transaction received");
            Ok(Vec::new())
        } else {
            Ok(response[9..(9 + response[6] as usize)].to_vec())
        }
    }

    // The Tartarus Pro utilises a write/read cycle so there are no discrete
    // read and write operations, just a transaction and a set of expectations.
    fn transaction(
        packet: [u8; RAZER_HID_ARRAY_SIZE],
        handle: &HidDevice,
    ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        if handle.get_device_info().unwrap().interface_number() == RAZER_FEATURE_ENDPOINT {
            let res = handle.send_feature_report(&packet);
            let mut feature_buffer: [u8; RAZER_HID_ARRAY_SIZE] = [0; RAZER_HID_ARRAY_SIZE];
            let _ = handle.get_feature_report(&mut feature_buffer);
            return Ok(Self::razer_decapsulate(&packet, &feature_buffer)?)
        }
        log::error!("Tartarus attempting to transact on incorrect endpoint");
        return Ok(Vec::new());
    }
}
