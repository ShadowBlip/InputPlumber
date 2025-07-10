use std::{collections::HashMap, error::Error, fmt::Debug};

use evdev::{FFEffectData, FFEffectKind};

use crate::{
    drivers::xpad_uhid::{
        driver::{Driver, JOY_AXIS_MAX, JOY_AXIS_MIN, TRIGGER_AXIS_MAX},
        event,
    },
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        event::{native::NativeEvent, value::InputValue},
        output_capability::OutputCapability,
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// XpadUhid source device implementation
pub struct XpadUhid {
    driver: Driver,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
}

impl XpadUhid {
    /// Create a new source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info)?;
        Ok(Self {
            driver,
            ff_evdev_effects: HashMap::new(),
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
            log::trace!("Stopping rumble");
            if let Err(e) = self.driver.rumble(0, 0) {
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
                let left_speed = (strong_magnitude as f64 / u16::MAX as f64) * u8::MAX as f64;
                let left_speed = left_speed.round() as u8;
                let right_speed = (weak_magnitude as f64 / u16::MAX as f64) * u8::MAX as f64;
                let right_speed = right_speed.round() as u8;

                // Do rumble
                if let Err(e) = self.driver.rumble(left_speed, right_speed) {
                    let err = format!("Failed to do rumble: {e:?}");
                    return Err(err.into());
                }
            }
        }

        Ok(())
    }
}

impl SourceInputDevice for XpadUhid {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = translate_events(events);
        Ok(native_events)
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for XpadUhid {
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
            OutputEvent::DualSense(_) => Ok(()),
            OutputEvent::Uinput(_) => Ok(()),
            OutputEvent::SteamDeckHaptics(_packed_haptic_report) => Ok(()),
            OutputEvent::SteamDeckRumble(_packed_rumble_report) => Ok(()),
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

impl Debug for XpadUhid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XpadUhid").finish()
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

/// Normalize the value to something between -1.0 and 1.0 based on the
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: event::AxisEvent) -> InputValue {
    let min = JOY_AXIS_MIN;
    let max = JOY_AXIS_MAX;
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
fn normalize_trigger_value(event: event::TriggerEvent) -> InputValue {
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
    let mut translated = Vec::with_capacity(events.len());
    for event in events.into_iter() {
        translated.push(translate_event(event));
    }
    if !translated.is_empty() {
        log::debug!("Translated events: {translated:?}");
    };
    translated
}

/// Translate the given event into a native event
fn translate_event(event: event::Event) -> NativeEvent {
    log::debug!("Got event {event:?}");
    match event {
        event::Event::Button(button) => match button {
            event::ButtonEvent::A(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::X(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::B(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::Y(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::Menu(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::View(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::Guide(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::DPadDown(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::DPadUp(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::DPadLeft(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::DPadRight(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::LB(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::ThumbL(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::RB(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::ThumbR(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
                InputValue::Bool(value.pressed),
            ),
        },
        event::Event::Axis(axis) => match axis.clone() {
            event::AxisEvent::LStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                normalize_axis_value(axis),
            ),
            event::AxisEvent::RStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                normalize_axis_value(axis),
            ),
        },
        event::Event::Trigger(trigg) => match trigg.clone() {
            event::TriggerEvent::TriggerL(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                normalize_trigger_value(trigg),
            ),
            event::TriggerEvent::TriggerR(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                normalize_trigger_value(trigg),
            ),
        },
    }
}

/// List of all capabilities that the driver implements
pub const CAPABILITIES: &[Capability] = &[
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
];

pub const OUTPUT_CAPABILITIES: &[OutputCapability] = &[OutputCapability::ForceFeedback];
