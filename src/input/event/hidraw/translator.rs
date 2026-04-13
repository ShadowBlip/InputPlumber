use crate::{
    config::capability_map::CapabilityMapConfigV2,
    input::{
        capability::Capability,
        event::{native::NativeEvent, value::InputValue},
    },
};

#[derive(Debug, Clone)]
struct HidrawButtonMapping {
    report_id: Option<u8>,
    byte_index: usize,
    detection: DetectionMode,
    capability: Capability,
}

#[derive(Debug, Clone)]
enum DetectionMode {
    NonZero,
    Value(u8),
    /// Bit position (LSB=0)
    Bit(u8),
}

/// Translates raw HID reports into [NativeEvent]s using a capability map.
#[derive(Debug)]
pub struct HidrawEventTranslator {
    source_events: Vec<HidrawButtonMapping>,
    state: Vec<bool>,
}

impl HidrawEventTranslator {
    /// Create a new translator from a V2 capability map.
    pub fn new(capability_map: &CapabilityMapConfigV2) -> Self {
        let mut source_events = Vec::new();

        for mapping in capability_map.mapping.iter() {
            for source in mapping.source_events.iter() {
                let Some(hidraw) = source.hidraw.as_ref() else {
                    continue;
                };

                if hidraw.input_type != "button" {
                    log::warn!(
                        "Unsupported hidraw input_type '{}' in mapping '{}', skipping",
                        hidraw.input_type,
                        mapping.name,
                    );
                    continue;
                }

                let cap: Capability = mapping.target_event.clone().into();
                if cap == Capability::NotImplemented {
                    log::warn!(
                        "Unresolved target capability in mapping '{}', skipping",
                        mapping.name,
                    );
                    continue;
                }

                let detection = if let Some(value) = hidraw.value {
                    DetectionMode::Value(value)
                } else if let Some(bit) = hidraw.bit_offset {
                    DetectionMode::Bit(bit)
                } else {
                    DetectionMode::NonZero
                };

                source_events.push(HidrawButtonMapping {
                    report_id: hidraw.report_id,
                    byte_index: hidraw.byte_start as usize,
                    detection,
                    capability: cap,
                });
            }
        }

        let state = vec![false; source_events.len()];
        Self { source_events, state }
    }

    pub fn has_hid_translation(&self) -> bool {
        !self.source_events.is_empty()
    }

    pub fn capabilities(&self) -> Vec<Capability> {
        self.source_events.iter().map(|m| m.capability.clone()).collect()
    }

    /// Translate a raw HID report into [NativeEvent]s. Only emits events on
    /// state changes.
    pub fn translate(&mut self, report: &[u8]) -> Vec<NativeEvent> {
        let mut events = Vec::new();

        for (idx, mapping) in self.source_events.iter().enumerate() {
            if let Some(expected_id) = mapping.report_id {
                if report.first().copied() != Some(expected_id) {
                    continue;
                }
            }

            if mapping.byte_index >= report.len() {
                log::warn!(
                    "HID report too short for mapping at byte {}: got {} bytes",
                    mapping.byte_index,
                    report.len(),
                );
                continue;
            }

            let byte_val = report[mapping.byte_index];
            let pressed = match mapping.detection {
                DetectionMode::NonZero => byte_val != 0,
                DetectionMode::Value(expected) => byte_val == expected,
                DetectionMode::Bit(bit) => (byte_val & (1 << bit)) != 0,
            };

            if pressed != self.state[idx] {
                self.state[idx] = pressed;
                events.push(NativeEvent::new(
                    mapping.capability.clone(),
                    InputValue::Bool(pressed),
                ));
            }
        }

        events
    }
}
