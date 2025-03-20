use std::{error::Error, fmt::Debug};

use crate::{
    drivers::legos::{
        event,
        touchpad_driver::{self, TouchpadDriver},
    },
    input::{
        capability::{Capability, Touch, TouchButton, Touchpad},
        event::{native::NativeEvent, value::InputValue},
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Legion Go Controller source device implementation
pub struct LegionSTouchpadController {
    driver: TouchpadDriver,
}

impl LegionSTouchpadController {
    /// Create a new Legion controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = TouchpadDriver::new(device_info.devnode())?;
        Ok(Self { driver })
    }
}

impl SourceInputDevice for LegionSTouchpadController {
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

impl SourceOutputDevice for LegionSTouchpadController {
    /// Write the given output event to the source device. Output events are
    /// events that flow from an application (like a game) to the physical
    /// input device, such as force feedback events.
    fn write_event(&mut self, _event: OutputEvent) -> Result<(), OutputError> {
        Ok(())
    }
}

impl Debug for LegionSTouchpadController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegionSController").finish()
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
        event::AxisEvent::Touchpad(value) => {
            let max = touchpad_driver::PAD_X_MAX;
            let x = normalize_unsigned_value(value.x as f64, max);

            let max = touchpad_driver::PAD_Y_MAX;
            let y = normalize_unsigned_value(value.y as f64, max);

            // If this is an UP event, don't override the position of X/Y
            let (x, y) = if !value.is_touching {
                (None, None)
            } else {
                (Some(x), Some(y))
            };

            InputValue::Touch {
                index: value.index,
                is_touching: value.is_touching,
                pressure: Some(1.0),
                x,
                y,
            }
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
        event::Event::Button(button) => match button.clone() {
            event::ButtonEvent::RPadPress(value) => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
                InputValue::Bool(value.pressed),
            ),
            _ => NativeEvent::new(Capability::NotImplemented, InputValue::None),
        },
        event::Event::Axis(axis) => match axis.clone() {
            event::AxisEvent::Touchpad(_) => NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
                normalize_axis_value(axis),
            ),
            _ => NativeEvent::new(Capability::NotImplemented, InputValue::None),
        },
        _ => NativeEvent::new(Capability::NotImplemented, InputValue::None),
    }
}

/// List of all capabilities that the Legion Go driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Touch))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
];
