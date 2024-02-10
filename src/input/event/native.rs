use crate::input::capability::{Capability, Gamepad, GamepadButton};

use super::{evdev::EvdevEvent, MappableEvent};

/// InputValue represents different ways to represent a value from an input event.
#[derive(Debug, Clone)]
pub enum InputValue {
    None,
    Bool(bool),
    Int(i32),
    Float(f64),
    Vector2 { x: f64, y: f64 },
    Vector3 { x: f64, y: f64, z: f64 },
}

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
}

impl From<EvdevEvent> for NativeEvent {
    /// Convert the [EvdevEvent] into a [NativeEvent]
    fn from(item: EvdevEvent) -> Self {
        let value = item.get_value();
        let normal_value = item.get_normalized_value();
        log::trace!("Normalized value from {} to {}", value, normal_value);
        let input_value = InputValue::Float(normal_value);
        NativeEvent {
            capability: item.as_capability(),
            value: input_value,
        }
    }
}

impl MappableEvent for NativeEvent {
    fn matches<T>(&self, event: T) -> bool
    where
        T: MappableEvent,
    {
        match event.kind() {
            _ => false,
        }
    }

    fn set_value(&mut self, value: f64) {
        self.value = InputValue::Float(value);
    }

    fn get_value(&self) -> f64 {
        match self.value {
            InputValue::Int(v) => v as f64,
            InputValue::Float(v) => v,
            _ => 0.0,
        }
    }

    fn get_signature(&self) -> String {
        todo!()
    }

    fn kind(&self) -> super::Event {
        super::Event::Native(self.clone())
    }
}
