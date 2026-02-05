use super::event::{Event, KeyCodes};
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
    analog_state: [u8; MESSAGE_DATA_PAYLOAD],
    key_state: Vec<KeyCodes>,
}

/// This driver implementation is shared across all three HID handles on the
/// Tartarus Pro. Certain code-paths will only execute on certain handles.
/// Refer to handle_input_report()

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
            analog_state: [0; MESSAGE_DATA_PAYLOAD],
            key_state: Vec::new(),
        })
    }

    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0; MESSAGE_DATA_PAYLOAD];
        let bytes_read = self.device.read(&mut buf[..])?;
        let slice = &buf[..bytes_read];
        let events = self.handle_input_report(slice, bytes_read)?;
        Ok(events)
    }

    // The hidapi crate enforces the Windows-ism of needing a dud zeroth byte for sending feature
    // reports to devices with no report ID specified. For unused report ID This means our window
    // is [1..] and we ignore [0], though leave it 0. The XOR is always the penultimate byte with
    // the ultimate byte being 0. Byte [1] is a status byte which for PC->DEV is 0. Replies
    // always start with 0x2. Bytes [3..5] are used in other Razer devices but not the Tartarus
    // so we leave them 0.
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

    fn transaction(
        packet: [u8; RAZER_HID_ARRAY_SIZE],
        handle: &HidDevice,
    ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        if handle.get_device_info().unwrap().interface_number() == RAZER_FEATURE_ENDPOINT {
            let _ = handle.send_feature_report(&packet);
            let mut feature_buffer: [u8; RAZER_HID_ARRAY_SIZE] = [0; RAZER_HID_ARRAY_SIZE];
            let _ = handle.get_feature_report(&mut feature_buffer);
            return Ok(Self::razer_decapsulate(&packet, &feature_buffer)?);
        }
        log::error!("Tartarus attempting to transact on incorrect endpoint");
        return Ok(Vec::new());
    }

    fn handle_input_report(
        &mut self,
        buf: &[u8],
        bytes_read: usize,
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let info = self.device.get_device_info()?;
        let mut events = Vec::new();

        // Depending on what handle we actually are (this code runs on all 3)
        // we have different reports that we can expect and respond to in kind.

        // Endpoint 1.1 is always played as per its HID report descriptor but it is simple enough
        // to re-implement here. There is no report ID, it gets straight to business.
        // This interface captures the D-pad and the aux key.

        // Endpoint 1.2 has two personalities, if report ID 1 is seen then it is in default
        // keyboard mode and is played exactly as its descriptor says. If report ID 6 is seen
        // that is analog mode so we need to run the threshold checks.
        // This interface regardless of personality captures the 20 numbered keys.

        // Endpoint 1.3 is in the same basket as 1.1, different patterns to match though.
        // This interface captures the scroll wheel.

        match info.interface_number() {
            0 => self.handle_basic(buf, KeyCodes::Aux, false),
            1 => {
                match buf[0] {
                    0x1 => return self.handle_basic(buf, KeyCodes::Blank, true),
                    0x6 => {
                        // Perform thresholding
                        todo!()
                    }
                    _ => {
                        // Other report types exist but don't appear to be
                        // actually used.
                        Ok(events)
                    }
                }
            }
            2 => self.handle_basic(buf, KeyCodes::MClick, false),
            _ => Ok(events),
        }
    }

    // The basic case for all 3 endpoints is essentially identical. The first byte is unique for
    // each; For endpoints 1.1 and 1.3 this is where code 0x04 is interpreted and discarded for
    // endpoint 1.2 as it was a report ID which has served its purpose if we got this far.
    // Given the conversion to variant space it is open to misinterpretation if left there.
    fn handle_basic(
        &mut self,
        buf: &[u8],
        key_replace: KeyCodes,
        overwrite: bool,
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut events = Vec::new();
        let mut pad_state: Vec<KeyCodes> = buf.iter().map(|&s| KeyCodes::from(s)).collect();

        // Override Byte 0 as specified and remove any blanks
        if pad_state[0] == KeyCodes::KeyTwelve || overwrite {
            pad_state[0] = key_replace;
        }
        pad_state.retain(|x| *x != KeyCodes::Blank);

        // If a key is present in the report then it was pressed
        for i in &pad_state {
            events.push(Event {
                key: i.clone(),
                pressed: true,
            });
        }

        // If a key is missing compared to last time then indicate it is no longer pressed
        for i in &self.key_state {
            if !pad_state.contains(&i) {
                events.push(Event {
                    key: i.clone(),
                    pressed: false,
                });
            }
        }

        // Save state for next time
        self.key_state = pad_state;
        Ok(events)
    }
}
