use packed_struct::types::SizedInteger;

use crate::{
    drivers::steam_deck::hid_report::PackedRumbleReport, input::output_capability::OutputCapability,
};

use super::native::NativeOutputEvent;

#[derive(Debug, Clone)]
pub enum OutputValue {
    ForceFeedback(ForceFeedbackValue),
    Led,
}

#[derive(Debug, Clone)]
pub enum ForceFeedbackValue {
    Magnitude { strong: f64, weak: f64 },
    SuperSpecialMegaRumble { x: f64, y: f64, z: f64 },
}

/// Deck -> Native
impl From<PackedRumbleReport> for NativeOutputEvent {
    fn from(value: PackedRumbleReport) -> Self {
        let capability = OutputCapability::ForceFeedback;
        let value = OutputValue::ForceFeedback(ForceFeedbackValue::Magnitude {
            strong: value.left_speed.to_primitive() as f64,
            weak: value.right_speed.to_primitive() as f64,
        });

        Self::new(capability, value)
    }
}

/// Native -> Deck
impl From<NativeOutputEvent> for PackedRumbleReport {
    fn from(event: NativeOutputEvent) -> Self {
        // TODO: this conversion
        let OutputValue::ForceFeedback(value) = event.get_value() else {
            // NOTE: UNABLE TO TRANSLATE!
            return Self::default();
        };

        match value {
            ForceFeedbackValue::Magnitude { strong, weak } => todo!(),
            ForceFeedbackValue::SuperSpecialMegaRumble { x, y, z } => todo!(),
        }

        Self {
            intensity: 0,
            left_speed: Default::default(),
            right_speed: Default::default(),
            ..Default::default()
        }
    }
}
