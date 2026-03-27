use std::{error::Error, fmt::Debug};

use crate::{
    drivers::oxp_hid::{driver::Driver, event},
    input::{
        capability::{Capability, Gamepad, GamepadButton},
        event::{native::NativeEvent, value::InputValue},
        output_capability::OutputCapability,
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// OXP HID source device implementation.
/// Reports extra buttons (M1/M2/Keyboard/Guide) from vendor HID report mode.
pub struct OxpHid {
    driver: Driver,
}

impl OxpHid {
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info.devnode())?;
        Ok(Self { driver })
    }
}

impl SourceInputDevice for OxpHid {
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = translate_events(events);
        for ev in &native_events {
            log::info!("OXP HID → NativeEvent: {:?} = {:?}", ev.as_capability(), ev.get_value());
        }
        Ok(native_events)
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(CAPABILITIES.into())
    }
}

impl SourceOutputDevice for OxpHid {
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![])
    }

    fn write_event(&mut self, _event: OutputEvent) -> Result<(), OutputError> {
        Ok(())
    }
}

impl Debug for OxpHid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OxpHid").finish()
    }
}

fn translate_events(events: Vec<event::Event>) -> Vec<NativeEvent> {
    events.into_iter().map(translate_event).collect()
}

fn translate_event(event: event::Event) -> NativeEvent {
    match event {
        event::Event::Button(button) => translate_button(button),
    }
}

fn translate_button(button: event::ButtonEvent) -> NativeEvent {
    let (capability, pressed) = match button {
        event::ButtonEvent::M1(v) => (GamepadButton::LeftPaddle1, v.pressed),
        event::ButtonEvent::M2(v) => (GamepadButton::RightPaddle1, v.pressed),
        event::ButtonEvent::Keyboard(v) => (GamepadButton::Keyboard, v.pressed),
        event::ButtonEvent::Guide(v) => (GamepadButton::Guide, v.pressed),
    };
    NativeEvent::new(
        Capability::Gamepad(Gamepad::Button(capability)),
        InputValue::Bool(pressed),
    )
}

pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Keyboard)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
];
