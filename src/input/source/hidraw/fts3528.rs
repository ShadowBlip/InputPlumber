use std::{collections::HashSet, error::Error, fmt::Debug};

use crate::{
    drivers::fts3528::{
        self,
        driver::Driver,
        event::TouchAxisInput,
        hid_report::{TOUCHSCREEN_X_MAX, TOUCHSCREEN_Y_MAX},
    },
    input::{
        capability::{Capability, Touch},
        event::{native::NativeEvent, value::normalize_unsigned_value, value::InputValue},
        source::{EventCapability, InputError, SourceInputDevice, SourceOutputDevice},
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

    fn update_event_filter(&mut self, events: HashSet<Capability>) -> Result<(), InputError> {
        let filtered = events
            .iter()
            .filter_map(EventCapability::from_capability)
            .collect();
        self.driver.update_filtered_events(filtered);
        Ok(())
    }
}

impl SourceOutputDevice for Fts3528Touchscreen {}

impl Debug for Fts3528Touchscreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fts3528Touchscreen").finish()
    }
}

/// Normalizes the given input into an input value
fn normalize_axis_value(touch: &TouchAxisInput) -> InputValue {
    // Normalize the x, y values if touching
    let (x, y) = match touch.is_touching {
        true => {
            // NOTE: X and Y are flipped due to panel rotation.
            let x = normalize_unsigned_value(touch.y as f64, TOUCHSCREEN_Y_MAX as f64);
            let y = 1.0 - normalize_unsigned_value(touch.x as f64, TOUCHSCREEN_X_MAX as f64);
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
    events.into_iter().filter_map(translate_event).collect()
}

/// Translate the given touchscreen event into a native event
fn translate_event(event: fts3528::event::Event) -> Option<NativeEvent> {
    let value = event.to_value()?;
    Some(NativeEvent::new(event.to_capability(), value))
}

impl EventCapability for fts3528::event::Event {
    fn to_value(&self) -> Option<InputValue> {
        match self {
            fts3528::event::Event::Touch(touch) => Some(normalize_axis_value(touch)),
        }
    }

    fn to_capability(&self) -> Capability {
        match self {
            fts3528::event::Event::Touch(_) => Capability::Touchscreen(Touch::Motion),
        }
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        match capability {
            Capability::Touchscreen(Touch::Motion) => {
                Some(fts3528::event::Event::Touch(Default::default()))
            }
            _ => None,
        }
    }
}

/// List of all capabilities that the Touchscreen driver implements
pub const CAPABILITIES: &[Capability] = &[Capability::Touchscreen(Touch::Motion)];
