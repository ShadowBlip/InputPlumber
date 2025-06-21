use std::collections::{HashMap, HashSet};

use tokio::sync::mpsc;

use crate::{
    config::CompositeDeviceConfig,
    input::{
        capability::Capability,
        event::{native::NativeEvent, Event},
        info::DeviceInfo,
        output_event::OutputEvent,
        target::{client::TargetDeviceClient, TargetDeviceTypeId},
    },
};

use super::InterceptMode;

/// CompositeDevice commands define all the different ways to interact with [CompositeDevice]
/// over a channel. These commands are processed in an asyncronous thread and
/// dispatched as they come in.
#[derive(Debug, Clone)]
pub enum CompositeCommand {
    AttachTargetDevices(HashMap<String, TargetDeviceClient>),
    GetConfig(mpsc::Sender<CompositeDeviceConfig>),
    GetCapabilities(mpsc::Sender<HashSet<Capability>>),
    GetDBusDevicePaths(mpsc::Sender<Vec<String>>),
    GetInterceptMode(mpsc::Sender<InterceptMode>),
    GetName(mpsc::Sender<String>),
    #[allow(dead_code)]
    GetProfileName(mpsc::Sender<String>),
    GetSourceDevicePaths(mpsc::Sender<Vec<String>>),
    GetTargetCapabilities(mpsc::Sender<HashSet<Capability>>),
    GetTargetDevicePaths(mpsc::Sender<Vec<String>>),
    HandleEvent(NativeEvent),
    LoadProfileFromYaml(String, mpsc::Sender<Result<(), String>>),
    LoadProfilePath(String, mpsc::Sender<Result<(), String>>),
    ProcessEvent(String, Event),
    ProcessOutputEvent(OutputEvent),
    RemoveRecentEvent(Capability),
    SetInterceptActivation(Vec<Capability>, Capability),
    SetInterceptMode(InterceptMode),
    SetTargetDevices(Vec<TargetDeviceTypeId>),
    SourceDeviceAdded(DeviceInfo),
    SourceDeviceRemoved(DeviceInfo),
    SourceDeviceStopped(DeviceInfo),
    #[allow(dead_code)]
    UpdateSourceCapabilities(String, HashSet<Capability>),
    UpdateTargetCapabilities(String, HashSet<Capability>),
    WriteChordEvent(Vec<NativeEvent>),
    WriteEvent(NativeEvent),
    WriteSendEvent(NativeEvent),
    Stop,
    Suspend(mpsc::Sender<()>),
    Resume(mpsc::Sender<()>),
    IsSuspended(mpsc::Sender<bool>),
}
