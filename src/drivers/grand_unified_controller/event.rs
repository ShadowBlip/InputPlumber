use super::{capability::InputCapability, value::Value};

/// Events that can be emitted by the Unified Controller
#[derive(Clone, Debug, Default)]
pub struct Event {
    pub capability: InputCapability,
    pub value: Value,
}
