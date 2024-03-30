use crate::input::capability::Capability;

use super::{evdev::EvdevEvent, value::InputValue};

/// A native event represents an InputPlumber event
#[derive(Debug, Clone)]
pub struct NativeEvent {
    capability: Capability,
    value: InputValue,
}

impl NativeEvent {
    pub fn new(capability: Capability, value: InputValue) -> NativeEvent {
        NativeEvent { capability, value }
    }

    /// Returns the capability that this event implements
    pub fn as_capability(&self) -> Capability {
        self.capability.clone()
    }

    /// Returns the value of this event
    pub fn get_value(&self) -> InputValue {
        self.value.clone()
    }

    /// Returns whether or not the event is "pressed"
    pub fn pressed(&self) -> bool {
        self.value.pressed()
    }
}

impl From<EvdevEvent> for NativeEvent {
    /// Convert the [EvdevEvent] into a [NativeEvent]
    fn from(item: EvdevEvent) -> Self {
        let capability = item.as_capability();
        let value = item.get_value();
        NativeEvent { capability, value }
    }
}
