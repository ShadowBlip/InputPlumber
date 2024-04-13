use std::sync::mpsc::Sender;

use ::evdev::{FFEffectData, InputEvent};

use crate::drivers::dualsense::hid_report::SetStatePackedOutputData;

use super::output_capability::OutputCapability;

/// Output events are events that flow from target devices back to source devices
#[derive(Debug, Clone)]
pub enum OutputEvent {
    Evdev(InputEvent),
    Uinput(UinputOutputEvent),
    DualSense(SetStatePackedOutputData),
}

impl OutputEvent {
    /// Returns the capability of the output event
    fn as_capability(&self) -> OutputCapability {
        match self {
            OutputEvent::Evdev(event) => match event.destructure() {
                evdev::EventSummary::Synchronization(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::Key(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::RelativeAxis(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::AbsoluteAxis(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::Misc(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::Switch(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::Led(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::Sound(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::Repeat(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::ForceFeedback(_, _, _) => OutputCapability::ForceFeedback,
                evdev::EventSummary::Power(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::ForceFeedbackStatus(_, _, _) => {
                    OutputCapability::NotImplemented
                }
                evdev::EventSummary::UInput(_, _, _) => OutputCapability::NotImplemented,
                evdev::EventSummary::Other(_, _, _) => OutputCapability::NotImplemented,
            },
            OutputEvent::Uinput(uinput) => match uinput {
                UinputOutputEvent::FFUpload(_, _) => OutputCapability::ForceFeedbackUpload,
                UinputOutputEvent::FFErase(_) => OutputCapability::ForceFeedbackErase,
            },
            OutputEvent::DualSense(report) => {
                if report.use_rumble_not_haptics {
                    OutputCapability::ForceFeedback
                } else {
                    OutputCapability::NotImplemented
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum UinputOutputEvent {
    /// Effect data to upload to a source device and a channel to send back
    /// the effect ID.
    FFUpload(FFEffectData, Sender<Option<i16>>),
    /// Effect id to erase
    FFErase(u32),
}
