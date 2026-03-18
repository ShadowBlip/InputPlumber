use std::{error::Error, fmt::Debug};

use crate::{
    drivers::gpd_win5::{driver::GpdWin5ButtonDriver, event},
    input::{
        capability::{Capability, Gamepad, GamepadButton},
        event::{native::NativeEvent, value::InputValue},
        source::{InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// GPD Win 5 hidden buttons source device implementation
pub struct GpdWin5Buttons {
    driver: GpdWin5ButtonDriver,
}

impl GpdWin5Buttons {
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = GpdWin5ButtonDriver::new(device_info)?;
        Ok(Self { driver })
    }
}

impl SourceInputDevice for GpdWin5Buttons {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = translate_events(events);
        Ok(native_events)
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for GpdWin5Buttons {}

impl Debug for GpdWin5Buttons {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpdWin5Buttons").finish()
    }
}

fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

fn translate_event(event: event::Event) -> NativeEvent {
    match event {
        event::Event::GamepadButton(button) => match button {
            event::GamepadButtonEvent::QuickAccess(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::L4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
                InputValue::Bool(value.pressed),
            ),
            event::GamepadButtonEvent::R4(value) => NativeEvent::new(
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
                InputValue::Bool(value.pressed),
            ),
        },
    }
}

pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
];
