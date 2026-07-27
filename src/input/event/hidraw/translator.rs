use std::collections::HashMap;

use thiserror::Error;

use crate::{
    config::capability_map::{
        hidraw::{Endianness, HidrawConfig, ValueType},
        CapabilityMapConfigV2,
    },
    input::{
        capability::Capability,
        event::{
            native::NativeEvent,
            value::{normalize_signed_value, normalize_unsigned_value, InputValue},
        },
    },
};

#[derive(Error, Debug, Clone)]
pub enum DecodeError {
    #[error("Read zero bytes from input report")]
    EmptyInputReport,
    #[error("Input report id {0} does not match expected report id: {1}")]
    UnexpectedReportId(u8, u8),
    #[error("Tried to read byte {0} from input report, but report is only {1} bytes")]
    StartByteExceedsReportSize(usize, usize),
    #[error("Tried to read a {0} sized value from byte {1} in input report, but report is only {2} bytes")]
    ValueExceedsReportSize(usize, usize, usize),
}

/// Used to translate hidraw input reports into native inputplumber events using a
/// capability map.
#[derive(Debug)]
pub struct HidrawEventTranslator {
    mappings: Vec<(Capability, HidrawConfig)>,
    last_state: HashMap<u8, Vec<u8>>,
}

impl HidrawEventTranslator {
    pub fn new(capability_map: &CapabilityMapConfigV2) -> Self {
        // Build a list of hidraw mappings
        let mut mappings = vec![];
        for mapping in capability_map.mapping.iter() {
            for source_event in mapping.source_events.iter() {
                let Some(hidraw_mapping) = source_event.hidraw.as_ref() else {
                    continue;
                };
                if hidraw_mapping.bit_offset.is_some()
                    && !matches!(hidraw_mapping.value_type, ValueType::Bool)
                {
                    log::warn!(
                        "bit_offset only applies to bool value types, but mapping '{}' uses value_type {:?}",
                        mapping.name,
                        hidraw_mapping.value_type
                    );
                }
                let capability: Capability = mapping.target_event.clone().into();
                mappings.push((capability, hidraw_mapping.clone()));
            }
        }

        Self {
            mappings,
            last_state: HashMap::new(),
        }
    }

    /// Translates hidraw input reports into native inputplumber events.
    pub fn translate(&mut self, report: &[u8]) -> Vec<NativeEvent> {
        // Key the last state by report id (the first byte of the report; reports
        // with no report id map to 0) so that devices emitting multiple report
        // types never compare values across different report layouts.
        let report_id = report.first().copied().unwrap_or(0);
        let last_state = self.last_state.get(&report_id).cloned();

        // We should only emit events on state change. If no last state exists
        // for this report id, then wait until the next translation cycle.
        let Some(last_state) = last_state else {
            self.last_state.insert(report_id, report.to_vec());
            return vec![];
        };

        // Decode the input report according to the mappings
        let mut events = vec![];
        for (target_capability, mapping) in self.mappings.iter() {
            let value = match Self::decode_value(report, mapping) {
                Ok(value) => value,
                Err(e) => {
                    if matches!(
                        e,
                        DecodeError::EmptyInputReport | DecodeError::UnexpectedReportId(..)
                    ) {
                        log::trace!("{e}");
                    } else {
                        log::warn!("{e}");
                    }
                    continue;
                }
            };
            let Ok(last_value) = Self::decode_value(&last_state, mapping) else {
                continue;
            };

            // Only emit events on state change
            if value == last_value {
                continue;
            }

            let event = NativeEvent::new(target_capability.clone(), value);
            events.push(event);
        }

        // Keep a copy of the last state per report id to determine if an
        // event needs to be emitted.
        self.last_state.insert(report_id, report.to_vec());

        events
    }

    /// Return the decoded value for the given input report and mapping
    fn decode_value(report: &[u8], mapping: &HidrawConfig) -> Result<InputValue, DecodeError> {
        // Check if the input report id matches
        if let Some(expected_report_id) = mapping.report_id {
            let Some(report_id) = report.first() else {
                return Err(DecodeError::EmptyInputReport);
            };
            if *report_id != expected_report_id {
                return Err(DecodeError::UnexpectedReportId(
                    *report_id,
                    expected_report_id,
                ));
            }
        }

        // Ensure that the input report is in range of the value
        if mapping.byte_start >= report.len() {
            return Err(DecodeError::StartByteExceedsReportSize(
                mapping.byte_start,
                report.len(),
            ));
        }

        // Translate the event based on the value type
        match &mapping.value_type {
            ValueType::Bool => {
                let value = Self::decode_bool(report, mapping);
                Ok(InputValue::Bool(value))
            }
            value_type => Self::decode_typed(report, value_type, mapping),
        }
    }

    /// Decode a value according to the given integer value type.
    fn decode_typed(
        report: &[u8],
        value_type: &ValueType,
        mapping: &HidrawConfig,
    ) -> Result<InputValue, DecodeError> {
        let value = Self::decode_int(
            report,
            value_type,
            mapping.byte_start,
            mapping.min_value,
            mapping.max_value,
            mapping.endian.as_ref(),
        )?;
        Ok(match value_type {
            ValueType::UInt8
            | ValueType::Int8
            | ValueType::UInt16
            | ValueType::Int16
            | ValueType::UInt32
            | ValueType::Int32 => InputValue::Float(value[0]),
            ValueType::UInt8Vector2
            | ValueType::UInt16Vector2
            | ValueType::UInt32Vector2
            | ValueType::Int8Vector2
            | ValueType::Int16Vector2
            | ValueType::Int32Vector2 => InputValue::Vector2 {
                x: Some(value[0]),
                y: Some(value[1]),
            },
            ValueType::UInt8Vector3
            | ValueType::UInt16Vector3
            | ValueType::UInt32Vector3
            | ValueType::Int8Vector3
            | ValueType::Int16Vector3
            | ValueType::Int32Vector3 => InputValue::Vector3 {
                x: Some(value[0]),
                y: Some(value[1]),
                z: Some(value[2]),
            },
            ValueType::Bool => unreachable!("bool values are decoded by [decode_bool]"),
        })
    }

    /// Decode the integer(s) for the given value type, returning normalized
    /// values (one per axis for vector types).
    fn decode_int(
        report: &[u8],
        value_type: &ValueType,
        byte_start: usize,
        min_value: Option<i64>,
        max_value: Option<i64>,
        endian: Option<&Endianness>,
    ) -> Result<Vec<f64>, DecodeError> {
        let is_signed = value_type.is_signed();
        let component_size = value_type.component_size();
        let dimensions = value_type.dimensions();

        // Ensure the value(s) don't exceed the report size
        if byte_start + component_size * dimensions > report.len() {
            return Err(DecodeError::ValueExceedsReportSize(
                component_size * dimensions,
                byte_start,
                report.len(),
            ));
        }

        let mut values = vec![0.0; dimensions];
        for (i, value) in values.iter_mut().enumerate() {
            let start = byte_start + i * component_size;
            let raw_value = match component_size {
                1 => {
                    let byte_value = report[start];
                    if is_signed {
                        byte_value.cast_signed() as i64
                    } else {
                        byte_value as i64
                    }
                }
                2 => {
                    let bytes: [u8; 2] = report[start..start + 2].try_into().unwrap();
                    if is_signed {
                        match endian {
                            Some(Endianness::Msb) => i16::from_be_bytes(bytes) as i64,
                            _ => i16::from_le_bytes(bytes) as i64,
                        }
                    } else {
                        match endian {
                            Some(Endianness::Msb) => u16::from_be_bytes(bytes) as i64,
                            _ => u16::from_le_bytes(bytes) as i64,
                        }
                    }
                }
                4 => {
                    let bytes: [u8; 4] = report[start..start + 4].try_into().unwrap();
                    if is_signed {
                        match endian {
                            Some(Endianness::Msb) => i32::from_be_bytes(bytes) as i64,
                            _ => i32::from_le_bytes(bytes) as i64,
                        }
                    } else {
                        match endian {
                            Some(Endianness::Msb) => u32::from_be_bytes(bytes) as i64,
                            _ => u32::from_le_bytes(bytes) as i64,
                        }
                    }
                }
                _ => unreachable!(),
            };

            // Normalize the value
            let (min, max) = if is_signed {
                match component_size {
                    1 => (
                        min_value.unwrap_or(i8::MIN as i64) as f64,
                        max_value.unwrap_or(i8::MAX as i64) as f64,
                    ),
                    2 => (
                        min_value.unwrap_or(i16::MIN as i64) as f64,
                        max_value.unwrap_or(i16::MAX as i64) as f64,
                    ),
                    _ => (
                        min_value.unwrap_or(i32::MIN as i64) as f64,
                        max_value.unwrap_or(i32::MAX as i64) as f64,
                    ),
                }
            } else {
                match component_size {
                    1 => (0.0, max_value.unwrap_or(u8::MAX as i64) as f64),
                    2 => (0.0, max_value.unwrap_or(u16::MAX as i64) as f64),
                    _ => (0.0, max_value.unwrap_or(u32::MAX as i64) as f64),
                }
            };
            if is_signed {
                *value = normalize_signed_value(raw_value as f64, min, max);
            } else {
                *value = normalize_unsigned_value(raw_value as f64, max);
            }
        }

        Ok(values)
    }

    fn decode_bool(report: &[u8], mapping: &HidrawConfig) -> bool {
        let byte_value = report[mapping.byte_start];
        if let Some(bit_offset) = mapping.bit_offset {
            (byte_value & (1 << bit_offset)) != 0
        } else {
            byte_value != 0
        }
    }
}
