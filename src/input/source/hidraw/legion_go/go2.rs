use std::collections::HashSet;
use std::{error::Error, fmt::Debug};

use crate::{
    drivers::legion_go::{
        event,
        go2_driver::{Driver, LegionGoGen},
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Mouse, Source,
        },
        event::native::NativeEvent,
        source::{EventCapability, InputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// Legion Go Controller source device implementation
pub struct LegionGo2Controller {
    driver: Driver,
}

impl LegionGo2Controller {
    /// Create a new Legion controller source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver = Driver::new(device_info)?;
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

impl SourceInputDevice for LegionGo2Controller {
    /// Poll the source device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = self.translate_events(events);

        Ok(native_events)
    }

    /// Returns the possible input events this device is capable of emitting
    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        if self.driver.gen() == LegionGoGen::Gen2 {
            return Ok(CAPABILITIES.into());
        }
        // The original Legion Go doesn't have these.
        let capabilities = CAPABILITIES
            .iter()
            .filter(|cap| {
                !matches!(
                    cap,
                    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch))
                        | Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess2))
                        | Capability::Gamepad(Gamepad::Button(GamepadButton::Keyboard))
                )
            })
            .cloned()
            .collect();
        Ok(capabilities)
    }

    fn update_event_filter(&mut self, events: HashSet<Capability>) -> Result<(), InputError> {
        let filtered = events
            .iter()
            .filter_map(EventCapability::from_capability)
            .collect();
        self.driver.update_filtered_events(filtered);
        Ok(())
    }

    fn get_default_event_filter(&self) -> Result<HashSet<Capability>, InputError> {
        let filtered_events = self
            .driver
            .get_default_event_filter()
            .map_err(|e| format!("Failed to get default event filter: {:?}", e))?;
        Ok(filtered_events
            .into_iter()
            .map(|e| e.to_capability())
            .collect())
    }
}

impl SourceOutputDevice for LegionGo2Controller {}

impl Debug for LegionGo2Controller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegionController").finish()
    }
}

/// Normalize the value to something between -1.0 and 1.0 based on the
/// minimum and maximum axis ranges.
/// List of all capabilities that the Legion Go driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Accelerometer(Source::Center),
    Capability::Accelerometer(Source::Left),
    Capability::Accelerometer(Source::Right),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
    Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Keyboard)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle3)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
    Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
    Capability::Gyroscope(Source::Center),
    Capability::Gyroscope(Source::Left),
    Capability::Gyroscope(Source::Right),
    Capability::Mouse(Mouse::Wheel),
];
