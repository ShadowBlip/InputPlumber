use super::{capability::InputCapability, value::Value};

/// Events that can be emitted by the Unified Controller
#[derive(Clone, Debug, Default)]
pub struct Event {
    #[allow(dead_code)]
    pub capability: InputCapability,
    #[allow(dead_code)]
    pub value: Value,
}
