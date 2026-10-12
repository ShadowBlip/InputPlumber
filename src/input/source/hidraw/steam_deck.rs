use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Debug,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use evdev::{FFEffectData, FFEffectKind, InputEvent};

use crate::{
    drivers::{
        dualsense::hid_report::SetStatePackedOutputData,
        steam_deck::{
            self,
            driver::Driver,
            event,
            hid_report::{PackedTriggerVibration, LIZARD_SLEEP_SEC},
            DECK_ACCEL_RAW_TO_MPS2, DECK_GYRO_RAW_TO_RAD_S,
        },
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Source, Touch,
            TouchButton, Touchpad,
        },
        event::{
            native::NativeEvent,
            value::{
                normalize_accel_value_i16, normalize_gyro_value_i16, normalize_signed_value,
                normalize_unsigned_value, InputValue,
            },
        },
        output_capability::{Haptic, OutputCapability},
        output_event::OutputEvent,
        source::{EventCapability, InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

pub struct DeckController {
    driver: Driver,
    device_info: UdevDevice,
    lizard_mode_started: bool,
    lizard_mode_running: Arc<Mutex<bool>>,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
}

impl DeckController {
    /// Create a new Deck Controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info.devnode())?;

        Ok(Self {
            driver,
            device_info,
            lizard_mode_started: false,
            lizard_mode_running: Arc::new(Mutex::new(false)),
            ff_evdev_effects: HashMap::new(),
        })
    }

    /// Start lizard mode task to keep lizard mode asleep.
    fn start_lizard_task(&mut self) {
        let path = self.device_info.devnode();
        let lizard_mode_running = self.lizard_mode_running.clone();
        tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
            let driver = Driver::new(path)?;
            *lizard_mode_running.lock().unwrap() = true;
            loop {
                // Check if lizard mode needs to stop
                if !*lizard_mode_running.lock().unwrap() {
                    break;
                }

                // Keep the lizard asleep
                driver.handle_lizard_mode()?;

                // Polling interval is about 4ms so we can sleep a little
                let duration = Duration::from_secs(LIZARD_SLEEP_SEC as u64);
                thread::sleep(duration);
            }

            Ok(())
        });
        self.lizard_mode_started = true;
    }

    /// Returns the next available evdev effect id
    fn next_ff_effect_id(&self) -> i16 {
        const MAX: i16 = 2096;
        let mut i = 0;
        loop {
            if !self.ff_evdev_effects.contains_key(&i) {
                return i;
            }
            i += 1;
            if i > MAX {
                return -1;
            }
        }
    }

    /// Write a rumble event to the driver
    fn write_rumble(
        driver: &mut Driver,
        left: u16,
        right: u16,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let event =
            event::OutputEvent::HapticRumble(PackedTriggerVibration::with_magnitudes(left, right));
        driver.write_event(&event)
    }

    /// Process the given evdev force feedback event.
    fn process_evdev_ff(&mut self, input_event: InputEvent) -> Result<(), Box<dyn Error>> {
        // Get the code (effect id) and value of the event
        let (code, value) =
            if let evdev::EventSummary::ForceFeedback(_, code, value) = input_event.destructure() {
                (code, value)
            } else {
                log::debug!("Unhandled evdev output event: {:?}", input_event);
                return Ok(());
            };

        // Find the effect data for this event
        let effect_id = code.0 as i16;
        let Some(effect_data) = self.ff_evdev_effects.get(&effect_id) else {
            log::warn!("No effect id found: {}", code.0);
            return Ok(());
        };

        // The value determines if the effect should be playing or not.
        if value == 0 {
            if let Err(e) = Self::write_rumble(&mut self.driver, 0, 0) {
                log::debug!("Failed to stop haptic rumble: {:?}", e);
                return Ok(());
            }
        }

        // Perform the rumble based on the effect
        // TODO: handle effect duration, etc.
        match effect_data.kind {
            FFEffectKind::Damper => (),
            FFEffectKind::Inertia => (),
            FFEffectKind::Constant {
                level: _,
                envelope: _,
            } => (),
            FFEffectKind::Ramp {
                start_level: _,
                end_level: _,
                envelope: _,
            } => (),
            FFEffectKind::Periodic {
                waveform: _,
                period: _,
                magnitude: _,
                offset: _,
                phase: _,
                envelope: _,
            } => (),
            FFEffectKind::Spring { condition: _ } => (),
            FFEffectKind::Friction { condition: _ } => (),
            FFEffectKind::Rumble {
                strong_magnitude,
                weak_magnitude,
            } => {
                // Do rumble
                if let Err(e) =
                    Self::write_rumble(&mut self.driver, strong_magnitude, weak_magnitude)
                {
                    let err = format!("Failed to do haptic rumble: {:?}", e);
                    return Err(err.into());
                }
            }
        }

        Ok(())
    }

    /// Process dualsense force feedback output reports
    fn process_dualsense_ff(
        &mut self,
        report: SetStatePackedOutputData,
    ) -> Result<(), Box<dyn Error>> {
        // Scale the per-motor speeds up to the deck's 16-bit range
        let left_speed = report.rumble_emulation_left as u16 * 256;
        let right_speed = report.rumble_emulation_right as u16 * 256;

        if let Err(e) = Self::write_rumble(&mut self.driver, left_speed, right_speed) {
            let err = format!("Failed to do haptic rumble: {:?}", e);
            return Err(err.into());
        }

        Ok(())
    }
}

impl SourceInputDevice for DeckController {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        // Spawn a blocking task to handle lizard mode
        if !self.lizard_mode_started {
            self.start_lizard_task();
        }

        let events = self.driver.poll()?;
        let native_events = translate_events(events);
        Ok(native_events)
    }

    /// Returns the possible input events this device is capable of emitting
    fn update_event_filter(&mut self, events: HashSet<Capability>) -> Result<(), InputError> {
        let filtered = events
            .iter()
            .filter_map(EventCapability::from_capability)
            .collect();
        self.driver.update_filtered_events(filtered);
        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for DeckController {
    /// Returns the possible output events this device is capable of (e.g. force feedback, LED,
    /// etc.)
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(OUTPUT_CAPABILITIES.into())
    }

    /// Write the given output event to the source device. Output events are
    /// events that flow from an application (like a game) to the physical
    /// input device, such as force feedback events.
    fn write_event(&mut self, event: OutputEvent) -> Result<(), OutputError> {
        log::trace!("Received output event: {:?}", event);
        match event {
            OutputEvent::Evdev(input_event) => {
                self.process_evdev_ff(input_event)?;
            }
            OutputEvent::DualSense(report) => {
                log::debug!("Received DualSense output report");
                if report.use_rumble_not_haptics || report.enable_improved_rumble_emulation {
                    self.process_dualsense_ff(report)?;
                }
            }
            OutputEvent::Uinput(_) => (),
            OutputEvent::TriggerHapticPulse(report) => {
                self.driver
                    .write_event(&event::OutputEvent::HapticClick(report))?;
            }
            OutputEvent::TriggerRepeatedHapticPulse(report) => {
                self.driver
                    .write_event(&event::OutputEvent::HapticPulse(report))?;
            }
            OutputEvent::TriggerVibration(report) => {
                self.driver
                    .write_event(&event::OutputEvent::HapticRumble(report))?;
            }
            OutputEvent::TriggerVibrationExtended(report) => {
                self.driver
                    .write_event(&event::OutputEvent::TriggerVibrationExtended(report))?;
            }
            OutputEvent::GenericRumble {
                weak_magnitude,
                strong_magnitude,
            } => {
                Self::write_rumble(&mut self.driver, strong_magnitude, weak_magnitude)?;
            }
        }

        Ok(())
    }

    /// Upload the given force feedback effect data to the source device. Returns
    /// a device-specific id of the uploaded effect if it is successful.
    fn upload_effect(&mut self, effect: evdev::FFEffectData) -> Result<i16, OutputError> {
        log::debug!("Uploading FF effect data");
        let id = self.next_ff_effect_id();
        if id == -1 {
            return Err("Maximum FF effects uploaded".into());
        }
        self.ff_evdev_effects.insert(id, effect);

        Ok(id)
    }

    /// Update the effect with the given id using the given effect data.
    fn update_effect(
        &mut self,
        effect_id: i16,
        effect: evdev::FFEffectData,
    ) -> Result<(), OutputError> {
        log::debug!("Updating FF effect data with id {effect_id}");
        self.ff_evdev_effects.insert(effect_id, effect);
        Ok(())
    }

    /// Erase the effect with the given id from the source device.
    fn erase_effect(&mut self, effect_id: i16) -> Result<(), OutputError> {
        log::debug!("Erasing FF effect data");
        self.ff_evdev_effects.remove(&effect_id);
        Ok(())
    }

    /// Stop the source device and terminate the lizard mode task
    fn stop(&mut self) -> Result<(), OutputError> {
        *self.lizard_mode_running.lock().unwrap() = false;
        Ok(())
    }
}

impl Debug for DeckController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeckController")
            .field("device_info", &self.device_info)
            .field("lizard_mode_started", &self.lizard_mode_started)
            .field("lizard_mode_running", &self.lizard_mode_running)
            .field("ff_evdev_effects", &self.ff_evdev_effects)
            .finish()
    }
}

/// Normalize the value to something between -1.0 and 1.0 based on the Deck's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: &steam_deck::event::AxisEvent) -> InputValue {
    match event {
        steam_deck::event::AxisEvent::LPad(value) => {
            let min = steam_deck::hid_report::PAD_X_MIN;
            let max = steam_deck::hid_report::PAD_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = (x + 1.0) / 2.0; // Convert from -1.0 - 1.0 range to 0.0 - 1.0 range
            let x = Some(x);

            let min = steam_deck::hid_report::PAD_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::PAD_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = -y; // Y-axis is inverted
            let y = (y + 1.0) / 2.0; // Convert from -1.0 - 1.0 range to 0.0 - 1.0 range
            let y = Some(y);

            InputValue::Touch {
                index: value.index,
                is_touching: value.is_touching,
                pressure: Some(1.0),
                x,
                y,
            }
        }
        steam_deck::event::AxisEvent::RPad(value) => {
            let min = steam_deck::hid_report::PAD_X_MIN;
            let max = steam_deck::hid_report::PAD_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = (x + 1.0) / 2.0; // Convert from -1.0 - 1.0 range to 0.0 - 1.0 range
            let x = Some(x);

            let min = steam_deck::hid_report::PAD_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::PAD_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = -y; // Y-axis is inverted
            let y = (y + 1.0) / 2.0; // Convert from -1.0 - 1.0 range to 0.0 - 1.0 range
            let y = Some(y);

            InputValue::Touch {
                index: value.index,
                is_touching: value.is_touching,
                pressure: Some(1.0),
                x,
                y,
            }
        }
        steam_deck::event::AxisEvent::LStick(value) => {
            let min = steam_deck::hid_report::STICK_X_MIN;
            let max = steam_deck::hid_report::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::STICK_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
        steam_deck::event::AxisEvent::RStick(value) => {
            let min = steam_deck::hid_report::STICK_X_MIN;
            let max = steam_deck::hid_report::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::STICK_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::STICK_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the Deck's
/// maximum axis ranges.
fn normalize_trigger_value(event: &steam_deck::event::TriggerEvent) -> InputValue {
    match event {
        steam_deck::event::TriggerEvent::LTrigger(value) => {
            let max = steam_deck::hid_report::TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::RTrigger(value) => {
            let max = steam_deck::hid_report::TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::LPadForce(value) => {
            let max = steam_deck::hid_report::PAD_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::RPadForce(value) => {
            let max = steam_deck::hid_report::PAD_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::LStickForce(value) => {
            let max = steam_deck::hid_report::STICK_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        steam_deck::event::TriggerEvent::RStickForce(value) => {
            let max = steam_deck::hid_report::STICK_FORCE_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// Translate the given Steam Deck events into native events
fn translate_events(events: Vec<steam_deck::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().filter_map(translate_event).collect()
}

/// Translate the given Steam Deck event into a native event
fn translate_event(event: steam_deck::event::Event) -> Option<NativeEvent> {
    let value = event.to_value()?;
    Some(NativeEvent::new(event.to_capability(), value))
}

impl EventCapability for steam_deck::event::Event {
    fn to_value(&self) -> Option<InputValue> {
        match self {
            event::Event::Button(button) => Some(InputValue::Bool(button.pressed())),
            event::Event::Inertia(inertia) => match inertia {
                event::InertialEvent::Accelerometer(value) => Some(InputValue::Vector3 {
                    x: Some(normalize_accel_value_i16(value.x, DECK_ACCEL_RAW_TO_MPS2)),
                    y: Some(normalize_accel_value_i16(value.y, DECK_ACCEL_RAW_TO_MPS2)),
                    z: Some(normalize_accel_value_i16(value.z, DECK_ACCEL_RAW_TO_MPS2)),
                }),
                event::InertialEvent::Gyroscope(value) => Some(InputValue::Vector3 {
                    x: Some(normalize_gyro_value_i16(value.x, DECK_GYRO_RAW_TO_RAD_S)),
                    y: Some(normalize_gyro_value_i16(value.y, DECK_GYRO_RAW_TO_RAD_S)),
                    z: Some(normalize_gyro_value_i16(value.z, DECK_GYRO_RAW_TO_RAD_S)),
                }),
            },
            event::Event::Axis(axis) => Some(normalize_axis_value(axis)),
            event::Event::Trigger(trigger) => match trigger {
                event::TriggerEvent::LTrigger(_) | event::TriggerEvent::RTrigger(_) => {
                    Some(normalize_trigger_value(trigger))
                }
                event::TriggerEvent::LPadForce(_)
                | event::TriggerEvent::RPadForce(_)
                | event::TriggerEvent::LStickForce(_)
                | event::TriggerEvent::RStickForce(_) => None,
            },
        }
    }

    fn to_capability(&self) -> Capability {
        match self {
            steam_deck::event::Event::Button(button) => match button {
                steam_deck::event::ButtonEvent::A(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::South))
                }
                steam_deck::event::ButtonEvent::X(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::North))
                }
                steam_deck::event::ButtonEvent::B(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::East))
                }
                steam_deck::event::ButtonEvent::Y(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::West))
                }
                steam_deck::event::ButtonEvent::Menu(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Start))
                }
                steam_deck::event::ButtonEvent::Options(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Select))
                }
                steam_deck::event::ButtonEvent::Steam(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide))
                }
                steam_deck::event::ButtonEvent::QuickAccess(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess))
                }
                steam_deck::event::ButtonEvent::DPadDown(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown))
                }
                steam_deck::event::ButtonEvent::DPadUp(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp))
                }
                steam_deck::event::ButtonEvent::DPadLeft(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft))
                }
                steam_deck::event::ButtonEvent::DPadRight(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight))
                }
                steam_deck::event::ButtonEvent::L1(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper))
                }
                steam_deck::event::ButtonEvent::L2(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger))
                }
                steam_deck::event::ButtonEvent::L3(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick))
                }
                steam_deck::event::ButtonEvent::L4(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1))
                }
                steam_deck::event::ButtonEvent::L5(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2))
                }
                steam_deck::event::ButtonEvent::R1(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper))
                }
                steam_deck::event::ButtonEvent::R2(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger))
                }
                steam_deck::event::ButtonEvent::R3(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick))
                }
                steam_deck::event::ButtonEvent::R4(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1))
                }
                steam_deck::event::ButtonEvent::R5(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2))
                }
                steam_deck::event::ButtonEvent::RPadTouch(_) => {
                    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Touch)))
                }
                steam_deck::event::ButtonEvent::LPadTouch(_) => {
                    Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Touch)))
                }
                steam_deck::event::ButtonEvent::RPadPress(_) => {
                    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press)))
                }
                steam_deck::event::ButtonEvent::LPadPress(_) => {
                    Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press)))
                }
                steam_deck::event::ButtonEvent::RStickTouch(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch))
                }
                steam_deck::event::ButtonEvent::LStickTouch(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch))
                }
            },
            steam_deck::event::Event::Inertia(inertia) => match inertia {
                steam_deck::event::InertialEvent::Accelerometer(_) => {
                    Capability::Accelerometer(Source::Center)
                }
                steam_deck::event::InertialEvent::Gyroscope(_) => {
                    Capability::Gyroscope(Source::Center)
                }
            },
            steam_deck::event::Event::Axis(axis) => match axis {
                steam_deck::event::AxisEvent::LPad(_) => {
                    Capability::Touchpad(Touchpad::LeftPad(Touch::Motion))
                }
                steam_deck::event::AxisEvent::RPad(_) => {
                    Capability::Touchpad(Touchpad::RightPad(Touch::Motion))
                }
                steam_deck::event::AxisEvent::LStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
                }
                steam_deck::event::AxisEvent::RStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
                }
            },
            steam_deck::event::Event::Trigger(trigg) => match trigg {
                steam_deck::event::TriggerEvent::LTrigger(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
                }
                steam_deck::event::TriggerEvent::RTrigger(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
                }
                _ => Capability::NotImplemented,
            },
        }
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        match capability {
            Capability::Gamepad(Gamepad::Button(button)) => {
                let event = match button {
                    GamepadButton::South => steam_deck::event::ButtonEvent::A(Default::default()),
                    GamepadButton::North => steam_deck::event::ButtonEvent::X(Default::default()),
                    GamepadButton::East => steam_deck::event::ButtonEvent::B(Default::default()),
                    GamepadButton::West => steam_deck::event::ButtonEvent::Y(Default::default()),
                    GamepadButton::Start => {
                        steam_deck::event::ButtonEvent::Menu(Default::default())
                    }
                    GamepadButton::Select => {
                        steam_deck::event::ButtonEvent::Options(Default::default())
                    }
                    GamepadButton::Guide => {
                        steam_deck::event::ButtonEvent::Steam(Default::default())
                    }
                    GamepadButton::QuickAccess => {
                        steam_deck::event::ButtonEvent::QuickAccess(Default::default())
                    }
                    GamepadButton::DPadDown => {
                        steam_deck::event::ButtonEvent::DPadDown(Default::default())
                    }
                    GamepadButton::DPadUp => {
                        steam_deck::event::ButtonEvent::DPadUp(Default::default())
                    }
                    GamepadButton::DPadLeft => {
                        steam_deck::event::ButtonEvent::DPadLeft(Default::default())
                    }
                    GamepadButton::DPadRight => {
                        steam_deck::event::ButtonEvent::DPadRight(Default::default())
                    }
                    GamepadButton::LeftBumper => {
                        steam_deck::event::ButtonEvent::L1(Default::default())
                    }
                    GamepadButton::LeftTrigger => {
                        steam_deck::event::ButtonEvent::L2(Default::default())
                    }
                    GamepadButton::LeftStick => {
                        steam_deck::event::ButtonEvent::L3(Default::default())
                    }
                    GamepadButton::LeftPaddle1 => {
                        steam_deck::event::ButtonEvent::L4(Default::default())
                    }
                    GamepadButton::LeftPaddle2 => {
                        steam_deck::event::ButtonEvent::L5(Default::default())
                    }
                    GamepadButton::RightBumper => {
                        steam_deck::event::ButtonEvent::R1(Default::default())
                    }
                    GamepadButton::RightTrigger => {
                        steam_deck::event::ButtonEvent::R2(Default::default())
                    }
                    GamepadButton::RightStick => {
                        steam_deck::event::ButtonEvent::R3(Default::default())
                    }
                    GamepadButton::RightPaddle1 => {
                        steam_deck::event::ButtonEvent::R4(Default::default())
                    }
                    GamepadButton::RightPaddle2 => {
                        steam_deck::event::ButtonEvent::R5(Default::default())
                    }
                    GamepadButton::RightStickTouch => {
                        steam_deck::event::ButtonEvent::RStickTouch(Default::default())
                    }
                    GamepadButton::LeftStickTouch => {
                        steam_deck::event::ButtonEvent::LStickTouch(Default::default())
                    }
                    _ => return None,
                };
                Some(steam_deck::event::Event::Button(event))
            }
            Capability::Gamepad(Gamepad::Axis(axis)) => match axis {
                GamepadAxis::LeftStick => Some(steam_deck::event::Event::Axis(
                    steam_deck::event::AxisEvent::LStick(Default::default()),
                )),
                GamepadAxis::RightStick => Some(steam_deck::event::Event::Axis(
                    steam_deck::event::AxisEvent::RStick(Default::default()),
                )),
                _ => None,
            },
            Capability::Gamepad(Gamepad::Trigger(trigger)) => match trigger {
                GamepadTrigger::LeftTrigger => Some(steam_deck::event::Event::Trigger(
                    steam_deck::event::TriggerEvent::LTrigger(Default::default()),
                )),
                GamepadTrigger::RightTrigger => Some(steam_deck::event::Event::Trigger(
                    steam_deck::event::TriggerEvent::RTrigger(Default::default()),
                )),
                _ => None,
            },
            Capability::Touchpad(Touchpad::LeftPad(Touch::Button(button))) => {
                let event = match button {
                    TouchButton::Press => {
                        steam_deck::event::ButtonEvent::LPadPress(Default::default())
                    }
                    TouchButton::Touch => {
                        steam_deck::event::ButtonEvent::LPadTouch(Default::default())
                    }
                };
                Some(steam_deck::event::Event::Button(event))
            }
            Capability::Touchpad(Touchpad::RightPad(Touch::Button(button))) => {
                let event = match button {
                    TouchButton::Press => {
                        steam_deck::event::ButtonEvent::RPadPress(Default::default())
                    }
                    TouchButton::Touch => {
                        steam_deck::event::ButtonEvent::RPadTouch(Default::default())
                    }
                };
                Some(steam_deck::event::Event::Button(event))
            }
            Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)) => {
                Some(steam_deck::event::Event::Axis(
                    steam_deck::event::AxisEvent::LPad(Default::default()),
                ))
            }
            Capability::Touchpad(Touchpad::RightPad(Touch::Motion)) => {
                Some(steam_deck::event::Event::Axis(
                    steam_deck::event::AxisEvent::RPad(Default::default()),
                ))
            }
            Capability::Accelerometer(Source::Center) => Some(steam_deck::event::Event::Inertia(
                steam_deck::event::InertialEvent::Accelerometer(Default::default()),
            )),
            Capability::Gyroscope(Source::Center) => Some(steam_deck::event::Event::Inertia(
                steam_deck::event::InertialEvent::Gyroscope(Default::default()),
            )),
            _ => None,
        }
    }
}

/// List of all capabilities that the Steam Deck driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Accelerometer(Source::Center),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
    Capability::Gyroscope(Source::Center),
    Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Touch))),
    Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Touch))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
];

pub const OUTPUT_CAPABILITIES: &[OutputCapability] = &[
    OutputCapability::ForceFeedback,
    OutputCapability::Haptics(Haptic::TrackpadLeft),
    OutputCapability::Haptics(Haptic::TrackpadRight),
];
