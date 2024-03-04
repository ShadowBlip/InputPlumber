use std::{error::Error, sync::mpsc::Sender};

use ::evdev::FFEffectData;

use super::output_event::OutputEvent;

pub mod evdev;
pub mod hidraw;
pub mod iio;

/// A [SourceDevice] is any physical input device that emits input events
#[derive(Debug)]
pub enum SourceDevice {
    EventDevice(evdev::EventDevice),
    HIDRawDevice(hidraw::HIDRawDevice),
    IIODevice(iio::IIODevice),
}

/// A [SourceCommand] is a message that can be sent to a [SourceDevice] over
/// a channel.
#[derive(Debug, Clone)]
pub enum SourceCommand {
    WriteEvent(OutputEvent),
    UploadEffect(
        FFEffectData,
        Sender<Result<i16, Box<dyn Error + Send + Sync>>>,
    ),
    EraseEffect(i16, Sender<Result<(), Box<dyn Error + Send + Sync>>>),
    Stop,
}
