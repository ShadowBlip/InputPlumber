use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Debug,
    time::Instant,
};

use evdev::{FFEffectData, FFEffectKind, InputEvent};
use packed_struct::types::SizedInteger;

use crate::{
    drivers::{
        dualsense::hid_report::SetStatePackedOutputData,
        msi_claw::{driver::Driver, event, AXIS_MAX},
        steam_deck::hid_report::{CommandType, PackedTriggerHapticPulse, PadSide},
    },
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        event::{
            native::NativeEvent,
            value::{normalize_signed_value, normalize_unsigned_value, InputValue},
        },
        output_capability::{Haptic, OutputCapability},
        output_event::OutputEvent,
        source::{
            haptic_duration, haptic_gain, magnitude, EventCapability, HapticPulseSeries,
            HapticState, InputError, OutputError, SourceInputDevice, SourceOutputDevice,
            HAPTIC_NOMINAL_PERCENT,
        },
    },
    udev::device::UdevDevice,
};

/// MSI ClawController source device implementation
pub struct MsiClawController {
    driver: Driver,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
    haptic_state: Option<HapticState>,
}

impl MsiClawController {
    /// Create a new MSI Claw controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info.devnode())?;
        Ok(Self {
            driver,
            ff_evdev_effects: HashMap::new(),
            haptic_state: None,
        })
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
            self.haptic_state = None;
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
                // Set rumble values based on the effect data
                let strong = (strong_magnitude / 256) as u8;
                let weak = (weak_magnitude / 256) as u8;

                // Do rumble
                if let Err(e) = Self::write_rumble(&mut self.driver, weak, strong) {
                    let err = format!("Failed to do haptic rumble: {e:?}");
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
        // The DualSense report carries per-motor speeds in FF convention
        if let Err(e) = Self::write_rumble(
            &mut self.driver,
            report.rumble_emulation_right,
            report.rumble_emulation_left,
        ) {
            let err = format!("Failed to do haptic rumble: {e:?}");
            return Err(err.into());
        }

        Ok(())
    }

    fn process_haptic_ff(
        &mut self,
        report: PackedTriggerHapticPulse,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if report.cmd_type == CommandType::Off {
            self.haptic_state = None;
            Self::write_rumble(&mut self.driver, 0, 0)?;
            return Ok(());
        }

        let new_gain = haptic_gain(report.gain, u8::MAX as f64, HAPTIC_NOMINAL_PERCENT) as u8;

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

impl SourceInputDevice for MsiClawController {
    /// Poll the source device for input events
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
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }

    fn update_event_filter(&mut self, events: HashSet<Capability>) -> Result<(), InputError> {
        let filtered = events
            .iter()
            .filter_map(EventCapability::from_capability)
            .collect();
        self.driver.update_filtered_events(filtered);
        Ok(())
    }
}

impl SourceOutputDevice for MsiClawController {
    /// Returns the possible output events this device is capable of handling
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        let capabilities = vec![
            OutputCapability::ForceFeedback,
            OutputCapability::ForceFeedbackUpload,
            OutputCapability::ForceFeedbackErase,
            OutputCapability::Haptics(Haptic::TrackpadLeft),
            OutputCapability::Haptics(Haptic::TrackpadRight),
            //OutputCapability::LED(LED::Color),
            //OutputCapability::LED(LED::Brightness),
        ];
        Ok(capabilities)
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
            OutputEvent::TriggerHapticPulse(report) => self.process_haptic_ff(report)?,
            OutputEvent::TriggerRepeatedHapticPulse(report) => {
                let series = HapticPulseSeries::new(&report, u8::MAX);
                let (weak, strong) = series.magnitudes();
                Self::write_rumble(&mut self.driver, weak, strong)?;
                self.haptic_state = Some(HapticState::Start(series));
            }
            OutputEvent::TriggerVibration(report) => {
                let strong =
                    magnitude(report.left_speed.to_primitive(), report.left_gain, 255.0) as u8;
                let weak =
                    magnitude(report.right_speed.to_primitive(), report.right_gain, 255.0) as u8;
                log::trace!("Writing rumble strong={strong} weak={weak}");
                Self::write_rumble(&mut self.driver, weak, strong)?;
            }
            OutputEvent::TriggerVibrationExtended(report) => {
                let combined = report.combined_rumble();
                let strong = magnitude(
                    combined.left_speed.to_primitive(),
                    combined.left_gain,
                    255.0,
                ) as u8;
                let weak = magnitude(
                    combined.right_speed.to_primitive(),
                    combined.right_gain,
                    255.0,
                ) as u8;
                log::trace!("Writing rumble strong={strong} weak={weak}");
                Self::write_rumble(&mut self.driver, weak, strong)?;
            }
            OutputEvent::GenericRumble {
                weak_magnitude,
                strong_magnitude,
            } => {
                Self::write_rumble(
                    &mut self.driver,
                    (weak_magnitude / 256) as u8,
                    (strong_magnitude / 256) as u8,
                )?;
            }
        }

        Ok(())
    }
}

impl Debug for MsiClawController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MSI ClawSController").finish()
    }
}

/// Normalize the value to something between -1.0 and 1.0 based on the Deck's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: &event::AxisEvent) -> InputValue {
    match event {
        event::AxisEvent::LStick(value) => {
            let max = AXIS_MAX;
            let min = 0.0;

            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
        event::AxisEvent::RStick(value) => {
            let max = AXIS_MAX;
            let min = 0.0;

            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the
/// MSI Claw Go's maximum axis ranges.
fn normalize_trigger_value(event: &event::TriggerEvent) -> InputValue {
    match event {
        event::TriggerEvent::ATriggerL(value) => {
            let max = AXIS_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::ATriggerR(value) => {
            let max = AXIS_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// Translate the given MSI Clawevents into native events
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    events.into_iter().filter_map(translate_event).collect()
}

/// Translate the given MSI Claw event into a native event
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
                event::ButtonEvent::LB(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper))
                }
                event::ButtonEvent::ThumbL(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick))
                }
                event::ButtonEvent::RB(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper))
                }
                event::ButtonEvent::ThumbR(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick))
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
            event::Event::Trigger(trigger) => match trigger {
                event::TriggerEvent::ATriggerL(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
                }
                event::TriggerEvent::ATriggerR(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
                }
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
                    GamepadButton::DPadDown => event::ButtonEvent::DPadDown(Default::default()),
                    GamepadButton::DPadUp => event::ButtonEvent::DPadUp(Default::default()),
                    GamepadButton::DPadLeft => event::ButtonEvent::DPadLeft(Default::default()),
                    GamepadButton::DPadRight => event::ButtonEvent::DPadRight(Default::default()),
                    GamepadButton::LeftBumper => event::ButtonEvent::LB(Default::default()),
                    GamepadButton::LeftStick => event::ButtonEvent::ThumbL(Default::default()),
                    GamepadButton::RightBumper => event::ButtonEvent::RB(Default::default()),
                    GamepadButton::RightStick => event::ButtonEvent::ThumbR(Default::default()),
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
                    event::TriggerEvent::ATriggerL(Default::default()),
                )),
                GamepadTrigger::RightTrigger => Some(event::Event::Trigger(
                    event::TriggerEvent::ATriggerR(Default::default()),
                )),
                _ => None,
            },
            _ => None,
        }
    }
}

/// List of all capabilities that the MSI Clawdriver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
];
