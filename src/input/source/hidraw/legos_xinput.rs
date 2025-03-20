use std::{collections::HashMap, error::Error, fmt::Debug};

use evdev::{FFEffectData, FFEffectKind, InputEvent};
use packed_struct::{types::SizedInteger, PrimitiveEnum};

use crate::{
    drivers::{
        dualsense::hid_report::SetStatePackedOutputData,
        legos::{
            event, xinput_driver::XInputDriver, STICK_X_MAX, STICK_X_MIN, STICK_Y_MAX, STICK_Y_MIN,
            TRIGG_MAX,
        },
        steam_deck::hid_report::{PackedHapticReport, PadSide},
    },
    input::{
        capability::{Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger},
        event::{native::NativeEvent, value::InputValue},
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Legion Go Controller source device implementation
pub struct LegionSXInputController {
    driver: XInputDriver,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
}

impl LegionSXInputController {
    /// Create a new Legion controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = XInputDriver::new(device_info.devnode())?;
        Ok(Self {
            driver,
            ff_evdev_effects: HashMap::new(),
        })
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
                let left_speed = (strong_magnitude / 256) as u8;
                let right_speed = (weak_magnitude / 256) as u8;

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
        let left_speed = report.rumble_emulation_left;
        let right_speed = report.rumble_emulation_right;

        if let Err(e) = self.driver.haptic_rumble(left_speed, right_speed) {
            let err = format!("Failed to do haptic rumble: {:?}", e);
            return Err(err.into());
        }

        Ok(())
    }

    fn process_haptic_ff(
        &self,
        report: PackedHapticReport,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let intensity = report.intensity.to_primitive() + 1;
        let scaled_gain = (report.gain + 24) as u8 * intensity;
        let normalized_gain = normalize_unsigned_value(scaled_gain as f64, 150.0);
        let new_gain = normalized_gain * u8::MAX as f64;
        let new_gain = new_gain as u8;

        match report.side {
            PadSide::Left => self.driver.haptic_rumble(new_gain, 0)?,
            PadSide::Right => self.driver.haptic_rumble(0, new_gain)?,
            PadSide::Both => self.driver.haptic_rumble(new_gain, new_gain)?,
        }

        Ok(())
    }
}

impl SourceInputDevice for LegionSXInputController {
    /// Poll the source device for input events
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

impl SourceOutputDevice for LegionSXInputController {
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
            OutputEvent::SteamDeckHaptics(report) => self.process_haptic_ff(report)?,
            OutputEvent::SteamDeckRumble(report) => {
                let l_speed = (report.left_speed.to_primitive() / 256) as u8;
                let r_speed = (report.right_speed.to_primitive() / 256) as u8;
                self.driver.haptic_rumble(l_speed, r_speed)?;
            }
        }

        Ok(())
    }
}

impl Debug for LegionSXInputController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegionSController").finish()
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
fn normalize_axis_value(event: event::AxisEvent) -> InputValue {
    match event {
        event::AxisEvent::LStick(value) => {
            let min = STICK_X_MIN;
            let max = STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = STICK_Y_MIN;
            let max = STICK_Y_MAX;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y);

            InputValue::Vector2 { x, y }
        }
        event::AxisEvent::RStick(value) => {
            let min = STICK_X_MIN;
            let max = STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = STICK_Y_MIN;
            let max = STICK_Y_MAX;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(-y);

            InputValue::Vector2 { x, y }
        }
        _ => InputValue::None,
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the
/// Legion Go's maximum axis ranges.
fn normalize_trigger_value(event: event::TriggerEvent) -> InputValue {
    match event {
        event::TriggerEvent::ATriggerL(value) => {
            let max = TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        event::TriggerEvent::ATriggerR(value) => {
            let max = TRIGG_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        _ => InputValue::None,
    }
}

/// Translate the given Legion Go events into native events
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given Legion Go event into a native event
fn translate_event(event: event::Event) -> NativeEvent {
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
            event::ButtonEvent::Legion(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::QuickAccess(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
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
            event::ButtonEvent::DTriggerL(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::ThumbL(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::Y1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::RB(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::DTriggerR(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::ThumbR(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
                InputValue::Bool(value.pressed),
            ),
            event::ButtonEvent::Y2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            _ => NativeEvent::new(Capability::NotImplemented, InputValue::None),
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
            _ => NativeEvent::new(Capability::NotImplemented, InputValue::None),
        },
        event::Event::Trigger(trigg) => match trigg.clone() {
            event::TriggerEvent::ATriggerL(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                normalize_trigger_value(trigg),
            ),
            event::TriggerEvent::ATriggerR(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                normalize_trigger_value(trigg),
            ),
            _ => NativeEvent::new(Capability::NotImplemented, InputValue::None),
        },
        event::Event::Inertia(_) => NativeEvent::new(Capability::NotImplemented, InputValue::None),
    }
}

/// List of all capabilities that the Legion Go driver implements
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
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
];
