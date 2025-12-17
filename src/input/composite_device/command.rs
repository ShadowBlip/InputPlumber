use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use tokio::sync::mpsc;

use crate::{
    config::{CompositeDeviceConfig, DeviceProfile},
    input::{
        capability::Capability,
        event::{native::NativeEvent, Event},
        info::DeviceInfo,
        output_capability::OutputCapability,
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
    GetOutputCapabilities(mpsc::Sender<HashSet<OutputCapability>>),
    GetDBusDevicePaths(mpsc::Sender<Vec<String>>),
    GetInterceptMode(mpsc::Sender<InterceptMode>),
    GetName(mpsc::Sender<String>),
    #[allow(dead_code)]
    GetProfileName(mpsc::Sender<String>),
    GetPersistentId(mpsc::Sender<String>),
    GetSourceDevicePaths(mpsc::Sender<Vec<String>>),
    GetTargetCapabilities(mpsc::Sender<HashSet<Capability>>),
    GetTargetDevicePaths(mpsc::Sender<Vec<String>>),
    HandleEvent(NativeEvent),
    LoadProfileFromYaml(String, mpsc::Sender<Result<(), String>>),
    LoadProfile(
        DeviceProfile,
        Option<PathBuf>,
        mpsc::Sender<Result<(), String>>,
    ),
    ProcessEvent(String, Event),
    ProcessOutputEvent(OutputEvent),
    RemoveRecentEvent(Capability),
    SetInterceptActivation(Vec<Capability>, Capability),
    SetInterceptMode(InterceptMode),
    SetTargetDevices(Vec<TargetDeviceTypeId>),
    GetFilteredEvents(mpsc::Sender<HashMap<String, Vec<Capability>>>),
    SetFilteredEvents(HashMap<String, Vec<Capability>>),
    GetFilterableEvents(mpsc::Sender<HashMap<String, Vec<Capability>>>),
    GetForceFeedbackEnabled(mpsc::Sender<bool>),
    SetForceFeedbackEnabled(bool),
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
