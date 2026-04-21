use super::event::{Event, KeyCodes, ANALOG_KEY_CODES};
use crate::config::SourceDevice;
use crate::udev::device::UdevDevice;
use hidapi::HidDevice;
use std::collections::VecDeque;
use std::{error::Error, ffi::CString, fs, io::Write, path::Path};

pub const VID: u16 = 0x1532;
pub const PID: u16 = 0x0244;

const ENDPOINT_1: i32 = 0;
const ENDPOINT_2: i32 = 1;
const ENDPOINT_3: i32 = 2;
const ANALOG_MODE: [u8; 2] = [0x3, 0x0];
const BASIC_MODE: [u8; 2] = [0x0, 0x0];
const SHORT_DATA_PAYLOAD: usize = 8;
const LONG_DATA_PAYLOAD: usize = 24;
const TREND_KEYDOWN: f64 = 1.0;
const TREND_KEYUP: f64 = -1.0;
const REGRESSION_WINDOW: usize = 5;
const KEY_COUNT: usize = 20;
const CONFIG_STEP_SIZE_MM: f64 = 0.1;
const KEY_TOP_MM: f64 = 1.4;
const KEY_BOTTOM_MM: f64 = 3.6;
const KEY_TRAVEL: f64 = KEY_BOTTOM_MM - KEY_TOP_MM - CONFIG_STEP_SIZE_MM;
const UNIT_TO_MM: f64 = CONFIG_STEP_SIZE_MM / 12.0;

pub struct Driver {
    device: HidDevice,
    control_path: String,
    hysteresis: VecDeque<[u8; KEY_COUNT]>,
    key_actions: [AnalogAction; KEY_COUNT],
    key_state: Vec<KeyCodes>,
}

/// Structure for per-key threshold values from profile and state machine memory
#[derive(Clone, Copy, Default, Debug)]
pub struct AnalogAction {
    /// User setting (primary function, secondary function), must be > 0 to be active.
    actuation_point: (u8, u8),
    /// User setting (downward, upward), > 0 activates. If upward == 0 it will use downward value.
    retrigger_threshold: (f64, f64),
    /// User setting enable continuous retrigger in algorithm.
    crt_en: bool,
    /// Captures whether the keystroke is in actuated range (primary binding, secondary binding)
    actuated: (bool, bool),
    /// Buffer for tracking the retrigger window in mm
    retrigger_track: f64,
    /// Tracks eligibility for retrigger
    retrigger_go: bool,
    /// Tracks keystroke direction of previous report
    key_is_upstroke: bool,
    /// Flag to prevent early actuation of primary function during secondary function reset
    backoff_first_func: bool,
}

/// Find the location of control nodes for the razerkbd driver instance
fn get_razerkbd_controls(hidraw_name: &str) -> Option<String> {
    // Log whether or not the razerkbd module is loaded on the system
    if Path::new(&String::from(format!("/sys/module/razerkbd"))).exists() {
        log::info!("razerkbd module detected on system");
    } else {
        log::info!("No Kernel module detected on system");
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

/// Translates an absolute key actuation point in mm with sanitised precision to a u8 unit value
fn convert_actuation_to_unit(value: f64) -> u8 {
    if value < KEY_TOP_MM || value > KEY_BOTTOM_MM {
        if value != 0.0 {
            log::error!(
                "Invalid range f64 value found in config: {}, treating as 0",
                value
            );
        }
        return 0;
    }

    let zeroed = value - KEY_TOP_MM;
    let remainder = zeroed.rem_euclid(CONFIG_STEP_SIZE_MM);
    let epsilon = 1e-10;

    if remainder < epsilon || (CONFIG_STEP_SIZE_MM - remainder).abs() < epsilon {
        return (zeroed / UNIT_TO_MM) as u8;
    }
    log::error!(
        "Invalid precision f64 value found in config: {}, treating as 0",
        value
    );
    0
}

/// Sanitise a relative displacement value to ensure it is in the correct range and precision
fn validate_retrigger_value(value: f64) -> f64 {
    if value < 0.0 || value > KEY_TRAVEL {
        log::error!(
            "Invalid range f64 value found in config: {}, treating as 0",
            value
        );
        return 0.0;
    }

    let remainder = value.rem_euclid(CONFIG_STEP_SIZE_MM);
    let epsilon = 1e-10;

    if remainder < epsilon || (CONFIG_STEP_SIZE_MM - remainder).abs() < epsilon {
        return value;
    }
    log::error!(
        "Invalid precision f64 value found in config: {}, treating as 0",
        value
    );
    0.0
}

/// This driver implementation is shared across all three HID handles on the
/// Tartarus Pro. Different interfaces traverse different code-paths.
/// Refer to handle_input_report()
impl Driver {
    pub fn new(
        udevice: UdevDevice,
        conf: Option<SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let hidrawpath = udevice.devnode();
        let cs_path = CString::new(hidrawpath.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;
        let info = device.get_device_info()?;
        let control_path: String;

        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{hidrawpath}' is not a Razer Tartarus Pro").into());
        }

        if info.interface_number() == ENDPOINT_3 {
            if let Some(path) = get_razerkbd_controls(&hidrawpath) {
                log::info!("Driver controls available at {:?}", path);
                control_path = path;
            } else {
                control_path = String::default();
                log::info!("Driver controls are not available, cannot determine device state");
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
            log::info!("Analog key mode set on device");
            // TODO DEMO END
        } else {
            control_path = String::default();
        }

        // Create a 20x5 null matrix to initialize the buffer that implements key hysteresis
        let mut zeroes = VecDeque::with_capacity(REGRESSION_WINDOW);
        zeroes.extend([[0; KEY_COUNT]; REGRESSION_WINDOW]);

        let mut tartarus = Self {
            device,
            hysteresis: zeroes,
            control_path: control_path,
            key_actions: [AnalogAction::default(); KEY_COUNT],
            key_state: Vec::new(),
        };

        // Read in analog key config and apply
        if info.interface_number() == ENDPOINT_2 {
            tartarus.key_action_config(conf);
        }
        Ok(tartarus)
    }

    /// Allocate analog key properties from device YAML to key_action structs
    fn key_action_config(&mut self, conf: Option<SourceDevice>) {
        if conf.is_some() {
            if let Some(analog_keys) = conf
                .map(|s| s.config.as_ref()?.analogkeys.clone())
                .unwrap_or(None)
            {
                if let Some(property) = analog_keys.primary_actuation {
                    for (s, new_val) in self.key_actions.iter_mut().zip(property.keys.iter()) {
                        s.actuation_point.0 = convert_actuation_to_unit(*new_val);
                    }
                }
                if let Some(property) = analog_keys.secondary_actuation {
                    for (s, new_val) in self.key_actions.iter_mut().zip(property.keys.iter()) {
                        s.actuation_point.1 = convert_actuation_to_unit(*new_val);
                    }
                }
                if let Some(property) = analog_keys.retrigger_reset_threshold {
                    for (s, new_val) in self.key_actions.iter_mut().zip(property.keys.iter()) {
                        s.retrigger_threshold.0 = validate_retrigger_value(*new_val);
                    }
                }
                if let Some(property) = analog_keys.retrigger_trigger_threshold {
                    for (s, new_val) in self.key_actions.iter_mut().zip(property.keys.iter()) {
                        s.retrigger_threshold.1 = validate_retrigger_value(*new_val);
                    }
                }
                if let Some(property) = analog_keys.continuous_retrigger {
                    for (s, new_val) in self.key_actions.iter_mut().zip(property.keys.iter()) {
                        s.crt_en = *new_val;
                    }
                }
            }
        }
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0; LONG_DATA_PAYLOAD];
        let bytes_read = self.device.read(&mut buf[..])?;
        let slice = &buf[..bytes_read];
        let events = self.handle_input_report(slice, bytes_read)?;
        Ok(events)
    }

    /// Route an input report to the appropriate handler based on origin endpoint and report ID
    fn handle_input_report(
        &mut self,
        buf: &[u8],
        bytes_read: usize,
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let info = self.device.get_device_info()?;

        // Each endpoint generates different types of reports. Whilst each handle is associated
        // with a single endpoint this code is common to all.
        // - Endpoint 1 is fully described by its HID report descriptor and relates to the
        //   D-pad and the aux key on the Tartarus.
        // - Endpoint 2 has two reports, if report ID 1 is seen this is a normal keyboard report
        //   and is fully described by the HID report descriptor. If report ID 6 is seen this is
        //   vendor defined and reflects analog mode which is handled separately. Other reports are
        //   defined in the descriptor table but are seemingly unused.
        //   This interface regardless of report captures the 20 numbered keys.
        // - Endpoint 3 like endpoint 1 is fully described by its HID report descriptor.
        //   This interface captures the scroll wheel and also serves as the control interface.

        match info.interface_number() {
            ENDPOINT_1 => {
                if bytes_read == SHORT_DATA_PAYLOAD {
                    self.handle_basic(buf, KeyCodes::PhantomAux, false)
                } else {
                    Ok(Vec::new())
                }
            }
            ENDPOINT_2 => {
                if bytes_read == LONG_DATA_PAYLOAD {
                    match buf[0] {
                        // Only manage report IDs 1 & 6. Other report types are defined in firmware
                        // but don't appear to be used.
                        0x1 => return self.handle_basic(buf, KeyCodes::PhantomBlank, true),
                        0x6 => return self.handle_analog(&buf[1..21]),
                        _ => Ok(Vec::new()),
                    }
                } else {
                    Ok(Vec::new())
                }
            }
            ENDPOINT_3 => {
                if bytes_read == SHORT_DATA_PAYLOAD {
                    self.handle_basic(buf, KeyCodes::PhantomMClick, false)
                } else {
                    Ok(Vec::new())
                }
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Manage input reports for endpoints 1 and 3
    fn handle_basic(
        &mut self,
        buf: &[u8],
        key_replace: KeyCodes,
        overwrite: bool,
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut events = Vec::new();
        let mut pad_state: Vec<KeyCodes> = buf.iter().map(|&s| KeyCodes::from(s)).collect();

        // Override Byte 0 as specified and remove any blanks. This refers to the overloaded value
        // of 0x04 and is managed differently based on which endpoint sent it.
        if pad_state[0] == KeyCodes::KeyTwelve || overwrite {
            pad_state[0] = key_replace;
        }
        pad_state.retain(|x| *x != KeyCodes::PhantomBlank);

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

    /// # Analog Mode Concepts
    ///
    /// ## Measurement
    /// A key in analog mode returns an 8-bit value representing height displacement with 0
    /// representing the top (~1.5mm) and 255 when it is bottomed out (~3.6mm).
    /// The OEM software has defined a minimum discernable step size as 0.1mm which we will adopt.
    /// We define a 'unit' which is total key travel (2.1mm) divided by quantization level (255)
    /// noting that '0' is a parked value as we measure changes from 0. With quantization error we
    /// declare that 0.1mm is equivalent to 12 (0xC) units.
    ///
    /// For a given keystroke it is not expected to see all values between 0 to 255. The guaranteed
    /// value is 0, and then 255 if you bottom out. All other values are based on poll rate and
    /// incidental key location at time of sampling. The other guarantee is trend - if you press
    /// down the value goes up and vice-versa. During development the shortest run of values
    /// measured on the downstroke (i.e. slamming the key down) was 5. Uncontrolled release
    /// generates 5 to 6 values. If you carefully manipulate the key you are able to make changes
    /// down to the unit value (0.00833 mm) but this is generally a slow motion and not reflective
    /// of all usage patterns. To account for the variable displacement speed and resultant
    /// discontinuous values when measuring a stroke, linear regression is used to establish the
    /// direction of travel and forms the basis of mapping displacement to actions. A 5-sample
    /// deep buffer matches the observed shortest run of values during a keystroke. Picking a trend
    /// criteria of ± 1 is important as this filters out key jitter (from say holding position)
    /// giving a clear indication as to whether a key is moving up or down.
    ///
    /// As the true values of the actuation range are somewhat ambiguous, for the purposes of
    /// managing configuration we have declared 1.4mm as the 'top-top' point which translates to
    /// unit value 0 allowing 3.6mm to translate (clamp) to 255. This does mean that 3.5mm is
    /// mapped to 252 instead of 243 when using 12 unit spacing.
    ///
    /// ## Reports
    /// Any change from any one of the Tartarus pro analog keys will generate an input report.
    /// The report (ID 6) contains the state of all 20 keys as individual bytes with no indication
    /// as to which key triggered its generation. Keys are mapped to fixed offsets in the report
    /// (0-19). The report ID byte is assumed to be dropped allowing for 1:1 mapping of keys to the
    /// displacement values.
    ///
    /// # Algorithm:
    ///
    /// A keystroke is defined as starting from the top, moving downwards & holding about a point
    /// until the desired effect is seen by the user. There may be subsequent move and holds but
    /// the key will eventually return to the top. To map an event we need to know from the user
    /// what displacement should be used as the event trigger, everything else from that point is
    /// filtering as analog-optical keyboards sample continuously during travel.
    ///
    /// The summary of key processing is
    /// - Push the report into the buffer, maintaining its depth of 5.
    /// - Iterate over each key in the current report:
    ///   * Check if this key should be actuated or ignored this cycle
    ///   * If actuated already get the key's direction of travel by slicing the buffer at the
    ///     key's offset and perform linear regression on the run to perform further functions
    ///   * With the direction determined:
    ///     + Manage direction specific retrigger functions
    ///     + Manage special cases, dual-function on downstroke and reset cases on upstroke
    ///
    /// A key returning to the top (zeroing) resets any state associated with that key's travel.
    ///
    /// # Analog Methods:
    ///
    /// ## Variable actuation
    ///   A key event can be mapped to a specific key displacement threshold as specified in the
    ///   device YAML file. Keydown and keyup events share the same threshold triggered based on
    ///   direction of travel.
    ///
    ///   A key may also have one of the following methods applied to it:
    /// ## Dual-Function
    ///   Allocate two actuation points to a key which can be mapped to two different events.
    ///   Triggered on the downstroke, the sequence goes f1 keydown -> f1 keyup / f2 keydown.
    ///   On the upstroke f2 keyup is triggered at the initial f2 actuation point. The functions do
    ///   not trigger again until the key is above their respective initial actuation points noting
    ///   that the functions trigger independently e.g. f2 keyup does not trigger f1 keydown, but
    ///   f2 keydown can occur again if the key changes direction.
    ///
    /// ## Retrigger
    ///   For a key allocated a single actuation point its reset state can be redefined to occur
    ///   after an amount of negative displacement instead of returning to the actuation point.
    ///   Once reached a user defined amount of positive displacement will retrigger the key.
    ///   This will continue until the set actuation point is reached on the upstroke, in which
    ///   case the retrigger logic will be disabled. Users can enable 'continuous retrigger'
    ///   which changes this to only disable the retrigger logic only once the key is at the top of
    ///   its travel regardless of the profile actuation point.
    ///   The retrigger displacement can be a shared value or defined independently.

    /// Manage analog input reports and translate to keystroke events
    fn handle_analog(&mut self, keys: &[u8]) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let key_arr: &[u8; KEY_COUNT];
        let mut events: Vec<Event> = Vec::new();

        if let Ok(value) = <&[u8] as TryInto<&[u8; KEY_COUNT]>>::try_into(keys) {
            key_arr = value;
        } else {
            log::error!("Incorrect size array passed to handle_analog");
            return Ok(events);
        }

        // Update hysteresis with the current report
        self.hysteresis.pop_front();
        self.hysteresis.push_back(*key_arr);

        // Alias local parameters for comprehension
        let previous_state = self.hysteresis.get(self.hysteresis.len() - 2).unwrap();
        let front_state = self.hysteresis.front().unwrap();

        // Iterate through each key using the incoming report
        for (key, displacement) in keys.iter().enumerate() {
            // Alias local parameters for comprehension
            let first_func_actd = self.key_actions[key].actuated.0;
            let second_func_actd = self.key_actions[key].actuated.1;
            let first_func_act_point = self.key_actions[key].actuation_point.0;
            let second_func_act_point = self.key_actions[key].actuation_point.1;
            let retrigger_shared_value = self.key_actions[key].retrigger_threshold.0;
            let retrigger_downstroke_value = self.key_actions[key].retrigger_threshold.1;
            let continuous_retrigger = self.key_actions[key].crt_en;
            let backoff_first_func = self.key_actions[key].backoff_first_func;

            // State machine traversal logic
            let func_actd = first_func_actd ^ second_func_actd;
            let first_func_reset_rule = first_func_actd && (first_func_act_point >= *displacement);
            let second_func_user_valid = second_func_act_point > first_func_act_point;
            let second_func_reset_rule = (second_func_act_point >= *displacement)
                && second_func_user_valid
                && second_func_actd;

            // Perform initial actuation checks, assuming key is not actuated.
            if !func_actd && !backoff_first_func {
                // Short-circuit as it is unlikely all 20 keys are pressed.
                // If a key is not actuated and has a value of 0, ignore processing this round.
                if *displacement == 0 {
                    continue;
                }

                // Actuate if the following criteria are met
                if first_func_act_point == 0 && *displacement > 0 {
                    // First the exception case where actuation is set to 0 as
                    // any displacement will trigger it.
                    self.key_actions[key].actuated.0 = true;
                    // Keydown for top-top case
                    log::trace!("A1 Key {} Keydown @ {}!", key, *displacement);
                    events.push(Event {
                        key: ANALOG_KEY_CODES[key].0.clone(),
                        pressed: true,
                    });
                } else if first_func_act_point <= *displacement {
                    // Finally general case where key is at or exceeds actuation point.
                    self.key_actions[key].actuated.0 = true;
                    // Keydown for general case
                    log::trace!("A1 Key {} Keydown @ {}!", key, *displacement);
                    events.push(Event {
                        key: ANALOG_KEY_CODES[key].0.clone(),
                        pressed: true,
                    });
                }
                // Stop processing as we have emitted a keystroke event
                continue;
            }

            // If already actuated then get the direction of travel and check for matching
            // event conditions based on downward or upward traversal.
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
                        // Keyup for first actuation
                        log::trace!("A1 Key {} Keyup @ {}!", key, *displacement);
                        self.key_actions[key].actuated.0 = false;
                        events.push(Event {
                            key: ANALOG_KEY_CODES[key].0.clone(),
                            pressed: false,
                        });
                    }
                    // Keydown for second actuation
                    log::trace!("A2 Key {} Keydown @ {}!", key, *displacement);
                    self.key_actions[key].actuated.1 = true;
                    events.push(Event {
                        key: ANALOG_KEY_CODES[key].1.clone(),
                        pressed: true,
                    });
                }

                // Manage retrigger events
                // Check if we should be evaluating a keydown retrigger
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
                        // Keydown for retrigger using separate reset and trigger values
                        log::trace!("Key {} separate value retrigger!", key);
                        events.push(Event {
                            key: ANALOG_KEY_CODES[key].0.clone(),
                            pressed: true,
                        });
                    } else if retrigger_downstroke_value == 0.0
                        && self.key_actions[key].retrigger_track >= retrigger_shared_value
                    {
                        // Otherwise check again using the shared point for keydown
                        self.key_actions[key].retrigger_go = false;
                        self.key_actions[key].retrigger_track = 0.0;
                        // Keydown for shared retrigger threshold
                        log::trace!("Key {} shared value retrigger!", key);
                        events.push(Event {
                            key: ANALOG_KEY_CODES[key].0.clone(),
                            pressed: true,
                        });
                    }
                } else if self.key_actions[key].retrigger_track != 0.0 {
                    // If not evaluating then calculate and apply retrigger 'resistance'.
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
                    // Cover off reset states: the top of travel and actuation point respectively.
                    if *displacement == 0 || !continuous_retrigger {
                        // Case for where actuation point is at or near the top
                        if first_func_actd && *displacement == 0 {
                            log::trace!("Key {} reached top! {:?}", key, run);
                            // Keyup for first function
                            events.push(Event {
                                key: ANALOG_KEY_CODES[key].0.clone(),
                                pressed: false,
                            });
                        } else if second_func_reset_rule {
                            // When the second function resets the first function must be prevented
                            // from immediately triggering as it will be within its actuation
                            // window. Set a flag to back-off actuation until its initial actuation
                            // (reset) point has been reached.
                            log::trace!("A2 Key {} Keyup @ {}!", key, *displacement);
                            events.push(Event {
                                key: ANALOG_KEY_CODES[key].1.clone(),
                                pressed: false,
                            });
                            if *displacement > 0 {
                                self.key_actions[key].backoff_first_func = true;
                            }
                        } else {
                            // Otherwise regular / first function reset
                            // Keyup for first function
                            log::trace!("A1 Key {} Keyup @ {}!", key, *displacement);
                            events.push(Event {
                                key: ANALOG_KEY_CODES[key].0.clone(),
                                pressed: false,
                            });
                        }

                        // Reset key state
                        self.key_actions[key].retrigger_go = false;
                        self.key_actions[key].retrigger_track = 0.0;
                        self.key_actions[key].actuated = (false, false);
                        continue;
                    }
                }
                // Reset actuation back-off flag when the first function reset point is reached
                if backoff_first_func && first_func_act_point >= *displacement {
                    self.key_actions[key].backoff_first_func = false;
                }
                // Manage retrigger for upward keystrokes
                // If the stroke direction has changed then our initial displacement is calculated
                // from the start of the regression window. If continuing in the same direction
                // then the displacement accumulates from the previous reading.
                // The displacement is compared against the retrigger threshold, in the case of
                // an upwards stroke reaching the threshold sets the retrigger flag allowing the
                // downward stroke to issue a retrigger. The user profile can define separate
                // thresholds for upwards and downwards displacement. Retrigger is disabled when
                // dual-function is defined for a key which matches OEM software behavior.
                if !self.key_actions[key].retrigger_go
                    && retrigger_shared_value > 0.0
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
                        log::trace!("Key {} retrigger reached!", key);
                        events.push(Event {
                            key: ANALOG_KEY_CODES[key].0.clone(),
                            pressed: false,
                        });
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
