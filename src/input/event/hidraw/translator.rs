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
    last_state: Option<Vec<u8>>,
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
                let capability: Capability = mapping.target_event.clone().into();
                mappings.push((capability, hidraw_mapping.clone()));
            }
        }

        Self {
            mappings,
            last_state: None,
        }
    }

    /// Translates hidraw input reports into native inputplumber events.
    pub fn translate(&mut self, report: &[u8]) -> Vec<NativeEvent> {
        // We should only emit events on state change. If no last state exists,
        // then wait until the next translation cycle.
        let Some(last_state) = self.last_state.as_ref() else {
            self.last_state = Some(report.to_vec());
            return vec![];
        };

        // Decode the input report according to the mappings
        let mut events = vec![];
        for (target_capability, mapping) in self.mappings.iter() {
            let value = match Self::decode_value(report, mapping) {
                Ok(value) => value,
                Err(e) => match e {
                    DecodeError::EmptyInputReport => {
                        log::trace!("{e}");
                        continue;
                    }
                    DecodeError::UnexpectedReportId(..) => {
                        log::trace!("{e}");
                        continue;
                    }
                    DecodeError::StartByteExceedsReportSize(..) => {
                        log::warn!("{e}");
                        continue;
                    }
                    DecodeError::ValueExceedsReportSize(..) => {
                        log::warn!("{e}");
                        continue;
                    }
                },
            };
            let Ok(last_value) = Self::decode_value(last_state, mapping) else {
                continue;
            };

            // Only emit events on state change
            if value == last_value {
                continue;
            }

            let event = NativeEvent::new(target_capability.clone(), value);
            events.push(event);
        }

        // Keep a copy of the last state to determine if an event needs to be
        // emitted.
        // TODO: What about multiple input reports?
        self.last_state = Some(report.to_vec());

        events
    }

    /// Return the decoded value for the given input report and mapping
    fn decode_value(report: &[u8], mapping: &HidrawConfig) -> Result<InputValue, DecodeError> {
        // Check if the input report id matches
        if let Some(expected_report_id) = mapping.report_id {
            let Some(report_id) = report.first() else {
                return Err(DecodeError::EmptyInputReport);
            };
            if *report_id == expected_report_id {
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
        let value = match mapping.value_type {
            ValueType::Bool => {
                let value = Self::decode_bool(report, mapping);
                InputValue::Bool(value)
            }
            ValueType::UInt8 => {
                let value = Self::decode_u8(report, mapping.byte_start, mapping.max_value);
                InputValue::Float(value)
            }
            ValueType::UInt16 => {
                let value = Self::decode_u16(
                    report,
                    mapping.byte_start,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                InputValue::Float(value)
            }
            ValueType::UInt32 => {
                let value = Self::decode_u32(
                    report,
                    mapping.byte_start,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                InputValue::Float(value)
            }
            ValueType::Int8 => {
                let value = Self::decode_i8(
                    report,
                    mapping.byte_start,
                    mapping.min_value,
                    mapping.max_value,
                );
                InputValue::Float(value)
            }
            ValueType::Int16 => {
                let value = Self::decode_i16(
                    report,
                    mapping.byte_start,
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                InputValue::Float(value)
            }
            ValueType::Int32 => {
                let value = Self::decode_i32(
                    report,
                    mapping.byte_start,
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                InputValue::Float(value)
            }
            ValueType::UInt8Vector2 => {
                const SIZE: usize = 1;
                let value_x = Self::decode_u8(report, mapping.byte_start, mapping.max_value);
                let value_y = Self::decode_u8(report, mapping.byte_start + SIZE, mapping.max_value);

                InputValue::Vector2 {
                    x: Some(value_x),
                    y: Some(value_y),
                }
            }
            ValueType::UInt16Vector2 => {
                const SIZE: usize = (u16::BITS / 8) as usize;
                let value_x = Self::decode_u16(
                    report,
                    mapping.byte_start,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_y = Self::decode_u16(
                    report,
                    mapping.byte_start + SIZE,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;

                InputValue::Vector2 {
                    x: Some(value_x),
                    y: Some(value_y),
                }
            }
            ValueType::UInt32Vector2 => {
                const SIZE: usize = (u32::BITS / 8) as usize;
                let value_x = Self::decode_u32(
                    report,
                    mapping.byte_start,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_y = Self::decode_u16(
                    report,
                    mapping.byte_start + SIZE,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;

                InputValue::Vector2 {
                    x: Some(value_x),
                    y: Some(value_y),
                }
            }
            ValueType::Int8Vector2 => {
                const SIZE: usize = 1;
                let value_x = Self::decode_i8(
                    report,
                    mapping.byte_start,
                    mapping.min_value,
                    mapping.max_value,
                );
                let value_y = Self::decode_i8(
                    report,
                    mapping.byte_start + SIZE,
                    mapping.min_value,
                    mapping.max_value,
                );

                InputValue::Vector2 {
                    x: Some(value_x),
                    y: Some(value_y),
                }
            }
            ValueType::Int16Vector2 => {
                const SIZE: usize = (i16::BITS / 8) as usize;
                let value_x = Self::decode_i16(
                    report,
                    mapping.byte_start,
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_y = Self::decode_i16(
                    report,
                    mapping.byte_start + SIZE,
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;

                InputValue::Vector2 {
                    x: Some(value_x),
                    y: Some(value_y),
                }
            }
            ValueType::Int32Vector2 => {
                const SIZE: usize = (i32::BITS / 8) as usize;
                let value_x = Self::decode_i32(
                    report,
                    mapping.byte_start,
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_y = Self::decode_i32(
                    report,
                    mapping.byte_start + SIZE,
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;

                InputValue::Vector2 {
                    x: Some(value_x),
                    y: Some(value_y),
                }
            }
            ValueType::UInt8Vector3 => {
                const SIZE: usize = 1;
                let value_x = Self::decode_u8(report, mapping.byte_start, mapping.max_value);
                let value_y = Self::decode_u8(report, mapping.byte_start + SIZE, mapping.max_value);
                let value_z =
                    Self::decode_u8(report, mapping.byte_start + (SIZE * 2), mapping.max_value);

                InputValue::Vector3 {
                    x: Some(value_x),
                    y: Some(value_y),
                    z: Some(value_z),
                }
            }
            ValueType::UInt16Vector3 => {
                const SIZE: usize = (u16::BITS / 8) as usize;
                let value_x = Self::decode_u16(
                    report,
                    mapping.byte_start,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_y = Self::decode_u16(
                    report,
                    mapping.byte_start + SIZE,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_z = Self::decode_u16(
                    report,
                    mapping.byte_start + (SIZE * 2),
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;

                InputValue::Vector3 {
                    x: Some(value_x),
                    y: Some(value_y),
                    z: Some(value_z),
                }
            }
            ValueType::UInt32Vector3 => {
                const SIZE: usize = (u32::BITS / 8) as usize;
                let value_x = Self::decode_u32(
                    report,
                    mapping.byte_start,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_y = Self::decode_u32(
                    report,
                    mapping.byte_start + SIZE,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_z = Self::decode_u32(
                    report,
                    mapping.byte_start + (SIZE * 2),
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;

                InputValue::Vector3 {
                    x: Some(value_x),
                    y: Some(value_y),
                    z: Some(value_z),
                }
            }
            ValueType::Int8Vector3 => {
                const SIZE: usize = 1;
                let value_x = Self::decode_i8(
                    report,
                    mapping.byte_start,
                    mapping.min_value,
                    mapping.max_value,
                );
                let value_y = Self::decode_i8(
                    report,
                    mapping.byte_start + SIZE,
                    mapping.min_value,
                    mapping.max_value,
                );
                let value_z = Self::decode_i8(
                    report,
                    mapping.byte_start + (SIZE * 2),
                    mapping.min_value,
                    mapping.max_value,
                );

                InputValue::Vector3 {
                    x: Some(value_x),
                    y: Some(value_y),
                    z: Some(value_z),
                }
            }
            ValueType::Int16Vector3 => {
                const SIZE: usize = (i16::BITS / 8) as usize;
                let value_x = Self::decode_i16(
                    report,
                    mapping.byte_start,
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_y = Self::decode_i16(
                    report,
                    mapping.byte_start + SIZE,
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_z = Self::decode_i16(
                    report,
                    mapping.byte_start + (SIZE * 2),
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;

                InputValue::Vector3 {
                    x: Some(value_x),
                    y: Some(value_y),
                    z: Some(value_z),
                }
            }
            ValueType::Int32Vector3 => {
                const SIZE: usize = (i32::BITS / 8) as usize;
                let value_x = Self::decode_i32(
                    report,
                    mapping.byte_start,
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_y = Self::decode_i32(
                    report,
                    mapping.byte_start + SIZE,
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;
                let value_z = Self::decode_i32(
                    report,
                    mapping.byte_start + (SIZE * 2),
                    mapping.min_value,
                    mapping.max_value,
                    mapping.endian.as_ref(),
                )?;

                InputValue::Vector3 {
                    x: Some(value_x),
                    y: Some(value_y),
                    z: Some(value_z),
                }
            }
        };

        Ok(value)
    }

    fn decode_bool(report: &[u8], mapping: &HidrawConfig) -> bool {
        let byte_value = report[mapping.byte_start];
        if let Some(bit_offset) = mapping.bit_offset {
            (byte_value & (1 << bit_offset)) != 0
        } else {
            byte_value != 0
        }
    }

    fn decode_u8(report: &[u8], byte_start: usize, max_value: Option<i64>) -> f64 {
        let byte_value = report[byte_start] as f64;
        let max = max_value.unwrap_or(u8::MAX as i64) as f64;
        normalize_unsigned_value(byte_value, max)
    }

    fn decode_i8(
        report: &[u8],
        byte_start: usize,
        min_value: Option<i64>,
        max_value: Option<i64>,
    ) -> f64 {
        let byte_value = report[byte_start].cast_signed() as f64;
        let min = min_value.unwrap_or(i8::MIN as i64) as f64;
        let max = max_value.unwrap_or(i8::MAX as i64) as f64;
        normalize_signed_value(byte_value, min, max)
    }

    fn decode_u16(
        report: &[u8],
        byte_start: usize,
        max_value: Option<i64>,
        endian: Option<&Endianness>,
    ) -> Result<f64, DecodeError> {
        // Calculate the byte start and end
        const SIZE: usize = (u16::BITS / 8) as usize;
        let start = byte_start;
        let end = start + (SIZE - 1);

        // Ensure end doesn't exceed report size
        if end >= report.len() {
            return Err(DecodeError::ValueExceedsReportSize(
                SIZE,
                start,
                report.len(),
            ));
        }

        // Copy the bytes from the report to decode based on endianness
        let mut value_bytes = [0u8; SIZE];
        for (i, j) in (start..end).enumerate() {
            value_bytes[i] = report[j];
        }
        let raw_value = match endian {
            Some(Endianness::Lsb) => u16::from_le_bytes(value_bytes),
            Some(Endianness::Msb) => u16::from_be_bytes(value_bytes),
            None => u16::from_le_bytes(value_bytes),
        };

        // Normalize the value
        let max = max_value.unwrap_or(u16::MAX as i64) as f64;
        let value = normalize_unsigned_value(raw_value as f64, max);

        Ok(value)
    }

    fn decode_i16(
        report: &[u8],
        byte_start: usize,
        min_value: Option<i64>,
        max_value: Option<i64>,
        endian: Option<&Endianness>,
    ) -> Result<f64, DecodeError> {
        // Calculate the byte start and end
        const SIZE: usize = (i16::BITS / 8) as usize;
        let start = byte_start;
        let end = start + (SIZE - 1);

        // Ensure end doesn't exceed report size
        if end >= report.len() {
            return Err(DecodeError::ValueExceedsReportSize(
                SIZE,
                start,
                report.len(),
            ));
        }

        // Copy the bytes from the report to decode based on endianness
        let mut value_bytes = [0u8; SIZE];
        for (i, j) in (start..end).enumerate() {
            value_bytes[i] = report[j];
        }
        let raw_value = match endian {
            Some(Endianness::Lsb) => i16::from_le_bytes(value_bytes),
            Some(Endianness::Msb) => i16::from_be_bytes(value_bytes),
            None => i16::from_le_bytes(value_bytes),
        };

        // Normalize the value
        let min = min_value.unwrap_or(i16::MIN as i64) as f64;
        let max = max_value.unwrap_or(i16::MAX as i64) as f64;
        let value = normalize_signed_value(raw_value as f64, min, max);

        Ok(value)
    }

    fn decode_u32(
        report: &[u8],
        byte_start: usize,
        max_value: Option<i64>,
        endian: Option<&Endianness>,
    ) -> Result<f64, DecodeError> {
        // Calculate the byte start and end
        const SIZE: usize = (u32::BITS / 8) as usize;
        let start = byte_start;
        let end = start + (SIZE - 1);

        // Ensure end doesn't exceed report size
        if end >= report.len() {
            return Err(DecodeError::ValueExceedsReportSize(
                SIZE,
                start,
                report.len(),
            ));
        }

        // Copy the bytes from the report to decode based on endianness
        let mut value_bytes = [0u8; SIZE];
        for (i, j) in (start..end).enumerate() {
            value_bytes[i] = report[j];
        }
        let raw_value = match endian {
            Some(Endianness::Lsb) => u32::from_le_bytes(value_bytes),
            Some(Endianness::Msb) => u32::from_be_bytes(value_bytes),
            None => u32::from_le_bytes(value_bytes),
        };

        // Normalize the value
        let max = max_value.unwrap_or(u32::MAX as i64) as f64;
        let value = normalize_unsigned_value(raw_value as f64, max);

        Ok(value)
    }

    fn decode_i32(
        report: &[u8],
        byte_start: usize,
        min_value: Option<i64>,
        max_value: Option<i64>,
        endian: Option<&Endianness>,
    ) -> Result<f64, DecodeError> {
        // Calculate the byte start and end
        const SIZE: usize = (i32::BITS / 8) as usize;
        let start = byte_start;
        let end = start + (SIZE - 1);

        // Ensure end doesn't exceed report size
        if end >= report.len() {
            return Err(DecodeError::ValueExceedsReportSize(
                SIZE,
                start,
                report.len(),
            ));
        }

        // Copy the bytes from the report to decode based on endianness
        let mut value_bytes = [0u8; SIZE];
        for (i, j) in (start..end).enumerate() {
            value_bytes[i] = report[j];
        }
        let raw_value = match endian {
            Some(Endianness::Lsb) => i32::from_le_bytes(value_bytes),
            Some(Endianness::Msb) => i32::from_be_bytes(value_bytes),
            None => i32::from_le_bytes(value_bytes),
        };

        // Normalize the value
        let min = min_value.unwrap_or(i32::MIN as i64) as f64;
        let max = max_value.unwrap_or(i32::MAX as i64) as f64;
        let value = normalize_signed_value(raw_value as f64, min, max);

        Ok(value)
    }
}
