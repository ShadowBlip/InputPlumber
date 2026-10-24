use std::fmt::Debug;
use std::time::Instant;
use std::{collections::HashMap, collections::HashSet, error::Error};

use evdev::{FFEffectData, FFEffectKind};
use packed_struct::types::SizedInteger;

use crate::drivers::dualsense::{
    DS5_EDGE_PID, DS5_PID, DS5_TOUCHPAD_HEIGHT, DS5_TOUCHPAD_WIDTH, DS5_VID, STICK_X_MAX,
    STICK_X_MIN, STICK_Y_MAX, STICK_Y_MIN, TRIGGER_MAX,
};
use crate::drivers::steam_deck::hid_report::PackedTriggerVibration;
use crate::input::capability::Source;
use crate::input::output_capability::{Haptic, OutputCapability, LED};
use crate::{
    drivers::dualsense::{self, driver::Driver, event},
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
        output_event::OutputEvent,
        source::{
            magnitude, EventCapability, HapticPulseSeries, HapticState, InputError, OutputError,
            SourceInputDevice, SourceOutputDevice,
        },
    },
    udev::device::UdevDevice,
};

/// Vendor ID
pub const VID: u16 = DS5_VID;
/// Product IDs
pub const PIDS: [u16; 2] = [DS5_EDGE_PID, DS5_PID];

/// Sony Playstation DualSense Controller source device implementation
pub struct DualSenseController {
    driver: Driver,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
    haptic_state: Option<HapticState>,
}

impl DualSenseController {
    /// Create a new DualSense controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info.devnode())?;
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
        left: u8,
        right: u8,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let event = event::OutputEvent::HapticRumble { left, right };
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
                let left_speed = strong_magnitude / u8::MAX as u16 + 1;
                let right_speed = weak_magnitude / u8::MAX as u16 + 1;

                // Do rumble
                if let Err(e) =
                    Self::write_rumble(&mut self.driver, left_speed as u8, right_speed as u8)
                {
                    let err = format!("Failed to do rumble: {e:?}");
                    return Err(err.into());
                }
            }
        }

        Ok(())
    }

    /// Procces Steam Deck FFB events.
    fn process_deck_ff(&mut self, report: PackedTriggerVibration) -> Result<(), Box<dyn Error>> {
        let left_speed =
            magnitude(report.left_speed.to_primitive(), report.left_gain, 255.0) as u16 + 1;
        let right_speed =
            magnitude(report.right_speed.to_primitive(), report.right_gain, 255.0) as u16 + 1;
        log::trace!("Writing rumble left={left_speed} right={right_speed}");
        Self::write_rumble(&mut self.driver, left_speed as u8, right_speed as u8)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl SourceInputDevice for DualSenseController {
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
                        let _ = Self::write_rumble(&mut self.driver, strong, weak);
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

impl SourceOutputDevice for DualSenseController {
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
                let state = event::OutputEvent::State(report);
                Ok(self.driver.write_event(&state)?)
            }
            OutputEvent::Uinput(_) => Ok(()),
            OutputEvent::TriggerHapticPulse(_report) => Ok(()),
            OutputEvent::TriggerRepeatedHapticPulse(report) => {
                let series = HapticPulseSeries::new(&report, u8::MAX);
                let (weak, strong) = series.magnitudes();
                Self::write_rumble(&mut self.driver, strong, weak)?;
                self.haptic_state = Some(HapticState::Start(series));
                Ok(())
            }
            OutputEvent::TriggerVibration(report) => {
                log::debug!("Received Steam Deck FFB Output Report");
                if let Err(e) = self.process_deck_ff(report) {
                    log::error!("Failed to process Steam Deck Force Feedback Report: {e:?}")
                }
                Ok(())
            }
            OutputEvent::TriggerVibrationExtended(report) => {
                let report = report.combined_rumble();
                if let Err(e) = self.process_deck_ff(report) {
                    log::error!("Failed to process Trigger Vibration Extended Report: {e:?}")
                }
                Ok(())
            }
            OutputEvent::GenericRumble {
                weak_magnitude,
                strong_magnitude,
            } => {
                let left_speed = (strong_magnitude / 256) as u8;
                let right_speed = (weak_magnitude / 256) as u8;
                Self::write_rumble(&mut self.driver, left_speed, right_speed)?;
                Ok(())
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

impl Debug for DualSenseController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DualSenseController")
            .field("ff_evdev_effects", &self.ff_evdev_effects)
            .finish()
    }
}

/// Translate the given DualSense events into native events
fn translate_events(events: Vec<dualsense::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().filter_map(translate_event).collect()
}

/// Translate the given DualSense event into a native event
fn translate_event(event: dualsense::event::Event) -> Option<NativeEvent> {
    let value = event.to_value()?;
    Some(NativeEvent::new(event.to_capability(), value))
}

/// Normalize the value to something between -1.0 and 1.0 based on the DualSense's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: &dualsense::event::AxisEvent) -> InputValue {
    match event {
        dualsense::event::AxisEvent::Pad(value) => {
            let max = DS5_TOUCHPAD_WIDTH;

            let x = normalize_unsigned_value(value.x as f64, max);
            let x = Some(x);

            let max = DS5_TOUCHPAD_HEIGHT;
            let y = normalize_unsigned_value(value.y as f64, max);
            let y = Some(y);

            InputValue::Touch {
                index: value.index,
                is_touching: value.is_touching,
                pressure: Some(1.0),
                x,
                y,
            }
        }
        dualsense::event::AxisEvent::LStick(value) => {
            let min = STICK_X_MIN;
            let max = STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = STICK_Y_MIN;
            let max = STICK_Y_MAX;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
        dualsense::event::AxisEvent::RStick(value) => {
            let min = STICK_X_MIN;
            let max = STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = STICK_Y_MIN;
            let max = STICK_Y_MAX;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the Deck's
/// maximum axis ranges.
fn normalize_trigger_value(event: &dualsense::event::TriggerEvent) -> InputValue {
    match event {
        dualsense::event::TriggerEvent::L2(value) => {
            let max = TRIGGER_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        dualsense::event::TriggerEvent::R2(value) => {
            let max = TRIGGER_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

pub const OUTPUT_CAPABILITIES: &[OutputCapability] = &[
    OutputCapability::ForceFeedback,
    OutputCapability::LED(LED::Color),
    OutputCapability::Haptics(Haptic::TrackpadLeft),
    OutputCapability::Haptics(Haptic::TrackpadRight),
];

impl EventCapability for event::Event {
    fn to_value(&self) -> Option<InputValue> {
        match self {
            event::Event::Button(button) => Some(InputValue::Bool(button.pressed())),
            event::Event::Inertia(inertia) => match inertia {
                event::InertialEvent::Accelerometer(value)
                | event::InertialEvent::Gyroscope(value) => Some(InputValue::Vector3 {
                    x: Some(value.x),
                    y: Some(value.y),
                    z: Some(value.z),
                }),
            },
            event::Event::Axis(axis) => Some(normalize_axis_value(axis)),
            event::Event::Trigger(trigger) => Some(normalize_trigger_value(trigger)),
        }
    }

    fn to_capability(&self) -> Capability {
        match self {
            event::Event::Button(button) => match button {
                event::ButtonEvent::Cross(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::South))
                }
                event::ButtonEvent::Circle(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::East))
                }
                event::ButtonEvent::Square(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::North))
                }
                event::ButtonEvent::Triangle(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::West))
                }
                event::ButtonEvent::Create(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Select))
                }
                event::ButtonEvent::Options(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Start))
                }
                event::ButtonEvent::Guide(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide))
                }
                event::ButtonEvent::Mute(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Mute))
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
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger))
                }
                event::ButtonEvent::L3(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick))
                }
                event::ButtonEvent::L4(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1))
                }
                event::ButtonEvent::L5(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2))
                }
                event::ButtonEvent::R1(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper))
                }
                event::ButtonEvent::R2(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger))
                }
                event::ButtonEvent::R3(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick))
                }
                event::ButtonEvent::R4(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1))
                }
                event::ButtonEvent::R5(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2))
                }
                event::ButtonEvent::PadPress(_) => {
                    Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press)))
                }
            },
            event::Event::Inertia(inertia) => match inertia {
                event::InertialEvent::Accelerometer(_) => Capability::Accelerometer(Source::Center),
                event::InertialEvent::Gyroscope(_) => Capability::Gyroscope(Source::Center),
            },
            event::Event::Axis(axis) => match axis {
                event::AxisEvent::Pad(_) => {
                    Capability::Touchpad(Touchpad::CenterPad(Touch::Motion))
                }
                event::AxisEvent::LStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick))
                }
                event::AxisEvent::RStick(_) => {
                    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick))
                }
            },
            event::Event::Trigger(trigger) => match trigger {
                event::TriggerEvent::L2(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger))
                }
                event::TriggerEvent::R2(_) => {
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger))
                }
            },
        }
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        match capability {
            Capability::Gamepad(Gamepad::Button(button)) => {
                let event = match button {
                    GamepadButton::South => event::ButtonEvent::Cross(Default::default()),
                    GamepadButton::East => event::ButtonEvent::Circle(Default::default()),
                    GamepadButton::North => event::ButtonEvent::Square(Default::default()),
                    GamepadButton::West => event::ButtonEvent::Triangle(Default::default()),
                    GamepadButton::Select => event::ButtonEvent::Create(Default::default()),
                    GamepadButton::Start => event::ButtonEvent::Options(Default::default()),
                    GamepadButton::Guide => event::ButtonEvent::Guide(Default::default()),
                    GamepadButton::Mute => event::ButtonEvent::Mute(Default::default()),
                    GamepadButton::DPadDown => event::ButtonEvent::DPadDown(Default::default()),
                    GamepadButton::DPadUp => event::ButtonEvent::DPadUp(Default::default()),
                    GamepadButton::DPadLeft => event::ButtonEvent::DPadLeft(Default::default()),
                    GamepadButton::DPadRight => event::ButtonEvent::DPadRight(Default::default()),
                    GamepadButton::LeftBumper => event::ButtonEvent::L1(Default::default()),
                    GamepadButton::LeftTrigger => event::ButtonEvent::L2(Default::default()),
                    GamepadButton::LeftStick => event::ButtonEvent::L3(Default::default()),
                    GamepadButton::LeftPaddle1 => event::ButtonEvent::L4(Default::default()),
                    GamepadButton::LeftPaddle2 => event::ButtonEvent::L5(Default::default()),
                    GamepadButton::RightBumper => event::ButtonEvent::R1(Default::default()),
                    GamepadButton::RightTrigger => event::ButtonEvent::R2(Default::default()),
                    GamepadButton::RightStick => event::ButtonEvent::R3(Default::default()),
                    GamepadButton::RightPaddle1 => event::ButtonEvent::R4(Default::default()),
                    GamepadButton::RightPaddle2 => event::ButtonEvent::R5(Default::default()),
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
                    event::TriggerEvent::L2(Default::default()),
                )),
                GamepadTrigger::RightTrigger => Some(event::Event::Trigger(
                    event::TriggerEvent::R2(Default::default()),
                )),
                _ => None,
            },
            Capability::Accelerometer(source) if *source == Source::Center => Some(
                event::Event::Inertia(event::InertialEvent::Accelerometer(Default::default())),
            ),
            Capability::Gyroscope(source) if *source == Source::Center => Some(
                event::Event::Inertia(event::InertialEvent::Gyroscope(Default::default())),
            ),
            Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)) => Some(event::Event::Axis(
                event::AxisEvent::Pad(Default::default()),
            )),
            Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))) => Some(
                event::Event::Button(event::ButtonEvent::PadPress(Default::default())),
            ),
            _ => None,
        }
    }
}

/// List of all capabilities that the DualSense driver implements
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
    Capability::Gamepad(Gamepad::Button(GamepadButton::Mute)),
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
    Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Touch))),
    Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)),
];
