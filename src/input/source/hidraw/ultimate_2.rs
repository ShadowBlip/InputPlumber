use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Debug,
    time::Instant,
};

use evdev::{FFEffectData, FFEffectKind};
use packed_struct::{types::SizedInteger, PrimitiveEnum};

use crate::{
    drivers::{
        dualsense::hid_report::SetStatePackedOutputData,
        steam_deck::hid_report::{CommandType, PackedTriggerHapticPulse, PadSide},
        ultimate_2::{
            driver::Driver, event, JOY_AXIS_MAX, JOY_AXIS_MIN, TRIGGER_AXIS_MAX,
            ULTIMATE_2_ACCEL_RAW_TO_MPS2, ULTIMATE_2_GYRO_RAW_TO_RAD_S,
        },
    },
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Source},
        event::{
            native::NativeEvent,
            value::{
                normalize_accel_value_i16, normalize_gyro_value_i16, normalize_signed_value,
                normalize_unsigned_value, InputValue,
            },
        },
        output_capability::{Haptic, OutputCapability},
        output_event::OutputEvent,
        source::{
            haptic_duration, EventCapability, HapticPulseSeries, HapticState, InputError,
            OutputError, SourceInputDevice, SourceOutputDevice,
        },
    },
    udev::device::UdevDevice,
};

/// XpadUhid source device implementation
pub struct Ultimate2 {
    driver: Driver,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
    haptic_state: Option<HapticState>,
}

impl Ultimate2 {
    /// Create a new source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info)?;
        Ok(Self {
            driver,
            ff_evdev_effects: HashMap::new(),
            haptic_state: None,
        })
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
        weak: u8,
        strong: u8,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let event = event::OutputEvent::Rumble { weak, strong };
        driver.write_event(&event)
    }

    /// Process the given evdev force feedback event.
    fn process_evdev_ff(&mut self, input_event: evdev::InputEvent) -> Result<(), Box<dyn Error>> {
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
            self.haptic_state = None;
            log::trace!("Stopping rumble");
            if let Err(e) = Self::write_rumble(&mut self.driver, 0, 0) {
                log::debug!("Failed to stop rumble: {:?}", e);
                return Ok(());
            }
            return Ok(());
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
                // Scale the rumble values to the DS5 values
                let strong =
                    ((strong_magnitude as f64 / u16::MAX as f64) * u8::MAX as f64).round() as u8;
                let weak =
                    ((weak_magnitude as f64 / u16::MAX as f64) * u8::MAX as f64).round() as u8;

                // Do rumble
                if let Err(e) = Self::write_rumble(&mut self.driver, weak, strong) {
                    let err = format!("Failed to do rumble: {e:?}");
                    return Err(err.into());
                }
            }
        }

        Ok(())
    }

    /// Process DualSense-style force feedback output reports
    fn process_dualsense_ff(
        &mut self,
        report: SetStatePackedOutputData,
    ) -> Result<(), Box<dyn Error>> {
        // The DualSense report carries per-motor speeds in FF convention
        if let Err(e) = Self::write_rumble(
            &mut self.driver,
            report.rumble_emulation_right,
            report.rumble_emulation_left,
        ) {
            let err = format!("Failed to do rumble: {e:?}");
            return Err(err.into());
        }

        Ok(())
    }

    /// Process Steam Deck-style haptic output reports
    fn process_haptic_ff(
        &mut self,
        report: PackedTriggerHapticPulse,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if report.cmd_type == CommandType::Off {
            self.haptic_state = None;
            Self::write_rumble(&mut self.driver, 0, 0)?;
            return Ok(());
        }

        let intensity = report.intensity.to_primitive() + 1;
        let scaled_gain = ((report.gain as i16 + 24).clamp(0, 30) as u8) * intensity;
        let normalized_gain = normalize_unsigned_value(scaled_gain as f64, 150.0);
        let new_gain = normalized_gain * u8::MAX as f64;
        let new_gain = new_gain as u8;

        let (weak, strong) = match report.side {
            PadSide::Left => (0, new_gain),
            PadSide::Right => (new_gain, 0),
            PadSide::Both => (new_gain, new_gain),
        };
        Self::write_rumble(&mut self.driver, weak, strong)?;

        // Level-based motors need a scheduled stop
        self.haptic_state = Some(HapticState::Stop(Instant::now() + haptic_duration(&report)));

        Ok(())
    }
}

impl SourceInputDevice for Ultimate2 {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        if let Some(state) = &mut self.haptic_state {
            match state {
                HapticState::Stop(stop_at) => {
                    if Instant::now() >= *stop_at {
                        self.haptic_state = None;
                        let _ = Self::write_rumble(&mut self.driver, 0, 0);
                    }
                }
                HapticState::Start(series) => {
                    if let Some((weak, strong)) = series.advance(Instant::now()) {
                        let _ = Self::write_rumble(&mut self.driver, weak, strong);
                    }
                    if series.is_finished() {
                        self.haptic_state = None;
                    }
                }
            }
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

impl SourceOutputDevice for Ultimate2 {
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
            OutputEvent::Evdev(input_event) => Ok(self.process_evdev_ff(input_event)?),
            OutputEvent::DualSense(report) => {
                log::debug!("Received DualSense output report");
                if report.use_rumble_not_haptics || report.enable_improved_rumble_emulation {
                    self.process_dualsense_ff(report)?;
                }
                Ok(())
            }
            OutputEvent::Uinput(_) => Ok(()),
            OutputEvent::TriggerHapticPulse(report) => Ok(self.process_haptic_ff(report)?),
            OutputEvent::TriggerRepeatedHapticPulse(report) => {
                let series = HapticPulseSeries::new(&report, u8::MAX);
                let (weak, strong) = series.magnitudes();
                Self::write_rumble(&mut self.driver, weak, strong)?;
                self.haptic_state = Some(HapticState::Start(series));
                Ok(())
            }
            OutputEvent::TriggerVibration(report) => {
                let strong = (report.left_speed.to_primitive() / 256) as u8;
                let weak = (report.right_speed.to_primitive() / 256) as u8;
                Ok(Self::write_rumble(&mut self.driver, weak, strong)?)
            }
            OutputEvent::TriggerVibrationExtended(report) => {
                let combined = report.combined_rumble();
                let strong = (combined.left_speed.to_primitive() / 256) as u8;
                let weak = (combined.right_speed.to_primitive() / 256) as u8;
                Ok(Self::write_rumble(&mut self.driver, weak, strong)?)
            }
            OutputEvent::GenericRumble {
                weak_magnitude,
                strong_magnitude,
            } => {
                let strong =
                    ((strong_magnitude as f64 / u16::MAX as f64) * u8::MAX as f64).round() as u8;
                let weak =
                    ((weak_magnitude as f64 / u16::MAX as f64) * u8::MAX as f64).round() as u8;
                Ok(Self::write_rumble(&mut self.driver, weak, strong)?)
            }
        }
    }

    /// Upload the given force feedback effect data to the source device. Returns
    /// a device-specific id of the uploaded effect if it is successful.
    fn upload_effect(&mut self, effect: FFEffectData) -> Result<i16, OutputError> {
        log::debug!("Uploading FF effect data");
        let id = self.next_ff_effect_id();
        if id == -1 {
            return Err("Maximum FF effects uploaded".into());
        }
        self.ff_evdev_effects.insert(id, effect);

        Ok(id)
    }

    /// Update the effect with the given id using the given effect data.
    fn update_effect(&mut self, effect_id: i16, effect: FFEffectData) -> Result<(), OutputError> {
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
}

impl Debug for Ultimate2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XpadUhid").finish()
    }
}

/// Normalize the value to something between 0 and 1.0 based on the
/// maximum axis range.
fn normalize_axis_value(event: &event::AxisEvent) -> InputValue {
    let max = JOY_AXIS_MAX;
    let min = JOY_AXIS_MIN;
    match event {
        event::AxisEvent::LStick(value) => {
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
        event::AxisEvent::RStick(value) => {
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the
/// maximum axis range.
fn normalize_trigger_value(event: &event::TriggerEvent) -> InputValue {
    let max = TRIGGER_AXIS_MAX;
    match event {
        event::TriggerEvent::TriggerL(value) => {
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::TriggerR(value) => {
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// Translate the given events into native events
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    events.into_iter().filter_map(translate_event).collect()
}

/// Translate the given event into a native event
fn translate_event(event: event::Event) -> Option<NativeEvent> {
    let value = event.to_value()?;
    Some(NativeEvent::new(event.to_capability(), value))
}

impl EventCapability for event::Event {
    fn to_value(&self) -> Option<InputValue> {
        match self {
            event::Event::Button(button) => Some(InputValue::Bool(button.pressed())),
            event::Event::Axis(axis) => Some(normalize_axis_value(axis)),
            event::Event::Trigger(trigger) => Some(normalize_trigger_value(trigger)),
            event::Event::Inertia(inertia) => match inertia {
                event::InertialEvent::Accelerometer(value) => Some(InputValue::Vector3 {
                    x: Some(normalize_accel_value_i16(
                        value.x,
                        ULTIMATE_2_ACCEL_RAW_TO_MPS2,
                    )),
                    y: Some(normalize_accel_value_i16(
                        value.y,
                        ULTIMATE_2_ACCEL_RAW_TO_MPS2,
                    )),
                    z: Some(normalize_accel_value_i16(
                        value.z,
                        ULTIMATE_2_ACCEL_RAW_TO_MPS2,
                    )),
                }),
                event::InertialEvent::Gyroscope(value) => Some(InputValue::Vector3 {
                    x: Some(normalize_gyro_value_i16(
                        value.x,
                        ULTIMATE_2_GYRO_RAW_TO_RAD_S,
                    )),
                    y: Some(normalize_gyro_value_i16(
                        value.y,
                        ULTIMATE_2_GYRO_RAW_TO_RAD_S,
                    )),
                    z: Some(normalize_gyro_value_i16(
                        value.z,
                        ULTIMATE_2_GYRO_RAW_TO_RAD_S,
                    )),
                }),
            },
        }
    }

    fn to_capability(&self) -> Capability {
        match self {
            event::Event::Button(button) => match button {
                event::ButtonEvent::A(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::South))
                }
                event::ButtonEvent::X(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::North))
                }
                event::ButtonEvent::B(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::East))
                }
                event::ButtonEvent::Y(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::West))
                }
                event::ButtonEvent::Menu(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Start))
                }
                event::ButtonEvent::View(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Select))
                }
                event::ButtonEvent::Guide(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide))
                }
                event::ButtonEvent::DPadDown(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown))
                }
                event::ButtonEvent::DPadUp(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp))
                }
                event::ButtonEvent::DPadLeft(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft))
                }
                event::ButtonEvent::DPadRight(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight))
                }
                event::ButtonEvent::L1(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper))
                }
                event::ButtonEvent::L2(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick))
                }
                event::ButtonEvent::L3(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1))
                }
                event::ButtonEvent::L4(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2))
                }
                event::ButtonEvent::R1(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper))
                }
                event::ButtonEvent::R2(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick))
                }
                event::ButtonEvent::R3(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1))
                }
                event::ButtonEvent::R4(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2))
                }
            },
            event::Event::Axis(axis) => match axis {
                event::AxisEvent::LStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
                }
                event::AxisEvent::RStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
                }
            },
            event::Event::Trigger(trigg) => match trigg {
                event::TriggerEvent::TriggerL(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
                }
                event::TriggerEvent::TriggerR(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
                }
            },
            event::Event::Inertia(inertia) => match inertia {
                event::InertialEvent::Accelerometer(_) => Capability::Accelerometer(Source::Center),
                event::InertialEvent::Gyroscope(_) => Capability::Gyroscope(Source::Center),
            },
        }
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        match capability {
            Capability::Gamepad(Gamepad::Button(button)) => {
                let event = match button {
                    GamepadButton::South => event::ButtonEvent::A(Default::default()),
                    GamepadButton::North => event::ButtonEvent::X(Default::default()),
                    GamepadButton::East => event::ButtonEvent::B(Default::default()),
                    GamepadButton::West => event::ButtonEvent::Y(Default::default()),
                    GamepadButton::Start => event::ButtonEvent::Menu(Default::default()),
                    GamepadButton::Select => event::ButtonEvent::View(Default::default()),
                    GamepadButton::Guide => event::ButtonEvent::Guide(Default::default()),
                    GamepadButton::DPadDown => event::ButtonEvent::DPadDown(Default::default()),
                    GamepadButton::DPadUp => event::ButtonEvent::DPadUp(Default::default()),
                    GamepadButton::DPadLeft => event::ButtonEvent::DPadLeft(Default::default()),
                    GamepadButton::DPadRight => event::ButtonEvent::DPadRight(Default::default()),
                    GamepadButton::LeftBumper => event::ButtonEvent::L1(Default::default()),
                    GamepadButton::LeftStick => event::ButtonEvent::L2(Default::default()),
                    GamepadButton::LeftPaddle1 => event::ButtonEvent::L3(Default::default()),
                    GamepadButton::LeftPaddle2 => event::ButtonEvent::L4(Default::default()),
                    GamepadButton::RightBumper => event::ButtonEvent::R1(Default::default()),
                    GamepadButton::RightStick => event::ButtonEvent::R2(Default::default()),
                    GamepadButton::RightPaddle1 => event::ButtonEvent::R3(Default::default()),
                    GamepadButton::RightPaddle2 => event::ButtonEvent::R4(Default::default()),
                    _ => return None,
                };
                Some(event::Event::Button(event))
            }
            Capability::Gamepad(Gamepad::Axis(axis)) => match axis {
                GamepadAxis::LeftStick => Some(event::Event::Axis(event::AxisEvent::LStick(
                    Default::default(),
                ))),
                GamepadAxis::RightStick => Some(event::Event::Axis(event::AxisEvent::RStick(
                    Default::default(),
                ))),
                _ => None,
            },
            Capability::Gamepad(Gamepad::Trigger(trigger)) => match trigger {
                GamepadTrigger::LeftTrigger => Some(event::Event::Trigger(
                    event::TriggerEvent::TriggerL(Default::default()),
                )),
                GamepadTrigger::RightTrigger => Some(event::Event::Trigger(
                    event::TriggerEvent::TriggerR(Default::default()),
                )),
                _ => None,
            },
            Capability::Accelerometer(Source::Center) => Some(event::Event::Inertia(
                event::InertialEvent::Accelerometer(Default::default()),
            )),
            Capability::Gyroscope(Source::Center) => Some(event::Event::Inertia(
                event::InertialEvent::Gyroscope(Default::default()),
            )),
            _ => None,
        }
    }
}

/// List of all capabilities that the driver implements
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
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
    Capability::Gyroscope(Source::Center),
];

pub const OUTPUT_CAPABILITIES: &[OutputCapability] = &[
    OutputCapability::ForceFeedback,
    OutputCapability::ForceFeedbackUpload,
    OutputCapability::ForceFeedbackErase,
    OutputCapability::Haptics(Haptic::TrackpadLeft),
    OutputCapability::Haptics(Haptic::TrackpadRight),
];
