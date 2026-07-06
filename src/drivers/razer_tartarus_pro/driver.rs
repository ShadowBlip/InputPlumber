use super::event::{Event, KeyCodes, ANALOG_KEY_CODES};
use crate::config::SourceDevice;
use crate::udev::device::UdevDevice;
use hidapi::HidDevice;
use std::collections::VecDeque;
use std::{error::Error, ffi::CString};

pub const VID: u16 = 0x1532;
pub const PID: u16 = 0x0244;

const ENDPOINT_1: i32 = 0;
const ENDPOINT_2: i32 = 1;
const ENDPOINT_3: i32 = 2;
const REPORT_1: u8 = 0x01;
const REPORT_6: u8 = 0x06;
const SHORT_DATA_PAYLOAD: usize = 8;
const LONG_DATA_PAYLOAD: usize = 24;
const TREND_KEYDOWN: f64 = 1.0;
const TREND_KEYUP: f64 = -1.0;
const REGRESSION_WINDOW: usize = 5;
const KEY_COUNT: usize = 20;
const CONFIG_STEP_SIZE_MM: f64 = 0.1;
const CONFIG_STEP_SIZE_UNIT: u8 = 0xC;
const KEY_TOP_MM: f64 = 1.4;
const KEY_BOTTOM_MM: f64 = 3.6;
const KEY_TRAVEL: f64 = KEY_BOTTOM_MM - KEY_TOP_MM - CONFIG_STEP_SIZE_MM;
const UNIT_TO_MM: f64 = CONFIG_STEP_SIZE_MM / CONFIG_STEP_SIZE_UNIT as f64;

pub struct Driver {
    device: HidDevice,
    hysteresis: VecDeque<[u8; KEY_COUNT]>,
    key_actions: [AnalogAction; KEY_COUNT],
    key_state: Vec<KeyCodes>,
}

/// Structure for per-key threshold values from profile and state machine memory
#[derive(Clone, Copy, Default, Debug)]
pub struct AnalogAction {
    /// User setting (primary function, secondary function), must be > 0 to be active.
    actuation_point: (u8, u8),
    /// User setting (reset, trigger), > 0 activates. If trigger == 0 the reset value is used
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

/// Translates an absolute key actuation point in mm with sanitised precision to a u8 unit value
fn convert_actuation_to_unit(value: f64) -> u8 {
    if !(KEY_TOP_MM..=KEY_BOTTOM_MM).contains(&value) {
        log::error!(
            "Invalid actuation range f64 value found in config: {}, treating as 0",
            value
        );
        return 0;
    }

    let zeroed = value - KEY_TOP_MM;
    let remainder = zeroed.rem_euclid(CONFIG_STEP_SIZE_MM);
    let epsilon = 1e-10;

    if remainder < epsilon || (CONFIG_STEP_SIZE_MM - remainder).abs() < epsilon {
        return (zeroed / UNIT_TO_MM) as u8;
    }
    log::error!(
        "Invalid actuation precision f64 value found in config: {}, treating as 0",
        value
    );
    0
}

/// Sanitise a relative displacement value to ensure it is in the correct range and precision
fn validate_retrigger_value(value: f64) -> f64 {
    if !(0.0..=KEY_TRAVEL).contains(&value) {
        log::error!(
            "Invalid retrigger range f64 value found in config: {}, treating as 0",
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
        "Invalid retrigger precision f64 value found in config: {}, treating as 0",
        value
    );
    0.0
}

/// This driver implementation is shared across all three HID handles on the Tartarus Pro.
/// Different interfaces traverse different code-paths. Refer to handle_input_report()
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

        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{hidrawpath}' is not a Razer Tartarus Pro").into());
        }

        // Create a 20x5 null matrix to initialize the buffer that implements key hysteresis
        let mut zeroes = VecDeque::with_capacity(REGRESSION_WINDOW);
        zeroes.extend([[0; KEY_COUNT]; REGRESSION_WINDOW]);

        let mut tartarus = Self {
            device,
            hysteresis: zeroes,
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
                        s.crt_en = new_val
                            .as_ref()
                            .is_some_and(|s| matches!(s.as_str(), "Y" | "y"));
                    }
                }
            }
        }
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0; LONG_DATA_PAYLOAD];
        let bytes_read = self.device.read_timeout(&mut buf[..], 0x2)?;
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
        // This function is common to all Tartarus Pro endpoints & device handles. Each device
        // handle manages a single endpoint and each endpoint generates different types of reports.
        // - Endpoint 1 describes the D-pad and aux keys per the HID report descriptor.
        // - Endpoint 2 describes the 20 numbered keys and uses two reports. Report ID 1 refers to
        //   normal keyboard functions and is fully described by the HID report descriptor. Report
        //   ID 6 is vendor defined and reflects analog mode which is handled separately. Other
        //   reports are defined in the descriptor table but are seemingly unused.
        // - Endpoint 3 describes the scroll wheel via the HID report descriptor. The extra feature
        //   report page used for control is unused by InputPlumber.
        match self.device.get_device_info()?.interface_number() {
            ENDPOINT_1 => {
                if bytes_read == SHORT_DATA_PAYLOAD {
                    return self.handle_basic(buf, KeyCodes::PhantomAux, false);
                }
                Ok(Vec::new())
            }
            ENDPOINT_2 => {
                if bytes_read == LONG_DATA_PAYLOAD {
                    return match buf[0] {
                        // Only manage report IDs 1 & 6. Other report types are defined in firmware
                        // but don't appear to be used.
                        REPORT_1 => self.handle_basic(buf, KeyCodes::PhantomBlank, true),
                        REPORT_6 => self.handle_analog(&buf[1..21]),
                        _ => Ok(Vec::new()),
                    };
                }
                Ok(Vec::new())
            }
            ENDPOINT_3 => {
                if bytes_read == SHORT_DATA_PAYLOAD {
                    return self.handle_basic(buf, KeyCodes::PhantomMClick, false);
                }
                Ok(Vec::new())
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Manage non-analog key input reports and translate to keystroke events
    fn handle_basic(
        &mut self,
        buf: &[u8],
        key_replace: KeyCodes,
        overwrite: bool,
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // The first byte of a report is interpreted based on the originating endpoint.
        // For endpoints 1 and 3 this byte is the overloaded scan code 0x04 which is substituted
        // for a phantom key and translated to a real key later. For endpoint 2 this byte is the
        // report ID which is zeroed then dropped during processing.
        let mut events = Vec::new();
        let mut pad_state: Vec<KeyCodes> = buf.iter().map(|&s| KeyCodes::from(s)).collect();
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
            if !pad_state.contains(i) {
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
    /// A key in analog mode returns an 8-bit value representing height displacement. 0 means
    /// reset / top of travel (~1.5mm) and 255 when it bottoms out (~3.6mm).
    /// The OEM software defines a minimum discernable step size of 0.1mm which we will adopt.
    /// We define a 'unit' which is total key travel (2.1mm) divided by quantization level (255)
    /// noting that '0' is a parked value as we measure changes from 0. With quantization error we
    /// declare that 0.1mm is equivalent to 12 (0xC) units.
    ///
    /// For a given keystroke it is not expected to see all values between 0 to 255. 0 is
    /// guaranteed, as is 255 if you bottom out. All other values are based on poll rate and
    /// incidental key location at time of sampling. The other guarantee is trend - if you press
    /// down the value goes up and vice-versa. During development the shortest run of values
    /// measured on the downstroke (i.e. slamming the key down) was 5. Uncontrolled release
    /// generates 5 to 6 values. If you carefully manipulate the key you are able to make changes
    /// down to the unit value (0.00833 mm) but this is generally a slow motion and not reflective
    /// of all usage patterns. To account for the variable displacement speed and resultant
    /// discontinuous values when measuring keystrokes, linear regression is used to establish the
    /// direction of travel and forms the basis of mapping displacement to actions. Per the above
    /// measurements, a 5 sample deep buffer is used for this calculation. Picking a trend
    /// criteria of ± 1 is important as this filters out key jitter (from say holding position)
    /// giving a clear indication as to whether a key is moving up or down.
    ///
    /// As the true values of the actuation range are somewhat ambiguous, for the purposes of
    /// managing configuration we have declared 1.4mm as the 'top-top' point which translates to
    /// unit value 0 allowing 3.6mm to translate (clamp) to 255. This does mean that 3.5mm is
    /// mapped to 252 instead of 243 when using 12 unit spacing. This may change in future.
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
    /// A keystroke is defined as a top start then a downwards movement, holding about a point
    /// until the desired effect is seen by the user. There may be subsequent move and holds but
    /// the key will eventually return to the top. To map an event we need to know from the user
    /// what displacement value should be used as an event trigger, everything else from that
    /// point is filtering as analog-optical keyboards sample continuously during travel.
    ///
    /// The summary of report processing is
    /// - Push the current report into the buffer maintaining its depth of 5.
    /// - Perform an action for each key in the current report:
    ///   * Where not marked as actuated then evaluate if it can, do so, or ignore for the round
    ///   * If already actuated get the direction of travel by slicing the buffer at the key's
    ///     offset then perform linear regression on the slice. The action to perform is then
    ///     determined:
    ///     + Where retrigger is enabled track that state using direction of travel
    ///     + Where dual-function is enabled manage the secondary actuation states
    ///     + Reset and special cases as applicable
    ///
    /// A key returning to the top (zeroing) resets any state associated with that key's travel.
    ///
    /// # Analog Methods:
    ///
    /// ## Variable Primary Actuation Point
    ///   A displacement value (f1) specified in the device YAML file is used to trigger an event.
    ///   The key is considered actuated when traversing downwards past this point, cleared only
    ///   when ascending past it. Keydown and keyup events share the same value based on direction.
    ///
    ///   A key may also have one of the following methods applied to it:
    /// ## Dual-Function
    ///   Allocate a second actuation point (f2) that binds a separate function.
    ///   Triggered during downstroke, the sequence is f1 keydown then f1 keyup / f2 keydown.
    ///   The f2 keyup is triggered during upstroke at the f2 actuation point. Functions do not
    ///   trigger again until the key is above their respective actuation points i.e. f1 must be
    ///   above its device file actuation point to retrigger, likewise with f2.
    ///
    /// ## Retrigger
    ///   Keys with a single actuation point can have their reset rule redefined to occur after
    ///   an amount of negative displacement instead of using a fixed point. When the threshold is
    ///   reached a user defined amount of positive displacement will retrigger the key. This will
    ///   continue until the fixed reset point is reached, removing the key's actuation status.
    ///   Users can enable 'continuous retrigger' which changes this limit to be the top of the key
    ///   in effect making the actuation point an entry gate to the sequence. The reset threshold
    ///   can be a shared with the retrigger or defined independently.
    ///
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
            let retrigger_reset_value = self.key_actions[key].retrigger_threshold.0;
            let retrigger_trigger_value = self.key_actions[key].retrigger_threshold.1;
            let continuous_retrigger = self.key_actions[key].crt_en;
            let backoff_first_func = self.key_actions[key].backoff_first_func;
            let key_is_upstroke = self.key_actions[key].key_is_upstroke;
            let retrigger_track = self.key_actions[key].retrigger_track;

            // State machine traversal logic
            let func_actd = first_func_actd ^ second_func_actd;
            let first_func_reset_rule = first_func_actd && (first_func_act_point >= *displacement);
            let second_func_user_valid = second_func_act_point > first_func_act_point;
            let second_func_reset_rule = (second_func_act_point >= *displacement)
                && second_func_user_valid
                && second_func_actd;

            // When not actuated check if key displacement met or exceeded the set point. If so
            // issue the appropriate keydown event and mark as actuated, otherwise ignore the key.
            if !func_actd && !backoff_first_func {
                if *displacement != 0 && first_func_act_point <= *displacement {
                    self.key_actions[key].actuated.0 = true;
                    log::trace!("A1 Key {} Keydown @ {}!", key, *displacement);
                    events.push(Event {
                        key: ANALOG_KEY_CODES[key].0.clone(),
                        pressed: true,
                    });
                }
                // End the round regardless of result as we were not actuated upon entry.
                continue;
            }

            // When already actuated get a mutable copy of retrigger_track to modify for the round.
            // Obtain the direction of key travel then check for matching event conditions to
            // action for the round.
            let mut next_retrigger_track = self.key_actions[key].retrigger_track;
            let run: Vec<f64> = self.hysteresis.iter().map(|arr| arr[key] as f64).collect();
            let trend = self.linear_regression(&run).unwrap_or(0.0);

            // Manage an actuated key moving downwards
            if trend >= TREND_KEYDOWN {
                self.key_actions[key].key_is_upstroke = false;
                // Check if we need to perform dual function
                if second_func_user_valid
                    && !second_func_actd
                    && second_func_act_point <= *displacement
                {
                    // Keyup for first actuation
                    log::trace!("A1 Key {} Keyup @ {}!", key, *displacement);
                    // Set key as no longer actuated to support the back-off logic
                    self.key_actions[key].actuated.0 = false;
                    events.push(Event {
                        key: ANALOG_KEY_CODES[key].0.clone(),
                        pressed: false,
                    });

                    // Keydown for second actuation
                    log::trace!("A2 Key {} Keydown @ {}!", key, *displacement);
                    self.key_actions[key].actuated.1 = true;
                    events.push(Event {
                        key: ANALOG_KEY_CODES[key].1.clone(),
                        pressed: true,
                    });
                    // Dual function and retrigger cannot be used simultaneously.
                    continue;
                }

                // Check if we should be evaluating a keydown retrigger
                if self.key_actions[key].retrigger_go {
                    // Calculate displacement track value based on whether we've changed direction
                    let difference = *displacement as f64 - previous_state[key] as f64;
                    next_retrigger_track = if retrigger_track == 0.0 {
                        (*displacement as f64 - front_state[key] as f64) * UNIT_TO_MM
                    } else {
                        next_retrigger_track + difference * UNIT_TO_MM
                    };

                    // Check retrigger condition for both shared and separate thresholds
                    if (retrigger_trigger_value != 0.0
                        && next_retrigger_track >= retrigger_trigger_value)
                        || (retrigger_trigger_value == 0.0
                            && next_retrigger_track >= retrigger_reset_value)
                    {
                        self.key_actions[key].retrigger_go = false;
                        next_retrigger_track = 0.0;
                        events.push(Event {
                            key: ANALOG_KEY_CODES[key].0.clone(),
                            pressed: true,
                        });
                        // Log which scenario triggered
                        if retrigger_trigger_value != 0.0 {
                            log::trace!("Key {} separate value retrigger!", key);
                        } else {
                            log::trace!("Key {} shared value retrigger!", key);
                        }
                    }

                    self.key_actions[key].retrigger_track = next_retrigger_track;
                    continue;
                }
                // If a threshold isn't met check if retrigger progress should be unwound.
                // retrigger_track uses negative (key moving up) and positive (key moving down)
                // values, mapping to reset and retrigger respectively. Progress is unwound during
                // upward motion, it can be zeroed but not cancelled. Progress does not go negative
                // so that retrigger difficulty does not increase when resuming downwards motion.
                if retrigger_track != 0.0 {
                    let difference = if key_is_upstroke {
                        *displacement as f64 - front_state[key] as f64
                    } else {
                        *displacement as f64 - previous_state[key] as f64
                    };

                    next_retrigger_track += difference * UNIT_TO_MM;
                    // If retrigger progress is rewound, zero it rather than increasing difficulty.
                    if next_retrigger_track > 0.0 {
                        next_retrigger_track = 0.0
                    };

                    self.key_actions[key].retrigger_track = next_retrigger_track;
                }
                continue;
            }

            // Manage an actuated key moving upwards
            if trend <= TREND_KEYUP {
                self.key_actions[key].key_is_upstroke = true;
                // Manage the reset of actuated keys
                if (first_func_reset_rule || second_func_reset_rule)
                    && (*displacement == 0 || !continuous_retrigger)
                {
                    // Reset key state
                    self.key_actions[key].retrigger_go = false;
                    self.key_actions[key].retrigger_track = 0.0;
                    self.key_actions[key].actuated = (false, false);

                    // Case for actuation point at or near the top. Keyup for first function
                    if first_func_actd && *displacement == 0 {
                        log::trace!("Key {} reached top! {:?}", key, run);
                        events.push(Event {
                            key: ANALOG_KEY_CODES[key].0.clone(),
                            pressed: false,
                        });
                        continue;
                    }

                    // When the second function resets the first function must be prevented from
                    // immediately triggering as it will be within its actuation window. Set a flag
                    // to back-off actuation until its initial actuation point has been reached.
                    if second_func_reset_rule {
                        if *displacement > 0 {
                            self.key_actions[key].backoff_first_func = true;
                        }

                        log::trace!("A2 Key {} Keyup @ {}!", key, *displacement);
                        events.push(Event {
                            key: ANALOG_KEY_CODES[key].1.clone(),
                            pressed: false,
                        });
                        continue;
                    }
                    // Otherwise regular / first function reset. Keyup for first function
                    log::trace!("A1 Key {} Keyup @ {}!", key, *displacement);
                    events.push(Event {
                        key: ANALOG_KEY_CODES[key].0.clone(),
                        pressed: false,
                    });
                    continue;
                }

                // Reset actuation back-off flag when the first function reset point is reached
                // Note that at this point the first function is *not* actuated
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
                    && retrigger_reset_value > 0.0
                    && !second_func_user_valid
                {
                    // Using the key_is_upstroke flag we can determine whether to look at only the
                    // previous value or look at the start of the buffer.
                    let difference = if key_is_upstroke {
                        previous_state[key] as f64 - *displacement as f64
                    } else {
                        front_state[key] as f64 - *displacement as f64
                    };

                    next_retrigger_track -= difference * UNIT_TO_MM;
                    if retrigger_reset_value <= next_retrigger_track.abs() {
                        log::trace!("Key {} retrigger reached!", key);
                        events.push(Event {
                            key: ANALOG_KEY_CODES[key].0.clone(),
                            pressed: false,
                        });
                        next_retrigger_track = 0.0;
                        self.key_actions[key].retrigger_go = true;
                    }
                    self.key_actions[key].retrigger_track = next_retrigger_track;
                }
                continue;
            }

            // Fall through case for minimum threshold with short trends to prevent repeating keys
            if first_func_act_point == 0 && *displacement == 0 {
                log::trace!("A1 Key {} Keyup minimum case @ {}!", key, *displacement);
                self.key_actions[key].actuated.0 = false;
                events.push(Event {
                    key: ANALOG_KEY_CODES[key].0.clone(),
                    pressed: false,
                });
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
