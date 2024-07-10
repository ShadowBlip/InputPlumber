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
    GetSampleRate(String, Sender<Result<f64, Box<dyn Error + Send + Sync>>>),
    GetSampleRatesAvail(
        String,
        Sender<Result<Vec<f64>, Box<dyn Error + Send + Sync>>>,
    ),
    SetSampleRate(
        String,
        f64,
        Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    ),
    GetScale(String, Sender<Result<f64, Box<dyn Error + Send + Sync>>>),
    GetScalesAvail(
        String,
        Sender<Result<Vec<f64>, Box<dyn Error + Send + Sync>>>,
    ),
    SetScale(
        String,
        f64,
        Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    ),
    Stop,
}
