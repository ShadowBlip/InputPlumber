use super::event::{Event, KeyCodes};
use crate::udev::device::UdevDevice;
use hidapi::HidDevice;
use std::collections::VecDeque;
use std::{error::Error, ffi::CString};

pub const VID: u16 = 0x1532;
pub const PID: u16 = 0x0244;

const RAZER_HID_ARRAY_SIZE: usize = 91;
const MESSAGE_DATA_PAYLOAD: usize = 24;
const RAZER_FEATURE_ENDPOINT: i32 = 2;
const TREND_KEYDOWN: f64 = 1.0;
const TREND_KEYUP: f64 = -1.0;
const REGRESSION_WINDOW: usize = 5;
const KEY_COUNT: usize = 20;
const UNIT_TO_MM: f64 = 0.1 / 12.0;

pub struct Driver {
    device: HidDevice,
    razer_message_id: u8,
    hysteresis: VecDeque<[u8; KEY_COUNT]>,
    key_actions: [AnalogAction; KEY_COUNT],
    key_state: Vec<KeyCodes>,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct AnalogAction {
    actuation_point: (u8, u8),
    retrigger_window: (f64, f64), // as mm
    crt_en: bool,
    actuated: (bool, bool),
    retrigger_track: f64, // as mm
    retrigger_go: bool,
    keydown_up_n: bool, // Reflects the previous trend on a key
}

// This driver implementation is shared across all three HID handles on the
// Tartarus Pro. Certain code-paths will only execute on certain handles.
// Refer to handle_input_report()

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
            let mut packet = Self::razer_report(0x0, 0x82, 0x16, &[0x0], &mut razer_message_id);
            let output = Self::transaction(packet, &device).unwrap();
            log::info!(
                "Tartarus Serial Number: {:?}",
                String::from_utf8(output).unwrap().trim_end_matches("\0")
            );
            // Enable analog mode
            packet = Self::razer_report(0x0, 0x4, 0x2, &[0x3, 0x0], &mut razer_message_id);
            log::info!("Enabling Tartarus Pro analog mode");
            let _ = Self::transaction(packet, &device).unwrap();
        }

        let mut zeroes = VecDeque::with_capacity(REGRESSION_WINDOW);
        for _ in 0..zeroes.capacity() {
            zeroes.push_back([0; KEY_COUNT]);
        }
        let mut check = Self {
            device,
            razer_message_id,
            hysteresis: zeroes,
            key_actions: [AnalogAction::default(); KEY_COUNT],
            key_state: Vec::new(),
        };
        check.key_actions[0].actuation_point = (0x24, 0x78);
        check.key_actions[0].retrigger_window = (0.0, 0.0);
        check.key_actions[0].crt_en = false;

        Ok(check)
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
                        return self.handle_analog(&buf[1..21]);
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

    fn handle_analog(&mut self, keys: &[u8]) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let key_arr: &[u8; KEY_COUNT];
        if let Ok(value) = <&[u8] as TryInto<&[u8; KEY_COUNT]>>::try_into(keys) {
            key_arr = value;
        } else {
            log::error!("Incorrect size array passed to handle_analog");
            return Ok(Vec::new());
        }

        self.hysteresis.pop_front();
        self.hysteresis.push_back(*key_arr);
        let previous_state = self.hysteresis.get(self.hysteresis.len() - 2).unwrap();
        let front_state = self.hysteresis.front().unwrap();
        // Add Vector of events TBD

        // Manage per-key functions as each report gives us a snapshot of the whole matrix
        for (index, element) in keys.iter().enumerate() {
            // Short-circuit as it is unlikely all 20 keys are pressed.
            // If a key is not actuated and has a value of 0, ignore processing this round.
            if !(self.key_actions[index].actuated.0 ^ self.key_actions[index].actuated.1)
                && *element == 0
            {
                continue;
            }

            // Establish the direction of travel for the keys
            let run: Vec<f64> = self
                .hysteresis
                .iter()
                .map(|arr| arr[index] as f64)
                .collect();
            let trend = self.linear_regression(&run).unwrap_or_else(|| 0.0);

            // If we are not actuated, establish whether we should
            if !(self.key_actions[index].actuated.0 ^ self.key_actions[index].actuated.1) {
                // Exception for 1.5mm as any depression will trigger it
                if self.key_actions[index].actuation_point.0 == 0 && *element > 0 {
                    self.key_actions[index].actuated.0 = true;
                    // Do keydown stuff (1.5mm case)
                    log::info!("A1 Key {} Keydown @ {}!", index, *element);
                } else if trend >= TREND_KEYDOWN
                    && self.key_actions[index].actuation_point.0 <= *element
                {
                    self.key_actions[index].actuated.0 = true;
                    // Do keydown stuff (standard)
                    log::info!("A1 Key {} Keydown @ {}!", index, *element);
                }
            } else {
                // We are actuated, what happens now is based mainly on trends

                // Picking a trend criteria of ± 1 is important as it allows us to initialize
                // the retrigger distance given all numbers are in the same direction.
                // Using the keydown_up_n flag we can determine whether to look at only the
                // previous value or look at the start of the buffer.
                if trend >= TREND_KEYDOWN {
                    // Check if we need to perform dual function
                    if self.key_actions[index].actuation_point.1
                        > self.key_actions[index].actuation_point.0
                        && !self.key_actions[index].actuated.1
                        && self.key_actions[index].actuation_point.1 <= *element
                    {
                        self.key_actions[index].actuated.1 = true;
                        self.key_actions[index].actuated.0 = false;
                        // Do keydown stuff (second function)
                        log::info!("A1 Key {} Keyup @ {}!", index, *element);
                        log::info!("A2 Key {} Keydown @ {}!", index, *element);
                    }

                    // Manage retrigger
                    // retrigger_track uses negative and positive values.
                    // When negative we track reset, when positive we track triggering.
                    if self.key_actions[index].retrigger_go {
                        if self.key_actions[index].retrigger_track == 0.0 {
                            self.key_actions[index].retrigger_track =
                                *element as f64 - front_state[index] as f64;
                            self.key_actions[index].retrigger_track *= UNIT_TO_MM;
                        } else {
                            let difference = *element as f64 - previous_state[index] as f64;
                            self.key_actions[index].retrigger_track += difference * UNIT_TO_MM;
                        }

                        // Retrigger windows can be shared or separate.
                        // Manage the case where keydown is separate from keyup
                        if self.key_actions[index].retrigger_window.1 != 0.0
                            && self.key_actions[index].retrigger_track
                                >= self.key_actions[index].retrigger_window.1
                        {
                            self.key_actions[index].retrigger_go = false;
                            self.key_actions[index].retrigger_track = 0.0;
                            // Do keydown stuff
                            // Then the shared case.
                        } else if self.key_actions[index].retrigger_window.0 != 0.0
                            && self.key_actions[index].retrigger_track
                                >= self.key_actions[index].retrigger_window.0
                        {
                            self.key_actions[index].retrigger_go = false;
                            self.key_actions[index].retrigger_track = 0.0;
                            log::info!("Key {} retriggered!", index);
                            // Do keydown stuff
                        }
                    // Cases for backing off retrigger, such as travelling back down.
                    // We don't cancel, but we do effectively undo progress.
                    } else if self.key_actions[index].retrigger_track != 0.0 {
                        let difference: f64;
                        if !self.key_actions[index].keydown_up_n {
                            difference = *element as f64 - front_state[index] as f64;
                        } else {
                            difference = *element as f64 - previous_state[index] as f64;
                        }
                        self.key_actions[index].retrigger_track += difference * UNIT_TO_MM;

                        // Regardless if we totally undo retrigger progress, zero it rather than
                        // making it harder to perform next time.
                        if self.key_actions[index].retrigger_track > 0.0 {
                            self.key_actions[index].retrigger_track = 0.0;
                        }
                    }
                    self.key_actions[index].keydown_up_n = true;
                } else if trend <= TREND_KEYUP {
                    // Manage actuation point
                    if self.key_actions[index].actuation_point.0 >= *element {
                        // If the key returns to the neutral position everything resets.
                        if *element == 0 {
                            self.key_actions[index].retrigger_go = false;
                            self.key_actions[index].retrigger_track = 0.0;
                            if self.key_actions[index].actuated.0 {
                                self.key_actions[index].actuated = (false, false);
                                // Do keyup stuff
                                log::info!("Key {} Neutral! {:?}", index, run);
                                continue;
                            }
                        // If crt_en isn't covering for retrigger then stop when our
                        // actuation point has been met.
                        } else if !self.key_actions[index].crt_en {
                            self.key_actions[index].retrigger_go = false;
                            self.key_actions[index].retrigger_track = 0.0;
                            self.key_actions[index].actuated = (false, false);
                            // Do keyup stuff
                            log::info!("A1 Key {} Keyup @ {}!", index, *element);
                            continue;
                        }
                    }

                    if self.key_actions[index].actuation_point.1
                        > self.key_actions[index].actuation_point.0
                        && self.key_actions[index].actuation_point.1 >= *element
                        && self.key_actions[index].actuated.1
                    {
                        if !self.key_actions[index].crt_en {
                            self.key_actions[index].retrigger_go = false;
                            self.key_actions[index].retrigger_track = 0.0;
                            self.key_actions[index].actuated.1 = false;
                            // Do keyup stuff
                            log::info!("A2 Key {} Keyup @ {}!", index, *element);
                            continue;
                        }
                    }
                    // Manage retrigger
                    // The tuple representing the retrigger window is conditionally set.
                    // If the tracker is 0, then initialise the counter from where we are.
                    if !self.key_actions[index].retrigger_go
                        && self.key_actions[index].retrigger_window.0 != 0.0
                    {
                        let difference: f64;
                        if self.key_actions[index].keydown_up_n {
                            difference = front_state[index] as f64 - *element as f64;
                        } else {
                            difference = previous_state[index] as f64 - *element as f64;
                        }
                        self.key_actions[index].retrigger_track -= difference * UNIT_TO_MM;

                        if self.key_actions[index].retrigger_window.0
                            <= self.key_actions[index].retrigger_track.abs()
                        {
                            log::info!("Key {} retrigger armed!", index);
                            self.key_actions[index].retrigger_track = 0.0;
                            self.key_actions[index].retrigger_go = true;
                            // Do keyup stuff
                        }
                    }
                    self.key_actions[index].keydown_up_n = false;
                }
            }
        }

        Ok(Vec::new())
    }

    fn linear_regression(&self, data: &[f64]) -> Option<f64> {
        // Implement simple linear regression equation
        // m = (n*Σxy - ΣxΣy) / (n*Σx² - (Σx)²)
        // where n >= 2 (defined by length of data)

        let n = data.len() as f64;
        if n < 2.0 {
            return None;
        }
        let sum_x: f64 = (0..data.len()).map(|i| i as f64).sum();
        let sum_y: f64 = data.iter().sum();
        let sum_xy: f64 = data.iter().enumerate().map(|(i, &y)| i as f64 * y).sum();
        let sum_xx: f64 = (0..data.len()).map(|i| (i as f64).powi(2)).sum();

        let numerator = n * sum_xy - sum_x * sum_y;
        let denominator = n * sum_xx - sum_x.powi(2);

        if denominator == 0.0 {
            return Some(0.0);
        }
        Some(numerator / denominator)
    }
}
