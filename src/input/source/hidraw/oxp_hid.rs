use std::{collections::HashSet, error::Error, fmt::Debug};

use crate::{
    drivers::oxp_hid::{driver::Driver, event},
    input::{
        capability::{Capability, Gamepad, GamepadButton},
        event::{native::NativeEvent, value::InputValue},
        output_capability::OutputCapability,
        output_event::OutputEvent,
        source::{EventCapability, InputError, OutputError, SourceInputDevice, SourceOutputDevice},
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
            log::debug!(
                "OXP HID → NativeEvent: {:?} = {:?}",
                ev.as_capability(),
                ev.get_value()
            );
        }
        Ok(native_events)
    }

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
    events.into_iter().filter_map(translate_event).collect()
}

fn translate_event(event: event::Event) -> Option<NativeEvent> {
    let value = event.to_value()?;
    Some(NativeEvent::new(event.to_capability(), value))
}

impl EventCapability for event::Event {
    fn to_value(&self) -> Option<InputValue> {
        match self {
            event::Event::Button(button) => Some(InputValue::Bool(button.pressed())),
        }
    }

    fn to_capability(&self) -> Capability {
        match self {
            event::Event::Button(button) => match button {
                event::ButtonEvent::M1(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1))
                }
                event::ButtonEvent::M2(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1))
                }
                event::ButtonEvent::Keyboard(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Keyboard))
                }
                event::ButtonEvent::Guide(_) => {
                    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide))
                }
            },
        }
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        match capability {
            Capability::Gamepad(Gamepad::Button(button)) => {
                let event = match button {
                    GamepadButton::LeftPaddle1 => event::ButtonEvent::M1(Default::default()),
                    GamepadButton::RightPaddle1 => event::ButtonEvent::M2(Default::default()),
                    GamepadButton::Keyboard => event::ButtonEvent::Keyboard(Default::default()),
                    GamepadButton::Guide => event::ButtonEvent::Guide(Default::default()),
                    _ => return None,
                };
                Some(event::Event::Button(event))
            }
            _ => None,
        }
    }
}

pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Keyboard)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
];
