use super::{capability::InputCapability, hid_report::ValueType, value::TouchValue};

#[derive(Clone, Debug, Default)]
pub struct StateUpdate {
    pub capability: InputCapability,
    pub value: ValueUpdate,
}

#[derive(Debug, Clone, Default)]
pub enum ValueUpdate {
    #[default]
    None,
    Bool(BoolUpdate),
    UInt8(UInt8Update),
    UInt16(UInt16Update),
    UInt16Vector2(UInt16Vector2Update),
    Int16Vector3(Int16Vector3Update),
    Touch(TouchValue),
}

impl ValueUpdate {
    /// Return the [ValueType] for this [Value]
    pub fn value_type(&self) -> ValueType {
        match self {
            Self::None => ValueType::None,
            Self::Bool(_) => ValueType::Bool,
            Self::UInt8(_) => ValueType::UInt8,
            Self::UInt16(_) => ValueType::UInt16,
            Self::UInt16Vector2(_) => ValueType::UInt16Vector2,
            Self::Int16Vector3(_) => ValueType::Int16Vector3,
            Self::Touch(_) => ValueType::Touch,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct BoolUpdate {
    pub value: bool,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UInt8Update {
    pub value: u8,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UInt16Update {
    pub value: u16,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct UInt16Vector2Update {
    pub x: Option<u16>,
    pub y: Option<u16>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Int16Vector3Update {
    pub x: Option<i16>,
    pub y: Option<i16>,
    pub z: Option<i16>,
}
