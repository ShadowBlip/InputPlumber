use std::{error::Error, fmt::Debug};

use crate::{
    drivers::fts3528::{
        self,
        driver::Driver,
        event::TouchAxisInput,
        hid_report::{TOUCHSCREEN_X_MAX, TOUCHSCREEN_Y_MAX},
    },
    input::{
        capability::{Capability, Touch},
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// FTS3528 Touchscreen source device implementation
pub struct Fts3528Touchscreen {
    driver: Driver,
}

impl Fts3528Touchscreen {
    /// Create a new FTS3528 touchscreen source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info.devnode())?;
        Ok(Self { driver })
    }
}

impl SourceInputDevice for Fts3528Touchscreen {
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

impl SourceOutputDevice for Fts3528Touchscreen {}

impl Debug for Fts3528Touchscreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fts3528Touchscreen").finish()
    }
}

// Returns a value between 0.0 and 1.0 based on the given value with its
// maximum.
fn normalize_unsigned_value(raw_value: u16, max: u16) -> f64 {
    raw_value as f64 / max as f64
}

/// Normalizes the given input into an input value
fn normalize_axis_value(touch: TouchAxisInput) -> InputValue {
    // Normalize the x, y values if touching
    let (x, y) = match touch.is_touching {
        true => {
            // NOTE: X and Y are flipped due to panel rotation.
            let x = normalize_unsigned_value(touch.y, TOUCHSCREEN_Y_MAX);
            let y = 1.0 - normalize_unsigned_value(touch.x, TOUCHSCREEN_X_MAX);
            (Some(x), Some(y))
        }
        false => (None, None),
    };

    InputValue::Touch {
        index: touch.index,
        is_touching: touch.is_touching,
        pressure: Some(1.0),
        x,
        y,
    }
}

/// Translate the given touchscreen events into native events
fn translate_events(events: Vec<fts3528::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

/// Translate the given touchscreen event into a native event
fn translate_event(event: fts3528::event::Event) -> NativeEvent {
    match event {
        fts3528::event::Event::Touch(touch) => NativeEvent::new(
            Capability::Touchscreen(Touch::Motion),
            normalize_axis_value(touch),
        ),
    }
}

/// List of all capabilities that the Touchscreen driver implements
pub const CAPABILITIES: &[Capability] = &[Capability::Touchscreen(Touch::Motion)];
