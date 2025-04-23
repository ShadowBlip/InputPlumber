use std::fmt::Debug;
use std::{collections::HashMap, error::Error};

use evdev::{FFEffectData, FFEffectKind};
use packed_struct::types::SizedInteger;

use crate::drivers::dualsense::driver::{DS5_EDGE_PID, DS5_PID, DS5_VID};
use crate::drivers::steam_deck::hid_report::PackedRumbleReport;
use crate::input::output_capability::{OutputCapability, LED};
use crate::{
    drivers::dualsense::{self, driver::Driver},
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
pub const VID: u16 = DS5_VID;
/// Product IDs
pub const PIDS: [u16; 2] = [DS5_EDGE_PID, DS5_PID];

/// Sony Playstation DualSense Controller source device implementation
pub struct DualSenseController {
    driver: Driver,
    ff_evdev_effects: HashMap<i16, FFEffectData>,
}

impl DualSenseController {
    /// Create a new DualSense controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info.devnode())?;
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
                let left_speed = strong_magnitude / u8::MAX as u16 + 1;
                let right_speed = weak_magnitude / u8::MAX as u16 + 1;

                // Do rumble
                if let Err(e) = self.driver.rumble(left_speed as u8, right_speed as u8) {
                    let err = format!("Failed to do rumble: {:?}", e);
                    return Err(err.into());
                }
            }
        }

        Ok(())
    }

    /// Procces Steam Deck FFB events.
    fn process_deck_ff(&mut self, report: PackedRumbleReport) -> Result<(), Box<dyn Error>> {
        let left_speed = report.left_speed.to_primitive() / u8::MAX as u16 + 1;
        let right_speed = report.right_speed.to_primitive() / u8::MAX as u16 + 1;
        self.driver
            .rumble(left_speed as u8, right_speed as u8)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl SourceInputDevice for DualSenseController {
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

impl SourceOutputDevice for DualSenseController {
    /// Write the given output event to the source device. Output events are
    /// events that flow from an application (like a game) to the physical
    /// input device, such as force feedback events.
    fn write_event(&mut self, event: OutputEvent) -> Result<(), OutputError> {
        log::trace!("Received output event: {:?}", event);
        match event {
            OutputEvent::Evdev(input_event) => Ok(self.process_evdev_ff(input_event)?),
            OutputEvent::DualSense(report) => {
                log::debug!("Received DualSense output report");
                Ok(self.driver.write(report)?)
            }
            OutputEvent::Uinput(_) => Ok(()),
            OutputEvent::SteamDeckHaptics(_report) => Ok(()),
            OutputEvent::SteamDeckRumble(report) => {
                log::debug!("Received Steam Deck FFB Output Report");
                if let Err(e) = self.process_deck_ff(report) {
                    log::error!("Failed to process Steam Deck Force Feedback Report: {e:?}")
                }
                Ok(())
            }
            OutputEvent::LedColor { r, g, b } => {
                self.driver.set_led_color(r, g, b)?;
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

    /// Returns the output capabilities of the device
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![
            OutputCapability::ForceFeedback,
            OutputCapability::LED(LED::Color),
        ])
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
    events.into_iter().map(translate_event).collect()
}

/// Translate the given DualSense event into a native event
fn translate_event(event: dualsense::event::Event) -> NativeEvent {
    match event {
        dualsense::event::Event::Button(button) => match button {
            dualsense::event::ButtonEvent::Cross(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Circle(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Square(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Triangle(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Create(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Options(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Guide(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::Mute(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::Mute)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::DPadDown(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::DPadUp(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::DPadLeft(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::DPadRight(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::L1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::L2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::L3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::L4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::L5(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::R1(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::R2(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::R3(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::R4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::R5(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
                InputValue::Bool(value.pressed),
            ),
            dualsense::event::ButtonEvent::PadPress(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))),
                InputValue::Bool(value.pressed),
            ),
        },
        dualsense::event::Event::Accelerometer(accel) => match accel {
            dualsense::event::AccelerometerEvent::Accelerometer(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Accelerometer),
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
            dualsense::event::AccelerometerEvent::Gyro(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Gyro),
                InputValue::Vector3 {
                    x: Some(value.x as f64),
                    y: Some(value.y as f64),
                    z: Some(value.z as f64),
                },
            ),
        },
        dualsense::event::Event::Axis(ref axis) => match axis {
            dualsense::event::AxisEvent::Pad(_) => NativeEvent::new(
                Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)),
                normalize_axis_value(axis),
            ),
            dualsense::event::AxisEvent::LStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
                normalize_axis_value(axis),
            ),
            dualsense::event::AxisEvent::RStick(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
                normalize_axis_value(axis),
            ),
        },
        dualsense::event::Event::Trigger(ref trigger) => match trigger {
            dualsense::event::TriggerEvent::L2(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
                normalize_trigger_value(trigger),
            ),
            dualsense::event::TriggerEvent::R2(_) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
                normalize_trigger_value(trigger),
            ),
        },
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

/// Normalize the value to something between -1.0 and 1.0 based on the DualSense's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: &dualsense::event::AxisEvent) -> InputValue {
    match event {
        dualsense::event::AxisEvent::Pad(value) => {
            let min = 0.0;
            let max = dualsense::driver::DS5_TOUCHPAD_WIDTH;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = 0.0;
            let max = dualsense::driver::DS5_TOUCHPAD_HEIGHT;
            let y = normalize_signed_value(value.y as f64, min, max);
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
            let min = dualsense::driver::STICK_X_MIN;
            let max = dualsense::driver::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = dualsense::driver::STICK_Y_MIN;
            let max = dualsense::driver::STICK_Y_MAX;
            let y = normalize_signed_value(value.y as f64, min, max);
            let y = Some(y);

            InputValue::Vector2 { x, y }
        }
        dualsense::event::AxisEvent::RStick(value) => {
            let min = dualsense::driver::STICK_X_MIN;
            let max = dualsense::driver::STICK_X_MAX;
            let x = normalize_signed_value(value.x as f64, min, max);
            let x = Some(x);

            let min = dualsense::driver::STICK_Y_MIN;
            let max = dualsense::driver::STICK_Y_MAX;
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
            let max = dualsense::driver::TRIGGER_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
        dualsense::event::TriggerEvent::R2(value) => {
            let max = dualsense::driver::TRIGGER_MAX;
            InputValue::Float(normalize_unsigned_value(value.value as f64, max))
        }
    }
}

/// List of all capabilities that the DualSense driver implements
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
    Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Touch))),
    Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)),
];
