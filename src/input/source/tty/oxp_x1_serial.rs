use crate::{
    drivers::oxp_tty::{
        driver::Driver,
        event::{self},
        OxpDriverType,
    },
    input::{
        capability::{Capability, Gamepad, GamepadButton},
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};
use std::{error::Error, fmt::Debug};

/// [OxpX1Serial] source device implementation
pub struct OxpX1Serial {
    driver: Driver,
}

impl OxpX1Serial {
    /// Create a new [OxpX1Serial] source device with the given udev
    /// device information
    pub fn new(
        device: UdevDevice,
        driver_type: OxpDriverType,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device.devnode().as_str(), driver_type)?;
        Ok(Self { driver })
    }

    /// Translate the given [Driver] events into native events
    fn translate_events(&mut self, events: Vec<event::Event>) -> Vec<NativeEvent> {
        //events.into_iter().map(translate_event).collect()
        let mut new_events = Vec::new();

        for event in events {
            let new_event = self.translate_event(event);
            new_events.push(new_event);
        }

        new_events
    }

    /// Translate the given [Driver] event into a native event
    fn translate_event(&mut self, event: event::Event) -> NativeEvent {
        match event {
            event::Event::GamepadButton(button) => match button {
                event::GamepadButtonEvent::Keyboard(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Keyboard)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::M1(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTop)),
                    InputValue::Bool(value.pressed),
                ),
                event::GamepadButtonEvent::M2(value) => NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTop)),
                    InputValue::Bool(value.pressed),
                ),
                _ => NativeEvent::new(Capability::None, InputValue::None),
            },
            _ => NativeEvent::new(Capability::None, InputValue::None),
        }
    }
}

impl Debug for OxpX1Serial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OxpX1Serial").finish()
    }
}

impl SourceInputDevice for OxpX1Serial {
    fn poll(&mut self) -> Result<Vec<crate::input::event::native::NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = self.translate_events(events);
        Ok(native_events)
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for OxpX1Serial {}

/// List of all capabilities that the [OxpX1Serial] implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Button(GamepadButton::Keyboard)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTop)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTop)),
];
