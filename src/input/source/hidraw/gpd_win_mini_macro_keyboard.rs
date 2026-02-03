use std::{error::Error, fmt::Debug};

use crate::{
    drivers::gpd_win_mini::{
        event, macro_keyboard_driver::MacroKeyboardDriver
    },
    input::{
        capability::{Capability, Gamepad, GamepadButton},
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice}
    },
    udev::device::UdevDevice,
};

/// GPD Win Mini macro keyboard source device implementation
pub struct GpdWinMiniMacroKeyboard {
    driver: MacroKeyboardDriver,
}

impl GpdWinMiniMacroKeyboard {
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = MacroKeyboardDriver::new(device_info)?;
        Ok(Self { driver })
    }
}

impl SourceInputDevice for GpdWinMiniMacroKeyboard {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = translate_events(events);
        Ok(native_events)
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for GpdWinMiniMacroKeyboard {}

impl Debug for GpdWinMiniMacroKeyboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpdWinMiniMacroKeyboard").finish()
    }
}

/// Translate the given events into native events
fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    let mut translated = Vec::with_capacity(events.len());
    for event in events.into_iter() {
        translated.push(translate_event(event));
    }
    translated
}

/// Translate the given event into a native event
fn translate_event(event: event::Event) -> NativeEvent {
    match event {
        event::Event::GamepadButton(button) => match button {
            event::GamepadButtonEvent::L4(value) => {
                NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
                    InputValue::Bool(value.pressed)
                )
            },
            event::GamepadButtonEvent::R4(value) => {
                NativeEvent::new(
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
                    InputValue::Bool(value.pressed)
                )
            },
        },
        _ => NativeEvent::new(Capability::NotImplemented, InputValue::None),
    }
}

/// List of all input capabilities that the GPD Win Mini macro keyboard driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
];
