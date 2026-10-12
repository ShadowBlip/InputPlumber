use std::{collections::HashSet, error::Error, fmt::Debug};

use crate::{
    drivers::gpd_device::{
        event, touchpad_driver_2024::TouchpadDriver2024, TOUCHPAD_2024_PAD_FORCE_MAX,
        TOUCHPAD_2024_X_MAX, TOUCHPAD_2024_Y_MAX,
    },
    input::{
        capability::{Capability, Gamepad, GamepadTrigger, Touch, TouchButton, Touchpad},
        event::{
            native::NativeEvent,
            value::{normalize_unsigned_value, InputValue},
        },
        source::{EventCapability, InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// GPD Win Mini (2024+) source device implementation
pub struct GpdTouchpad2024 {
    driver: TouchpadDriver2024,
}

impl GpdTouchpad2024 {
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = TouchpadDriver2024::new(device_info)?;
        Ok(Self { driver })
    }
}

impl SourceInputDevice for GpdTouchpad2024 {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = translate_events(events);
        Ok(native_events)
    }

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

impl SourceOutputDevice for GpdTouchpad2024 {}

impl Debug for GpdTouchpad2024 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpdWinMiniTouchpad2024").finish()
    }
}

/// Normalize the value to something between -1.0 and 1.0 based on the
/// touchpad's minimum and maximum axis ranges.
fn normalize_axis_value(event: &event::TouchAxisEvent) -> InputValue {
    let x = event.x;
    let y = event.y;
    log::trace!("Got axis to normalize: {x}, {y}");
    let max = TOUCHPAD_2024_X_MAX;
    let x = normalize_unsigned_value(x as f64, max);

    let max = TOUCHPAD_2024_Y_MAX;
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
fn normalize_trigger_value(event: &event::TriggerEvent) -> InputValue {
    match event {
        event::TriggerEvent::PadForce(value) => InputValue::Float(normalize_unsigned_value(
            value.value as f64,
            TOUCHPAD_2024_PAD_FORCE_MAX,
        )),
    }
}

/// Translate the given touchpad events into native events
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    events.into_iter().filter_map(translate_event).collect()
}

/// Translate the given touchpad event into a native event
fn translate_event(event: event::Event) -> Option<NativeEvent> {
    // The touchpad axis and force trigger use per-model max values, so they
    // are not covered by the shared to_value
    if let event::Event::TouchAxis(axis) = event {
        return Some(NativeEvent::new(
            Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
            normalize_axis_value(&axis),
        ));
    }
    if let event::Event::Trigger(trigger) = event {
        return Some(NativeEvent::new(
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)),
            normalize_trigger_value(&trigger),
        ));
    }
    let value = event.to_value()?;
    Some(NativeEvent::new(event.to_capability(), value))
}

/// List of all input capabilities that the GPD Win Mini (2024+) touchpad
/// driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
];
