use std::sync::mpsc::Sender;

use ::evdev::{FFEffectData, FFEffectKind, InputEvent};
use packed_struct::types::{Integer, SizedInteger};

use crate::drivers::{
    dualsense::hid_report::SetStatePackedOutputData,
    steam_deck::hid_report::{PackedHapticReport, PackedRumbleReport, PadSide},
};

use super::output_capability::{Haptic, OutputCapability};

/// Output events are events that flow from target devices back to source devices
#[derive(Debug, Clone)]
pub enum OutputEvent {
    Evdev(InputEvent),
    Uinput(UinputOutputEvent),
    DualSense(SetStatePackedOutputData),
    SteamDeckHaptics(PackedHapticReport),
    SteamDeckRumble(PackedRumbleReport),
}

impl OutputEvent {
    /// Returns the capability of the output event
    pub fn as_capability(&self) -> Vec<OutputCapability> {
        match self {
            OutputEvent::Evdev(event) => match event.destructure() {
                evdev::EventSummary::Synchronization(_, _, _) => {
                    vec![OutputCapability::NotImplemented]
                }
                evdev::EventSummary::Key(_, _, _) => vec![OutputCapability::NotImplemented],
                evdev::EventSummary::RelativeAxis(_, _, _) => {
                    vec![OutputCapability::NotImplemented]
                }
                evdev::EventSummary::AbsoluteAxis(_, _, _) => {
                    vec![OutputCapability::NotImplemented]
                }
                evdev::EventSummary::Misc(_, _, _) => vec![OutputCapability::NotImplemented],
                evdev::EventSummary::Switch(_, _, _) => vec![OutputCapability::NotImplemented],
                evdev::EventSummary::Led(_, _, _) => vec![OutputCapability::NotImplemented],
                evdev::EventSummary::Sound(_, _, _) => vec![OutputCapability::NotImplemented],
                evdev::EventSummary::Repeat(_, _, _) => vec![OutputCapability::NotImplemented],
                evdev::EventSummary::ForceFeedback(_, _, _) => {
                    vec![OutputCapability::ForceFeedback]
                }
                evdev::EventSummary::Power(_, _, _) => vec![OutputCapability::NotImplemented],
                evdev::EventSummary::ForceFeedbackStatus(_, _, _) => {
                    vec![OutputCapability::NotImplemented]
                }
                evdev::EventSummary::UInput(_, _, _) => vec![OutputCapability::NotImplemented],
                evdev::EventSummary::Other(_, _, _) => vec![OutputCapability::NotImplemented],
            },
            OutputEvent::Uinput(uinput) => match uinput {
                UinputOutputEvent::FFUpload(_, _, _) => vec![OutputCapability::ForceFeedbackUpload],
                UinputOutputEvent::FFErase(_) => vec![OutputCapability::ForceFeedbackErase],
            },
            OutputEvent::DualSense(report) => {
                if report.use_rumble_not_haptics {
                    vec![OutputCapability::ForceFeedback]
                } else {
                    vec![OutputCapability::NotImplemented]
                }
            }
            OutputEvent::SteamDeckHaptics(packed_haptic_report) => {
                match packed_haptic_report.side {
                    PadSide::Left => vec![OutputCapability::Haptics(Haptic::TrackpadLeft)],
                    PadSide::Right => vec![OutputCapability::Haptics(Haptic::TrackpadRight)],
                    PadSide::Both => vec![
                        OutputCapability::Haptics(Haptic::TrackpadLeft),
                        OutputCapability::Haptics(Haptic::TrackpadRight),
                    ],
                }
            }
            OutputEvent::SteamDeckRumble(_) => vec![OutputCapability::ForceFeedback],
        }
    }

    /// Returns true if the output event is a force feedback/rumble event
    pub fn is_force_feedback(&self) -> bool {
        match self {
            OutputEvent::Evdev(event) => matches!(
                event.destructure(),
                evdev::EventSummary::ForceFeedback(_, _, _)
            ),
            OutputEvent::Uinput(_) => true,
            OutputEvent::DualSense(report) => report.use_rumble_not_haptics,
            OutputEvent::SteamDeckHaptics(_) => true,
            OutputEvent::SteamDeckRumble(_) => true,
        }
    }

    /// Scale the intensity value of the output event. A value of 0.0 is the
    /// minimum intensity. A value of 1.0 is the maximum intensity.
    pub fn scale(&mut self, scale: f64) {
        let scale = scale.clamp(0.0, 1.0);
        match self {
            Self::Evdev(_) => (),
            Self::Uinput(event) => match event {
                UinputOutputEvent::FFUpload(_, data, _) => match data.kind {
                    FFEffectKind::Damper => (),
                    FFEffectKind::Inertia => (),
                    FFEffectKind::Constant {
                        level: _,
                        envelope: _,
                    } => (),
                    FFEffectKind::Ramp {
                        start_level: _,
                        end_level: _,
                        envelope: _,
                    } => (),
                    FFEffectKind::Periodic {
                        waveform: _,
                        period: _,
                        magnitude: _,
                        offset: _,
                        phase: _,
                        envelope: _,
                    } => (),
                    FFEffectKind::Spring { condition: _ } => (),
                    FFEffectKind::Friction { condition: _ } => (),
                    FFEffectKind::Rumble {
                        ref mut strong_magnitude,
                        ref mut weak_magnitude,
                    } => {
                        let scaled_strong = *strong_magnitude as f64 * scale;
                        *strong_magnitude = scaled_strong as u16;
                        let scaled_weak = *weak_magnitude as f64 * scale;
                        *weak_magnitude = scaled_weak as u16;
                    }
                },
                UinputOutputEvent::FFErase(_) => (),
            },
            Self::DualSense(report) => {
                if report.use_rumble_not_haptics {
                    let scaled_left = report.rumble_emulation_left as f64 * scale;
                    report.rumble_emulation_left = scaled_left as u8;
                    let scaled_right = report.rumble_emulation_right as f64 * scale;
                    report.rumble_emulation_right = scaled_right as u8;
                }
            }
            Self::SteamDeckHaptics(data) => {
                data.gain = (data.gain as f64 * scale) as i8;
            }
            Self::SteamDeckRumble(data) => {
                data.intensity = (data.intensity as f64 * scale) as u8;
                let scaled_left_speed = data.left_speed.to_primitive() as f64 * scale;
                data.left_speed = Integer::from_primitive(scaled_left_speed as u16);
                let scaled_right_speed = data.right_speed.to_primitive() as f64 * scale;
                data.right_speed = Integer::from_primitive(scaled_right_speed as u16);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum UinputOutputEvent {
    /// Effect data to upload to a source device and a channel to send back
    /// the effect ID.
    FFUpload(i16, FFEffectData, Sender<Option<i16>>),
    /// Effect id to erase
    FFErase(u32),
}
