use std::{
    collections::HashMap,
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
            driver::{Driver, ACCEL_SCALE},
            hid_report::LIZARD_SLEEP_SEC,
        },
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Touch, TouchButton,
            Touchpad,
        },
        event::{native::NativeEvent, value::InputValue},
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Vendor ID
pub const VID: u16 = 0x28de;
/// Product ID
pub const PID: u16 = 0x1205;

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
            if let Err(e) = self.driver.haptic_rumble(0, 0, 0, 0, 0) {
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
                let intensity = 0;
                let left_speed = strong_magnitude;
                let right_speed = weak_magnitude;
                let mut left_gain = 0; // Max 130
                let mut right_gain = 0;
                if left_speed == 0 {
                    left_gain = 0;
                }
                if right_speed == 0 {
                    right_gain = 0;
                }

                // Do rumble
                if let Err(e) = self.driver.haptic_rumble(
                    intensity,
                    left_speed,
                    right_speed,
                    left_gain,
                    right_gain,
                ) {
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
        let intensity = 0;
        let left_speed = report.rumble_emulation_left as u16 * 256;
        let right_speed = report.rumble_emulation_right as u16 * 256;
        let mut left_gain = 0; // Max 130
        let mut right_gain = 0;
        if left_speed == 0 {
            left_gain = 0;
        }
        if right_speed == 0 {
            right_gain = 0;
        }

        if let Err(e) =
            self.driver
                .haptic_rumble(intensity, left_speed, right_speed, left_gain, right_gain)
        {
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
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for DeckController {
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

/// Returns a value between -1.0 and 1.0 based on the given value with its
/// minimum and maximum values.
fn normalize_signed_value(raw_value: f64, min: f64, max: f64) -> f64 {
    let mid = (max + min) / 2.0;
    let event_value = raw_value - mid;

    // Normalize the value
    if event_value >= 0.0 {
        let maximum = max - mid;
        event_value / maximum
    } else {
        let minimum = min - mid;
        let value = event_value / minimum;
        -value
    }
}

// Returns a value between 0.0 and 1.0 based on the given value with its
// maximum.
fn normalize_unsigned_value(raw_value: f64, max: f64) -> f64 {
    raw_value / max
}

/// Normalize the value to something between -1.0 and 1.0 based on the Deck's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: steam_deck::event::AxisEvent) -> InputValue {
    match event {
        steam_deck::event::AxisEvent::LPad(value) => {
            let min = steam_deck::hid_report::PAD_X_MIN;
            let max = steam_deck::hid_report::PAD_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = steam_deck::hid_report::PAD_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::PAD_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

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
            let x = Some(x);

            let min = steam_deck::hid_report::PAD_Y_MAX; // uses inverted Y-axis
            let max = steam_deck::hid_report::PAD_Y_MIN;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y); // Y-Axis is inverted

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

/// Translate the given Steam Deck events into native events
fn translate_events(events: Vec<steam_deck::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
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
