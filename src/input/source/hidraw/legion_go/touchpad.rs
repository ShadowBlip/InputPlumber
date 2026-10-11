use std::collections::HashSet;
use std::{error::Error, fmt::Debug};

use crate::{
    drivers::legion_go::{event, go_touchpad_driver::Driver},
    input::{
        capability::{Capability, Gamepad, GamepadTrigger, Touch, TouchButton, Touchpad},
        event::native::NativeEvent,
        source::{EventCapability, InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Legion Go Controller source device implementation
pub struct LegionGoTouchpad {
    driver: Driver,
}

impl LegionGoTouchpad {
    /// Create a new Legion controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info.devnode())?;
        Ok(Self { driver })
    }

    /// Translate the given Legion Go events into native events
    fn translate_events(&mut self, events: Vec<event::Event>) -> Vec<NativeEvent> {
        let mut new_events = Vec::new();

        for event in events {
            if let Some(new_event) = self.translate_event(event) {
                new_events.push(new_event);
            }
        }

        new_events
    }

    /// Translate the given Legion Go event into a native event
    fn translate_event(&mut self, event: event::Event) -> Option<NativeEvent> {
        let value = event.to_value()?;
        Some(NativeEvent::new(event.to_capability(), value))
    }
}

impl SourceInputDevice for LegionGoTouchpad {
    /// Poll the source device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = self.translate_events(events);
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

impl SourceOutputDevice for LegionGoTouchpad {}

impl Debug for LegionGoTouchpad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegionGoTouchpad").finish()
    }
}

/// List of all capabilities that the Legion Go driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
];
