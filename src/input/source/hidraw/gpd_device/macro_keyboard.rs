use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Debug,
};

use crate::{
    config::capability_map::{exposed_capabilities, translate_capabilities},
    drivers::gpd_device::{event, macro_keyboard_driver::MacroKeyboardDriver},
    input::{
        capability::{Capability, Gamepad, GamepadButton},
        event::native::NativeEvent,
        source::{EventCapability, InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// GPD Win Mini macro keyboard source device implementation
pub struct GpdMacroKeyboard {
    driver: MacroKeyboardDriver,
    capability_map: HashMap<Capability, Capability>,
}

impl GpdMacroKeyboard {
    pub fn new(
        device_info: UdevDevice,
        capability_map: HashMap<Capability, Capability>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = MacroKeyboardDriver::new(device_info)?;
        Ok(Self {
            driver,
            capability_map,
        })
    }
}

impl SourceInputDevice for GpdMacroKeyboard {
    fn translation_targets(&self) -> Vec<Capability> {
        self.capability_map.values().cloned().collect()
    }

    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = translate_events(events);
        Ok(translate_capabilities(native_events, &self.capability_map))
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
        Ok(exposed_capabilities(
            CAPABILITIES.into(),
            &self.capability_map,
        ))
    }
}

impl SourceOutputDevice for GpdMacroKeyboard {}

impl Debug for GpdMacroKeyboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpdWinMiniMacroKeyboard").finish()
    }
}

/// Translate the given events into native events
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    events.into_iter().filter_map(translate_event).collect()
}

/// Translate the given event into a native event
fn translate_event(event: event::Event) -> Option<NativeEvent> {
    let value = event.to_value()?;
    Some(NativeEvent::new(event.to_capability(), value))
}

/// List of all input capabilities that the GPD Win Mini macro keyboard driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
];
