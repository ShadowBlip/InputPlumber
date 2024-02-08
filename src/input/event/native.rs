use crate::input::capability::{Capability, Gamepad, GamepadButton};

use super::{evdev::EvdevEvent, MappableEvent};

/// A native event represents an InputPlumber event
#[derive(Debug, Clone)]
pub struct NativeEvent {
    capability: Capability,
    kind: u32,
    code: u32,
    value: u32,
}

impl NativeEvent {
    pub fn new() -> NativeEvent {
        NativeEvent {
            capability: Capability::None,
            code: 0,
            value: 0,
            kind: 0,
        }
    }

    /// Returns the capability that this event implements
    pub fn as_capability(&self) -> Capability {
        self.capability.clone()
    }
}

impl From<EvdevEvent> for NativeEvent {
    fn from(item: EvdevEvent) -> Self {
        NativeEvent {
            capability: item.as_capability(),
            kind: 0,
            code: 0,
            value: item.get_value() as u32,
        }
    }
}

impl MappableEvent for NativeEvent {
    fn matches<T>(&self, event: T) -> bool
    where
        T: MappableEvent,
    {
        match event.kind() {
            super::Event::Native(event) => self.code == event.code && self.kind == event.kind,
            _ => false,
        }
    }

    fn set_value(&mut self, value: f64) {
        self.value = value.round() as u32;
    }

    fn get_value(&self) -> f64 {
        self.value as f64
    }

    fn get_signature(&self) -> String {
        todo!()
    }

    fn kind(&self) -> super::Event {
        super::Event::Native(self.clone())
    }
}
