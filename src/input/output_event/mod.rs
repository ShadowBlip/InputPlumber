use std::sync::mpsc::Sender;

use ::evdev::{FFEffectData, InputEvent};

/// Output events are events that flow from target devices back to source devices
#[derive(Debug, Clone)]
pub enum OutputEvent {
    Evdev(InputEvent),
    Uinput(UinputOutputEvent),
}

#[derive(Debug, Clone)]
pub enum UinputOutputEvent {
    /// Effect data to upload to a source device and a channel to send back
    /// the effect ID.
    FFUpload(FFEffectData, Sender<Option<i16>>),
    /// Effect id to erase
    FFErase(u32),
}
