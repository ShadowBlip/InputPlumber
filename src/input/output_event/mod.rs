use std::sync::mpsc::Sender;

use ::evdev::{FFEffectData, InputEvent};

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
}

#[derive(Debug, Clone)]
pub enum UinputOutputEvent {
    /// Effect data to upload to a source device and a channel to send back
    /// the effect ID.
    FFUpload(i16, FFEffectData, Sender<Option<i16>>),
    /// Effect id to erase
    FFErase(u32),
}
