use std::{
    collections::HashMap,
    error::Error,
    fmt::Debug,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use evdev::{FFEffectData, FFEffectKind, InputEvent};
use packed_struct::PackedStruct;

use crate::{
    drivers::{
        dualsense::hid_report::SetStatePackedOutputData,
        steam_deck::{
            self,
            driver::{Driver, ACCEL_SCALE},
            hid_report::LIZARD_SLEEP_SEC,
        },
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Touch, TouchButton,
            Touchpad,
        },
        event::{
            native::NativeEvent,
            value::InputValue,
            value::{normalize_signed_value, normalize_unsigned_value},
        },
        output_capability::{Haptic, OutputCapability},
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

pub struct DeckController {
    driver: Driver,
    device_info: UdevDevice,
    lizard_mode_started: bool,
    lizard_mode_running: Arc<Mutex<bool>>,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
    left_click_until: Option<Instant>,
    right_click_until: Option<Instant>,
    rumble_last: (u16, u16),
    leftpad_dpad: DpadState,
    leftpad_pressed: bool,
    leftpad_last_xy: Option<(f64, f64)>,
}

const CLICK_MS: u64 = 25;
const CLICK_STRENGTH: u16 = u16::MAX;
const HAPTIC_MAX: u16 = 32767;

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

            left_click_until: None,
            right_click_until: None,
            rumble_last: (0, 0),

            leftpad_dpad: DpadState::default(),
            leftpad_pressed: false,
            leftpad_last_xy: None,
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
            if let Err(e) = self.driver.haptic_rumble(0, 0) {
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
                // Set rumble values based on the effect data
                let left_speed = strong_magnitude;
                let right_speed = weak_magnitude;

                // Do rumble
                if let Err(e) = self.driver.haptic_rumble(left_speed, right_speed) {
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
        // Set the rumble values based on the DualSense output report
        let left_speed = report.rumble_emulation_left as u16 * 256;
        let right_speed = report.rumble_emulation_right as u16 * 256;

        if let Err(e) = self.driver.haptic_rumble(left_speed, right_speed) {
            let err = format!("Failed to do haptic rumble: {:?}", e);
            return Err(err.into());
        }

        Ok(())
    }

    fn arm_left_click(&mut self) {
        self.left_click_until = Some(Instant::now() + Duration::from_millis(CLICK_MS));
    }
    fn arm_right_click(&mut self) {
        self.right_click_until = Some(Instant::now() + Duration::from_millis(CLICK_MS));
    }

    fn update_click_rumble(&mut self) {
        let now = Instant::now();
        let left = self.left_click_until.map_or(0, |t| if now < t { CLICK_STRENGTH } else { 0 });
        let right = self.right_click_until.map_or(0, |t| if now < t { CLICK_STRENGTH } else { 0 });

        if (left, right) != self.rumble_last {
            let left  = left.min(HAPTIC_MAX);
            let right = right.min(HAPTIC_MAX);
            let _ = self.driver.haptic_rumble(left, right);
            self.rumble_last = (left, right);
        }

        // clear expired timers (optional hygiene)
        if self.left_click_until.is_some_and(|t| now >= t) { self.left_click_until = None; }
        if self.right_click_until.is_some_and(|t| now >= t) { self.right_click_until = None; }
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

        // Track whether finger is currently on the left pad
        for e in &events {
            if let steam_deck::event::Event::Button(btn) = e {
                match btn {
                    steam_deck::event::ButtonEvent::LPadPress(_) => self.arm_left_click(),
                    steam_deck::event::ButtonEvent::RPadPress(_) => self.arm_right_click(),
                    steam_deck::event::ButtonEvent::DPadUp(v)
                    | steam_deck::event::ButtonEvent::DPadDown(v)
                    | steam_deck::event::ButtonEvent::DPadLeft(v)
                    | steam_deck::event::ButtonEvent::DPadRight(v) => {
                        log::info!("Real DPad event: {:?} pressed={}", btn, v.pressed);
                    },
                    _ => {}
                }
            }
        }

        let native_events = translate_events(
            events,
            &mut self.leftpad_dpad,
            &mut self.leftpad_pressed,
            &mut self.leftpad_last_xy,
        );
        self.update_click_rumble();

        Ok(native_events)
    }

    /// Returns the possible input events this device is capable of emitting
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
            OutputEvent::SteamDeckHaptics(packed_haptic_report) => {
                match packed_haptic_report.pack() {
                    Ok(report) => {
                        if let Err(e) = self.driver.write(&report) {
                            log::debug!("Ignoring invalid SteamDeckHaptics write: {:?}", e);
                        }
                    }
                    Err(e) => {
                        log::debug!("Ignoring invalid SteamDeckHaptics pack: {:?}", e);
                    }
                }
            }
            OutputEvent::SteamDeckRumble(packed_rumble_report) => {
                let report = packed_rumble_report.pack().map_err(|e| e.to_string())?;
                self.driver.write(&report)?;
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
        let _ = self.driver.haptic_rumble(0, 0);
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
fn normalize_axis_value(event: steam_deck::event::AxisEvent) -> InputValue {
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
fn normalize_trigger_value(event: steam_deck::event::TriggerEvent) -> InputValue {
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

/// Translate the given Steam Deck event into a native event
fn translate_event(event: steam_deck::event::Event) -> NativeEvent {
    match event {
        steam_deck::event::Event::Button(button) => match button {
            steam_deck::event::ButtonEvent::A(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::X(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::B(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Y(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Menu(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Options(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::Steam(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::QuickAccess(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadDown(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadUp(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadLeft(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::DPadRight(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::L5(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::R5(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::RPadTouch(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Touch))),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LPadTouch(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Touch))),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::RPadPress(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LPadPress(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press))),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::RStickTouch(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
                InputValue::Bool(value.pressed),
            ),
            steam_deck::event::ButtonEvent::LStickTouch(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
                InputValue::Bool(value.pressed),
            ),
        },
        steam_deck::event::Event::Accelerometer(accel) => match accel {
            steam_deck::event::AccelerometerEvent::Accelerometer(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Accelerometer),
                InputValue::Vector3 {
                    x: Some(value.x as f64 * ACCEL_SCALE),
                    y: Some(value.y as f64 * ACCEL_SCALE),
                    z: Some(value.z as f64 * ACCEL_SCALE),
                },
            ),
            steam_deck::event::AccelerometerEvent::Attitude(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Gyro),
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
        },
        steam_deck::event::Event::Axis(axis) => match axis.clone() {
            steam_deck::event::AxisEvent::LPad(_) => NativeEvent::new(
                Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
                normalize_axis_value(axis),
            ),
            steam_deck::event::AxisEvent::RPad(_) => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
                normalize_axis_value(axis),
            ),
            steam_deck::event::AxisEvent::LStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                normalize_axis_value(axis),
            ),
            steam_deck::event::AxisEvent::RStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                normalize_axis_value(axis),
            ),
        },
        steam_deck::event::Event::Trigger(trigg) => match trigg.clone() {
            steam_deck::event::TriggerEvent::LTrigger(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                normalize_trigger_value(trigg),
            ),
            steam_deck::event::TriggerEvent::RTrigger(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                normalize_trigger_value(trigg),
            ),
            steam_deck::event::TriggerEvent::LPadForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
            steam_deck::event::TriggerEvent::RPadForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
            steam_deck::event::TriggerEvent::LStickForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
            steam_deck::event::TriggerEvent::RStickForce(_) => {
                NativeEvent::new(Capability::NotImplemented, normalize_trigger_value(trigg))
            }
        },
    }
}

/// Represents the current state of a 4-way D-pad.
///
/// Why this exists:
/// - We synthesize D-pad button events (Up/Down/Left/Right) from analog inputs (e.g. left touchpad
///   position) and sometimes need to track what is currently "held".
/// - Tracking state lets us emit *only* the transitions (press/release diffs) instead of spamming
///   repeated button events every poll.
///
/// Expected behavior:
/// - Each field indicates whether that D-pad direction is currently considered pressed.
/// - Consumers should treat this as a *state snapshot* used for diffing (old -> new).
/// - In 4-way mode, at most one of {up, down, left, right} should be true at a time (unless you
///   intentionally allow diagonals elsewhere).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct DpadState {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

/// Converts a touch position (0..1, 0..1) into a 4-way D-pad state with hysteresis.
///
/// Why this exists:
/// - Touchpads are noisy near the center; hysteresis prevents rapid toggling (sticky/laggy feel).
///
/// Expected behavior:
/// - Returns `Default::default()` when within the dead-zone.
/// - When already active, requires a smaller threshold to remain active than to activate.
fn touch_to_dpad_hysteresis(x: f64, y: f64, old: DpadState) -> DpadState {
    let sx = (x - 0.5) * 2.0;
    let sy = (y - 0.5) * 2.0;

    const PRESS: f64 = 0.22;
    const RELEASE: f64 = 0.16;

    let mag = (sx * sx + sy * sy).sqrt();
    let was_active = old.up || old.down || old.left || old.right;

    // If we were active, don't release until we cross RELEASE
    if was_active && mag < RELEASE {
        return DpadState::default();
    }
    // If we were inactive, don't activate until we cross PRESS
    if !was_active && mag < PRESS {
        return DpadState::default();
    }

    if sx.abs() >= sy.abs() {
        if sx > 0.0 { DpadState { right: true, ..Default::default() } }
        else        { DpadState { left:  true, ..Default::default() } }
    } else {
        if sy > 0.0 { DpadState { down:  true, ..Default::default() } }
        else        { DpadState { up:    true, ..Default::default() } }
    }
}

/// Emit only the D-pad button transitions between two states.
///
/// Why this exists:
/// - When we synthesize D-pad buttons from an analog source (e.g. touchpad position),
///   we track the previous `DpadState` and the newly computed `DpadState`.
/// - Rather than emitting repeated button events every poll, we only emit events for
///   directions whose pressed state changed.
///
/// Expected behavior:
/// - Returns a list of `NativeEvent`s for each direction that changed.
/// - For each changed direction, emits `InputValue::Bool(true)` on press and
///   `InputValue::Bool(false)` on release.
/// - If `old == new`, returns an empty vector.
fn emit_dpad_diffs(old: DpadState, new: DpadState) -> Vec<NativeEvent> {
    let mut out = Vec::new();

    if old.up != new.up {
        out.push(NativeEvent::new(
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                                  InputValue::Bool(new.up),
        ));
    }
    if old.down != new.down {
        out.push(NativeEvent::new(
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                                  InputValue::Bool(new.down),
        ));
    }
    if old.left != new.left {
        out.push(NativeEvent::new(
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                                  InputValue::Bool(new.left),
        ));
    }
    if old.right != new.right {
        out.push(NativeEvent::new(
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                                  InputValue::Bool(new.right),
        ));
    }

    out
}

/// Translate raw Steam Deck driver events into `NativeEvent`s, with special handling for the left pad.
///
/// Why this exists:
/// - We want normal apps to continue receiving left touchpad motion + press events.
/// - Separately, we want the left pad to act like a virtual 4-way D-pad *only while the pad is
///   physically pressed*, to avoid “phantom” D-pad presses just from touch contact.
///
/// Expected behavior:
/// - Always passes through LeftPad motion as `Touchpad::LeftPad(Touch::Motion)` so cursor/motion
///   bindings keep working.
/// - Tracks the latest left pad position in `leftpad_last_xy`.
/// - Tracks whether the left pad is pressed in `leftpad_pressed` (from `LPadPress`).
/// - While pressed, converts the current touch position into a virtual D-pad state and emits only
///   press/release diffs via `emit_dpad_diffs`.
/// - On release, immediately forces the virtual D-pad state back to neutral (all false) and emits
///   the corresponding releases.
/// - All other events (including the physical D-pad) are passed through unchanged.
fn translate_events(
    events: Vec<steam_deck::event::Event>,
    leftpad_dpad: &mut DpadState,
    leftpad_pressed: &mut bool,
    leftpad_last_xy: &mut Option<(f64, f64)>,
) -> Vec<NativeEvent> {
    let mut out = Vec::new();

    for e in events {
        match e {
            // --- LEFT PAD PRESS: gate D-pad output, but still pass through press to apps ---
            steam_deck::event::Event::Button(steam_deck::event::ButtonEvent::LPadPress(v)) => {
                *leftpad_pressed = v.pressed;

                // Pass through so apps still see press
                out.push(translate_event(steam_deck::event::Event::Button(
                    steam_deck::event::ButtonEvent::LPadPress(v.clone()),
                )));

                // On release: force virtual dpad neutral immediately
                if !v.pressed {
                    let old = *leftpad_dpad;
                    let new_state = DpadState::default();
                    if new_state != old {
                        *leftpad_dpad = new_state;
                        out.extend(emit_dpad_diffs(old, new_state));
                    }
                }

                continue;
            }

            // --- LEFT PAD AXIS: always pass motion to apps; only emit dpad if pressed ---
            steam_deck::event::Event::Axis(steam_deck::event::AxisEvent::LPad(axis)) => {
                // 1) Motion passthrough (apps still see touchpad motion)
                out.push(NativeEvent::new(
                    Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
                                          normalize_axis_value(steam_deck::event::AxisEvent::LPad(axis.clone())),
                ));

                // 2) Compute normalized x/y for dpad logic
                let touch_val = normalize_axis_value(steam_deck::event::AxisEvent::LPad(axis));
                let (x, y) = match touch_val {
                    InputValue::Touch { x: Some(x), y: Some(y), .. } => (x, y),
                    _ => continue,
                };

                *leftpad_last_xy = Some((x, y));

                // 3) Update virtual dpad ONLY if pressed
                let old = *leftpad_dpad;
                let new_state = if *leftpad_pressed {
                    touch_to_dpad_hysteresis(x, y, old)
                } else {
                    DpadState::default()
                };

                if new_state != old {
                    *leftpad_dpad = new_state;
                    out.extend(emit_dpad_diffs(old, new_state));
                }

                continue;
            }

            // Everything else passthrough (INCLUDING PHYSICAL DPAD)
            other => out.push(translate_event(other)),
        }
    }

    out
}




/// List of all capabilities that the Steam Deck driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Accelerometer),
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
    Capability::Gamepad(Gamepad::Gyro),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
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
