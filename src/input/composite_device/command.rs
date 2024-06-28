use std::collections::{HashMap, HashSet};

use tokio::sync::mpsc;

use crate::input::{
    capability::Capability,
    event::{native::NativeEvent, Event},
    manager::SourceDeviceInfo,
    output_event::OutputEvent,
    target::TargetCommand,
};

use super::InterceptMode;

/// CompositeDevice commands define all the different ways to interact with [CompositeDevice]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug, Clone)]
pub enum Command {
    GetName(mpsc::Sender<String>),
    ProcessEvent(String, Event),
    ProcessOutputEvent(OutputEvent),
    GetCapabilities(mpsc::Sender<HashSet<Capability>>),
    GetTargetCapabilities(mpsc::Sender<HashSet<Capability>>),
    SetInterceptMode(InterceptMode),
    GetInterceptMode(mpsc::Sender<InterceptMode>),
    GetSourceDevicePaths(mpsc::Sender<Vec<String>>),
    GetTargetDevicePaths(mpsc::Sender<Vec<String>>),
    GetDBusDevicePaths(mpsc::Sender<Vec<String>>),
    SourceDeviceAdded(SourceDeviceInfo),
    SourceDeviceStopped(String),
    SourceDeviceRemoved(String),
    SetTargetDevices(Vec<String>),
    AttachTargetDevices(HashMap<String, mpsc::Sender<TargetCommand>>),
    GetProfileName(mpsc::Sender<String>),
    LoadProfilePath(String, mpsc::Sender<Result<(), String>>),
    WriteEvent(NativeEvent),
    WriteChordEvent(Vec<NativeEvent>),
    WriteSendEvent(NativeEvent),
    HandleEvent(NativeEvent),
    RemoveRecentEvent(Capability),
    SetInterceptActivation(Vec<Capability>, Capability),
    Stop,
}
