use crate::input::output_capability::OutputCapability;

use super::value::OutputValue;

#[derive(Debug, Clone)]
pub struct NativeOutputEvent {
    capability: OutputCapability,
    value: OutputValue,
}

impl NativeOutputEvent {
    pub fn new(capability: OutputCapability, value: OutputValue) -> Self {
        Self { capability, value }
    }

    /// Returns the value of this event
    pub fn get_value(&self) -> &OutputValue {
        &self.value
    }
}
