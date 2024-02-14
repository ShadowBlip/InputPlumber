use crate::input::capability::Capability;

use super::evdev::EvdevEvent;

/// InputValue represents different ways to represent a value from an input event.
#[derive(Debug, Clone)]
pub enum InputValue {
    None,
    Bool(bool),
    Int(i32),
    UInt(u32),
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

    /// Returns the value of this event
    pub fn get_value(&self) -> InputValue {
        self.value.clone()
    }

    /// Returns whether or not the event is "pressed"
    pub fn pressed(&self) -> bool {
        match self.value {
            InputValue::None => false,
            InputValue::Bool(value) => value,
            InputValue::Int(value) => value != 0,
            InputValue::UInt(value) => value != 0,
            InputValue::Float(value) => value != 0.0,
            InputValue::Vector2 { x, y } => todo!(),
            InputValue::Vector3 { x, y, z } => todo!(),
        }
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
