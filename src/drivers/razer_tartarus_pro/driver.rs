use super::event::{Event, KeyCodes};
use crate::udev::device::UdevDevice;
use hidapi::HidDevice;
use std::collections::VecDeque;
use std::{error::Error, ffi::CString, fs, io::Write, path::Path};

pub const VID: u16 = 0x1532;
pub const PID: u16 = 0x0244;

const ANALOG_MODE: [u8; 2] = [0x3, 0x0];
const BASIC_MODE: [u8; 2] = [0x0, 0x0];
const MESSAGE_DATA_PAYLOAD: usize = 24;
const RAZER_FEATURE_ENDPOINT: i32 = 2;
const TREND_KEYDOWN: f64 = 1.0;
const TREND_KEYUP: f64 = -1.0;
const REGRESSION_WINDOW: usize = 5;
const KEY_COUNT: usize = 20;
const UNIT_TO_MM: f64 = 0.1 / 12.0;

pub struct Driver {
    device: HidDevice,
    control_path: String,
    hysteresis: VecDeque<[u8; KEY_COUNT]>,
    key_actions: [AnalogAction; KEY_COUNT],
    key_state: Vec<KeyCodes>,
}

/// Stores threshold values derived from user profile and provides state machine memory for a
/// given key
#[derive(Clone, Copy, Default, Debug)]
pub struct AnalogAction {
    /// User setting (primary function, secondary function), must be > 0 to be active.
    actuation_point: (u8, u8),
    /// User setting (downward, upward), > 0 activates. If upward == 0 it will use downward value.
    retrigger_threshold: (f64, f64),
    /// User setting from profile, enable continuous retrigger in algorithm.
    crt_en: bool,
    /// (primary binding, secondary binding), captures whether the keystroke is in actuated range
    actuated: (bool, bool),
    /// Buffer for tracking the retrigger window in mm
    retrigger_track: f64,
    /// Tracks eligibility for retrigger
    retrigger_go: bool,
    /// Reflects the previous trend
    key_is_upstroke: bool,
    /// Do not immediately activate first function when second function resets in dual function.
    backoff_first_func: bool,
}

/// Find the location of control nodes for the razerkbd driver instance
fn get_razerkbd_controls(hidraw_name: &str) -> Option<String> {
    // Log whether or not the razerkbd module is loaded on the system
    if Path::new(&String::from(format!("/sys/module/razerkbd"))).exists() {
        log::info!("razerkbd module detected on system");
    } else {
        log::info!("No module detected on system");
    }

    // Get name from a path (/dev/hidrawX) or node name
    let node = hidraw_name
        .rsplit_once('/')
        .map(|(_, suffix)| suffix)
        .unwrap_or(hidraw_name);

    // Map hidraw to the USB identifier then check if it exists within razerkbd
    let sys_path = format!("/sys/class/hidraw/{}/device", node);

    if let Ok(target) = fs::read_link(sys_path) {
        let razer_path = String::from(format!(
            "/sys/bus/hid/drivers/razerkbd/{}",
            target
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap()
        ));
        if Path::new(&razer_path).exists() {
            return Some(razer_path);
        }
    }
    None
}

/// This driver implementation is shared across all three HID handles on the
/// Tartarus Pro. Different interfaces traverse different code-paths though.
/// Refer to handle_input_report()
impl Driver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let hidrawpath = udevice.devnode();
        let cs_path = CString::new(hidrawpath.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;
        let info = device.get_device_info()?;
        let control_path: String;

        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{hidrawpath}' is not a Razer Tartarus Pro").into());
        }

        if info.interface_number() == RAZER_FEATURE_ENDPOINT {
            if let Some(path) = get_razerkbd_controls(&hidrawpath) {
                log::info!("Driver controls available at {:?}", path);
                control_path = path;
            } else {
                control_path = String::default();
            }

            // TODO based on the loaded user profile:
            // - Check if profile asks for features that require Kernel module,
            //   fail if module is not loaded.
            // - Validate device matching if defined (serial # specific profiles)
            // - Set hardware mode as required
            // END TODO

            // TODO DEMO set analog mode using driver, to refactor before release
            let mode_path = format!("{}/{}", &control_path, "device_mode");
            let mut file = fs::OpenOptions::new().write(true).open(mode_path)?;
            file.write_all(&ANALOG_MODE)?;
            log::info!("Device mode set");
            // TODO DEMO END
        } else {
            control_path = String::default();
        }

        // Create a 20x5 null matrix to initialize the buffer supporting key hysteresis
        let mut zeroes = VecDeque::with_capacity(5);
        zeroes.extend([[0; KEY_COUNT]; REGRESSION_WINDOW]);

        let mut demo_config = Self {
            device,
            hysteresis: zeroes,
            control_path: control_path,
            key_actions: [AnalogAction::default(); KEY_COUNT],
            key_state: Vec::new(),
        };
        // TODO DEMO Remove, demo allocations in lieu of profile support
        // Can be customised to suit
        demo_config.key_actions[0].actuation_point = (0x24, 0x78);
        demo_config.key_actions[0].retrigger_threshold = (0.0, 0.0);
        demo_config.key_actions[0].crt_en = false;
        demo_config.key_actions[1].actuation_point = (0x45, 0x0);
        demo_config.key_actions[1].retrigger_threshold = (0.3, 0.0);
        demo_config.key_actions[1].crt_en = false;
        demo_config.key_actions[2].actuation_point = (0x50, 0x0);
        demo_config.key_actions[2].retrigger_threshold = (0.5, 0.0);
        demo_config.key_actions[2].crt_en = true;
        demo_config.key_actions[3].actuation_point = (0x50, 0x0);
        demo_config.key_actions[3].retrigger_threshold = (0.5, 0.8);
        demo_config.key_actions[3].crt_en = true;
        // TODO DEMO END

        Ok(demo_config)
    }

    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0; MESSAGE_DATA_PAYLOAD];
        let bytes_read = self.device.read(&mut buf[..])?;
        let slice = &buf[..bytes_read];
        let events = self.handle_input_report(slice, bytes_read)?;
        Ok(events)
    }

    /// Routes an input report to the appropriate hander based on what endpoint received it
    /// and the report ID
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

    /// Manage all input reports other than the analog function.
    /// The basic case for all 3 endpoints is essentially identical. The first byte is unique for
    /// each; For endpoints 1.1 and 1.3 this is where code 0x04 is interpreted and discarded for
    /// endpoint 1.2 as it was a report ID which has served its purpose if we got this far.
    /// Given the conversion to variant space it is best removed to stop confusion.
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

    /// Manage analog input reports and translate to key strokes
    ///
    /// Concepts:
    ///
    /// Measurement
    /// A key in analog mode returns an 8-bit value representing height displacement with 0
    /// representing the top (~1.5mm) and 255 when it is bottomed out (~3.6mm).
    /// The OEM software has defined a minimum discernable step size as 0.1mm which we will adopt.
    /// We define a 'unit' which is total key travel (2.1mm) divided by quantization level (255)
    /// noting that '0' is a parked value as we measure changes from 0.
    /// With quantization error we declare that 0.1mm is equivalent to 12 (0xC) units and this
    /// forms the basis of determining the displacement of a key.
    ///
    /// For a given keystroke it is not expected to see all values between 0 to 255. The guaranteed
    /// value is 0 and 255 if you bottom out. All other values are based on poll rate and
    /// incidental location at time of sampling. The other guarantee is trend - if you press
    /// down, the value goes up and vice-versa. During development the shortest run of values
    /// measured on the downstroke (i.e. slamming the key down) was 5. Uncontrolled return
    /// (lifting off) also generated 5 to 6 values. If you gradually move the key are you able to
    /// get fidelity down to individual units (0.00833 mm) but this generally a slow motion and not
    /// reflective of all usage patterns. To account for the variable displacement speed and the
    /// resultant discontinuous values when measuring a stroke, linear regression is used to
    /// establish the direction of travel and forms the basis of mapping displacement to actions.
    /// A 5-sample deep buffer matches the observed shortest run of values during a keystroke.
    /// Picking a trend criteria of ± 1 is important as that tells us all numbers in the buffer
    /// are trending in the same direction.
    ///
    /// Reports
    /// The Tartarus Pro generates reports on the change of any one of its analog keys.
    /// The report contains the state of all 20 keys (bytes) with no indication as to what key
    /// triggered it. Keys are mapped to fixed offsets in the report making translation straight
    /// forward. In the case of this function it just sees an array of key values to process with
    /// no further interpretation.
    ///
    /// Algorithm:
    ///
    /// A keystroke is defined as starting at the top, displaces downwards & optionally
    /// hovers around a point then displaces again downward or upward, optionally hovering
    /// and repeat until eventually returning to the top. To map an event we need to know from
    /// the user what displacement should be used as the event trigger and then everything else
    /// is filtering as analog-optical keyboards continuously sample during travel.
    ///
    /// The summary of key processing is
    /// - Update group hysteresis
    /// - Check if this key should be actuated or ignored this cycle
    /// - If actuated previously get the key direction of travel
    /// - For a given direction
    ///   * Manage special cases, dual-function on downstroke and reset cases on upstroke
    ///   * Manage direction specific retrigger functions
    ///
    /// A key returning to the top (zeroing) resets any state associated with that key's travel.
    ///
    /// Analog Methods:
    ///
    /// Dual-Function
    /// Allocate two actuation points to a key which can be mapped to two different events.
    /// Triggered on the downstroke, the sequence goes f1 keydown -> f1 keyup / f2 keydown.
    /// On the upstroke f2 keyup is triggered at the initial f2 actuation point. The functions do
    /// not trigger again until the key is above their respective initial actuation points noting
    /// that the functions trigger independently e.g. f2 keyup does not trigger f1 keydown, but
    /// f2 keydown can occur again if the key changes direction.
    ///
    /// Retrigger
    /// For a key allocated a single actuation point instead of resetting only at this point
    /// it can be reset after a user defined about of negative displacement under actuation.
    /// Once reset (or armed) a user defined amount of positive displacement will retrigger the
    /// key. This will continue until the set actuation point is reached on the upstroke, in which
    /// case the retrigger logic will be disabled. Users can enable 'continuous retrigger'
    /// which changes this to only disable the retrigger logic only once the key is at the top of
    /// its travel regardless of the profile actuation point.
    /// The retrigger displacement can be a shared value or defined independently.
    ///
    /// Dual function and retrigger on a specific key is an either/or affair at this stage.
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
        //  TODO PLumb in
        let mut events: Vec<Event> = Vec::new();
        // END TODO

        // Manage per-key functions as each report gives us a snapshot of the whole matrix
        for (key, displacement) in keys.iter().enumerate() {
            // Calculate and label decisions for readability
            let first_func_actd = self.key_actions[key].actuated.0;
            let first_func_act_point = self.key_actions[key].actuation_point.0;
            let first_func_reset_rule = first_func_actd && (first_func_act_point >= *displacement);
            let second_func_actd = self.key_actions[key].actuated.1;
            let second_func_act_point = self.key_actions[key].actuation_point.1;
            let second_func_user_valid = second_func_act_point > first_func_act_point;
            let second_func_reset_rule = (second_func_act_point >= *displacement)
                && second_func_user_valid
                && second_func_actd;
            let continuous_retrigger = self.key_actions[key].crt_en;
            let backoff_first_func = self.key_actions[key].backoff_first_func;
            let func_actd = first_func_actd ^ second_func_actd;
            let retrigger_shared_value = self.key_actions[key].retrigger_threshold.0;
            let retrigger_downstroke_value = self.key_actions[key].retrigger_threshold.1;

            // Perform starting actuation checks if we aren't backing off or we aren't actuated.
            if !func_actd && !backoff_first_func {
                // Short-circuit as it is unlikely all 20 keys are pressed.
                // If a key is not actuated and has a value of 0, ignore processing this round.
                if *displacement == 0 {
                    continue;
                }

                // Actuate if the following criteria are met
                if first_func_act_point == 0 && *displacement > 0 {
                    // First the exception case where actuation is set to 0 (1.5mm) as
                    // any displacement will trigger it.
                    self.key_actions[key].actuated.0 = true;
                    // TODO keydown stuff (1.5mm case). Logged for demo purposes
                    log::info!("A1 Key {} Keydown @ {}!", key, *displacement);
                } else if first_func_act_point <= *displacement {
                    // Finally general case where key is at or exceeds actuation point.
                    self.key_actions[key].actuated.0 = true;
                    // TODO keydown stuff (all other cases), logged for demo purposes
                    log::info!("A1 Key {} Keydown @ {}!", key, *displacement);
                }
                continue;
            }

            // As we have an actuation, track behavior using direction of travel and issue
            // events per user profile.

            // Establish the direction of travel for the key
            let run: Vec<f64> = self.hysteresis.iter().map(|arr| arr[key] as f64).collect();
            let trend = self.linear_regression(&run).unwrap_or_else(|| 0.0);

            // Manage a key moving downwards
            if trend >= TREND_KEYDOWN {
                // Check if we need to perform dual function
                if second_func_user_valid
                    && !second_func_actd
                    && second_func_act_point <= *displacement
                {
                    if first_func_actd {
                        // TODO Keyup stuff for A1, logged for demo purposes
                        log::info!("A1 Key {} Keyup @ {}!", key, *displacement);
                        self.key_actions[key].actuated.0 = false;
                    }
                    // TODO Keydown stuff for A2, logged for demo purposes
                    log::info!("A2 Key {} Keydown @ {}!", key, *displacement);
                    self.key_actions[key].actuated.1 = true;
                }

                // Manage retrigger events
                // This assumes we are armed from keyup
                if self.key_actions[key].retrigger_go {
                    // Calculate displacement track value based on whether we've changed direction
                    if self.key_actions[key].retrigger_track == 0.0 {
                        self.key_actions[key].retrigger_track =
                            *displacement as f64 - front_state[key] as f64;
                        self.key_actions[key].retrigger_track *= UNIT_TO_MM;
                    } else {
                        let difference = *displacement as f64 - previous_state[key] as f64;
                        self.key_actions[key].retrigger_track += difference * UNIT_TO_MM;
                    }
                    // Check if we can retrigger, firstly if the downstroke value is defined
                    if retrigger_downstroke_value != 0.0
                        && self.key_actions[key].retrigger_track >= retrigger_downstroke_value
                    {
                        // Retrigger windows can be shared or separate.
                        // Manage the case where keydown is separate from keyup
                        self.key_actions[key].retrigger_go = false;
                        self.key_actions[key].retrigger_track = 0.0;
                        // TODO keydown stuff, Logged for demo purposes
                        log::info!("Key {} separate value retrigger!", key);
                    } else if retrigger_downstroke_value == 0.0
                        && self.key_actions[key].retrigger_track >= retrigger_shared_value
                    {
                        // Otherwise check again using the shared point for keydown
                        self.key_actions[key].retrigger_go = false;
                        self.key_actions[key].retrigger_track = 0.0;
                        // TODO keydown stuff, Logged for demo purposes
                        log::info!("Key {} shared value retrigger!", key);
                    }
                } else if self.key_actions[key].retrigger_track != 0.0 {
                    // Calculate and apply retrigger resistance
                    // retrigger_track uses negative and positive displacement.
                    // When negative we track reset, when positive we track triggering.
                    // Normally with a retrigger sequence the key moves up then down on one motion.
                    // But if you move up then down then up again you can 'undo' retriggering
                    // progress so much so as to negate it. In such case we call it zero
                    // and do nothing more, otherwise it just increases difficulty to retrigger.
                    let difference: f64;
                    if self.key_actions[key].key_is_upstroke {
                        difference = *displacement as f64 - front_state[key] as f64;
                    } else {
                        difference = *displacement as f64 - previous_state[key] as f64;
                    }
                    self.key_actions[key].retrigger_track += difference * UNIT_TO_MM;
                    // If we totally undo retrigger progress, zero it rather than
                    // making it harder to perform next time.
                    if self.key_actions[key].retrigger_track > 0.0 {
                        self.key_actions[key].retrigger_track = 0.0;
                    }
                }
                self.key_actions[key].key_is_upstroke = false;
            }

            // Manage a key moving upwards
            if trend <= TREND_KEYUP {
                // Manage actuation point reset
                if first_func_reset_rule || second_func_reset_rule {
                    // If the key returns to the top everything resets.
                    // If crt_en isn't covering for retrigger then stop when our
                    // actuation point has been met.
                    if *displacement == 0 || !continuous_retrigger {
                        // TODO These logs clarify the entry state, remove once
                        // events are plumbed in.
                        // Case for where actuation point is 0
                        if first_func_actd && *displacement == 0 {
                            // TODO this is effectively a NOP, logged to track state.
                            log::info!("Key {} reached top! {:?}", key, run);
                        } else if second_func_reset_rule {
                            // If the second function resets we have to prevent the first
                            // function from actuating until we reach its reset point.
                            // If left as-is it will trigger immediately as the function is
                            // in its actuation window.
                            // TODO keyup stuff, Logged for demo purposes
                            log::info!("A2 Key {} Keyup @ {}!", key, *displacement);
                            if *displacement > 0 {
                                self.key_actions[key].backoff_first_func = true;
                            }
                        } else {
                            // TODO Do keyup stuff, Logged for demo purposes
                            log::info!("A1 Key {} Keyup @ {}!", key, *displacement);
                        }

                        // Update persistent state
                        self.key_actions[key].retrigger_go = false;
                        self.key_actions[key].retrigger_track = 0.0;
                        self.key_actions[key].actuated = (false, false);
                        continue;
                    }
                }
                // Remove the actuation block in dual function once we've reached reset
                // for the first function.
                if backoff_first_func && first_func_act_point >= *displacement {
                    self.key_actions[key].backoff_first_func = false;
                }
                // Manage retrigger for upward keystrokes
                // If we've transitioned state (from down to up) then our initial
                // displacement is calculated from the start of the regression window. If
                // continuing in the same direction then the displacement accumulates from
                // the previous reading. The displacement is compared against the retrigger
                // threshold, in the case of upwards stroke reaching the threshold 'arms' the
                // downward stroke allowing it to issue a retrigger. There can be separate
                // thresholds for upwards and downwards displacement per the profile.
                // Retrigger is disabled when dual-function is defined for a key
                // pending a plausible use case. This matches OEM software behavior.
                if !self.key_actions[key].retrigger_go
                    && retrigger_shared_value != 0.0
                    && !second_func_user_valid
                {
                    let difference: f64;
                    // Using the key_is_upstroke flag we can determine whether to look at only the
                    // previous value or look at the start of the buffer.
                    if !self.key_actions[key].key_is_upstroke {
                        difference = front_state[key] as f64 - *displacement as f64;
                    } else {
                        difference = previous_state[key] as f64 - *displacement as f64;
                    }
                    self.key_actions[key].retrigger_track -= difference * UNIT_TO_MM;
                    if retrigger_shared_value <= self.key_actions[key].retrigger_track.abs() {
                        // TODO do keyup stuff, logged for demo purposes
                        log::info!("Key {} retrigger armed!", key);
                        self.key_actions[key].retrigger_track = 0.0;
                        self.key_actions[key].retrigger_go = true;
                    }
                }
                self.key_actions[key].key_is_upstroke = true;
            }
        }
        Ok(events)
    }

    /// Implement the simple linear regression equation
    /// m = (n*Σxy - ΣxΣy) / (n*Σx² - (Σx)²)
    /// where n >= 2 (defined by length of data)
    fn linear_regression(&self, data: &[f64]) -> Option<f64> {
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
