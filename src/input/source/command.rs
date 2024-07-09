use std::{error::Error, sync::mpsc::Sender};

use evdev::FFEffectData;

use crate::input::output_event::OutputEvent;

/// A [SourceCommand] is a message that can be sent to a [SourceDevice] over
/// a channel.
#[derive(Debug, Clone)]
pub enum SourceCommand {
    WriteEvent(OutputEvent),
    UploadEffect(
        FFEffectData,
        Sender<Result<i16, Box<dyn Error + Send + Sync>>>,
    ),
    UpdateEffect(i16, FFEffectData),
    EraseEffect(i16, Sender<Result<(), Box<dyn Error + Send + Sync>>>),
    Stop,
}
