use std::{error::Error, fmt::Debug};

use crate::{
    drivers::gpd_win_mini::{
        event, touchpad_driver::{self, TouchpadDriver}
    },
    input::{
        capability::{Capability, Gamepad, GamepadTrigger, Touch, TouchButton, Touchpad},
        event::{native::NativeEvent, value::{InputValue, normalize_unsigned_value}},
        source::{InputError, SourceInputDevice, SourceOutputDevice}
    },
    udev::device::UdevDevice,
};

/// GPD Win Mini source device implementation
pub struct GpdWinMiniTouchpad {
    driver: TouchpadDriver,
}

impl GpdWinMiniTouchpad {
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = TouchpadDriver::new(device_info)?;
        Ok(Self { driver })
    }
}

impl SourceInputDevice for GpdWinMiniTouchpad {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = translate_events(events);
        Ok(native_events)
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for GpdWinMiniTouchpad {}

impl Debug for GpdWinMiniTouchpad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpdWinMiniTouchpad").finish()
    }
}

/// Normalize the value to something between -1.0 and 1.0 based on the Deck's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: event::TouchAxisEvent) -> InputValue {
    let x = event.x;
    let y = event.y;
    log::trace!("Got axis to normalize: {x}, {y}");
    let max = touchpad_driver::TOUCHPAD_X_MAX;
    let x = normalize_unsigned_value(x as f64, max);

    let max = touchpad_driver::TOUCHPAD_Y_MAX;
    let y = normalize_unsigned_value(y as f64, max);

    // If this is an UP event, don't override the position of X/Y
    let (x, y) = if !event.is_touching {
        (None, None)
    } else {
        (Some(x), Some(y))
    };

    log::trace!("Normalized axis: {x:?}, {y:?}");
    InputValue::Touch {
        index: event.index,
        is_touching: event.is_touching,
        pressure: Some(1.0),
        x,
        y,
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the
/// maximum axis ranges.
fn normalize_trigger_value(event: event::TriggerEvent) -> InputValue {
    match event {
        event::TriggerEvent::PadForce(value) => {
            InputValue::Float(normalize_unsigned_value(
                value.value as f64,
                touchpad_driver::PAD_FORCE_MAX
            ))
        }
    }
}

/// Translate the given touchpad events into native events
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    let mut translated = Vec::with_capacity(events.len());
    for event in events.into_iter() {
        translated.push(translate_event(event));
    }
    translated
}

/// Translate the given touchpad event into a native event
fn translate_event(event: event::Event) -> NativeEvent {
    match event {
        event::Event::TouchAxis(axis) => {
            NativeEvent::new(
                Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
                normalize_axis_value(axis)
            )
        },
        event::Event::TouchButton(button) => match button {
            event::TouchButtonEvent::Left(value) => {
                NativeEvent::new(
                    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
                    InputValue::Bool(value.pressed),
                )
            },
        },
        event::Event::Trigger(trigg) => match trigg.clone() {
            event::TriggerEvent::PadForce(_) => {
                NativeEvent::new(
                    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)),
                    normalize_trigger_value(trigg),
                )
            },
        },
    }
}

/// List of all input capabilities that the GPD Win Mini touchpad driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
];
