use std::{collections::HashSet, error::Error, fmt::Debug};

use crate::{
    drivers::opineo::{
        driver::Driver, event, LPAD_NAMES, PAD_FORCE_MAX, PAD_X_MAX, PAD_Y_MAX, RPAD_NAMES,
    },
    input::{
        capability::{Capability, Gamepad, GamepadTrigger, Touch, TouchButton, Touchpad},
        event::{
            native::NativeEvent,
            value::{normalize_unsigned_value, InputValue},
        },
        output_capability::OutputCapability,
        source::{EventCapability, InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};

/// The OrangePi Neo has two touchpads; one on the left and one on the right
#[derive(Debug, Clone, Copy)]
enum TouchpadSide {
    Unknown,
    Left,
    Right,
}

/// OrangePi Neo Touchpad source device implementation
pub struct OrangePiNeoTouchpad {
    driver: Driver,
    side: TouchpadSide,
}

impl OrangePiNeoTouchpad {
    /// Create a new OrangePi Neo touchscreen source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // Query the udev module to determine if this is the left or right touchpad.
        let name = device_info.name();
        let touchpad_side = {
            if LPAD_NAMES.contains(&name.as_str()) {
                log::debug!("Detected left pad.");
                TouchpadSide::Left
            } else if RPAD_NAMES.contains(&name.as_str()) {
                log::debug!("Detected right pad.");
                TouchpadSide::Right
            } else {
                log::debug!("Unable to detect pad side.");
                TouchpadSide::Unknown
            }
        };
        let driver = Driver::new(device_info)?;

        Ok(Self {
            driver,
            side: touchpad_side,
        })
    }
}

impl SourceInputDevice for OrangePiNeoTouchpad {
    /// Poll the given input device for input events
    fn poll(&mut self) -> Result<Vec<NativeEvent>, InputError> {
        let events = self.driver.poll()?;
        let native_events = translate_events(events, self.side);
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

impl SourceOutputDevice for OrangePiNeoTouchpad {
    /// Returns the possible output events this device is capable of (e.g. force feedback, LED,
    /// etc.)
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(OUTPUT_CAPABILITIES.into())
    }
}

impl Debug for OrangePiNeoTouchpad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrangePiNeoTouchpad")
            .field("side", &self.side)
            .finish()
    }
}

/// Normalize the value to something between -1.0 and 1.0 based on the Deck's
/// minimum and maximum axis ranges.
fn normalize_axis_value(event: &event::TouchAxisEvent) -> InputValue {
    let x = event.x;
    let y = event.y;
    log::trace!("Got axis to normalize: {x}, {y}");
    let max = PAD_X_MAX;
    let x = normalize_unsigned_value(x as f64, max);

    let max = PAD_Y_MAX;
    let y = normalize_unsigned_value(y as f64, max);

    // If this is an UP event, don't override the position of X/Y
    let (x, y) = if !event.is_touching {
        (None, None)
    } else {
        (Some(x), Some(y))
    };

    log::trace!("Normalized axis: {x:?}, {y:?}");
    InputValue::Touch {
        index: event.index,
        is_touching: event.is_touching,
        pressure: Some(1.0),
        x,
        y,
    }
}

/// Normalize the trigger value to something between 0.0 and 1.0 based on the
/// Orange Pi's maximum axis ranges.
fn normalize_trigger_value(event: &event::TriggerEvent) -> InputValue {
    match event {
        event::TriggerEvent::PadForce(value) => {
            InputValue::Float(normalize_unsigned_value(value.value as f64, PAD_FORCE_MAX))
        }
    }
}

/// Translate the given OrangePi NEO events into native events
fn translate_events(events: Vec<event::Event>, touchpad_side: TouchpadSide) -> Vec<NativeEvent> {
    events
        .into_iter()
        .filter_map(|event| translate_event(event, touchpad_side))
        .collect()
}

/// Translate the given OrangePi NEO event into a native event
fn translate_event(event: event::Event, touchpad_side: TouchpadSide) -> Option<NativeEvent> {
    let value = event.to_value()?;
    // The side of the pad is only known by the source, so the event kind has
    // no single fixed capability
    let cap = match (&event, touchpad_side) {
        (event::Event::TouchAxis(_), TouchpadSide::Left) => {
            Capability::Touchpad(Touchpad::LeftPad(Touch::Motion))
        }
        (event::Event::TouchAxis(_), TouchpadSide::Right) => {
            Capability::Touchpad(Touchpad::RightPad(Touch::Motion))
        }
        (event::Event::TouchButton(_), TouchpadSide::Left) => {
            Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press)))
        }
        (event::Event::TouchButton(_), TouchpadSide::Right) => {
            Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press)))
        }
        (event::Event::Trigger(_), TouchpadSide::Left) => {
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTouchpadForce))
        }
        (event::Event::Trigger(_), TouchpadSide::Right) => {
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce))
        }
        (_, TouchpadSide::Unknown) => return None,
    };
    Some(NativeEvent::new(cap, value))
}

impl EventCapability for event::Event {
    fn to_value(&self) -> Option<InputValue> {
        match self {
            event::Event::TouchAxis(axis) => Some(normalize_axis_value(axis)),
            event::Event::TouchButton(button) => match button {
                event::TouchButtonEvent::Left(value) => Some(InputValue::Bool(value.pressed)),
            },
            event::Event::Trigger(trigger) => Some(normalize_trigger_value(trigger)),
        }
    }

    // The side of the pad is only known by the source, so the event kind has
    // no single fixed capability
    fn to_capability(&self) -> Capability {
        Capability::NotImplemented
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        match capability {
            Capability::Touchpad(Touchpad::LeftPad(Touch::Motion))
            | Capability::Touchpad(Touchpad::RightPad(Touch::Motion)) => {
                Some(event::Event::TouchAxis(Default::default()))
            }
            Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press)))
            | Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))) => Some(
                event::Event::TouchButton(event::TouchButtonEvent::Left(Default::default())),
            ),
            Capability::Gamepad(Gamepad::Trigger(
                GamepadTrigger::LeftTouchpadForce | GamepadTrigger::RightTouchpadForce,
            )) => Some(event::Event::Trigger(event::TriggerEvent::PadForce(
                Default::default(),
            ))),
            _ => None,
        }
    }
}

/// List of all output capabilities that the OrangePi NEO supports
pub const OUTPUT_CAPABILITIES: &[OutputCapability] = &[OutputCapability::ForceFeedback];
/// List of all input capabilities that the OrangePi NEO driver implements
pub const CAPABILITIES: &[Capability] = &[
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTouchpadForce)),
    Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)),
    Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
    Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
    Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
];
