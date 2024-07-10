use tokio::sync::mpsc::Sender;

use crate::input::{
    capability::Capability, composite_device::client::CompositeDeviceClient,
    event::native::NativeEvent,
};

/// A [TargetCommand] is a message that can be sent to a [TargetDevice] over
/// a channel.
#[derive(Debug, Clone)]
pub enum TargetCommand {
    WriteEvent(NativeEvent),
    SetCompositeDevice(CompositeDeviceClient),
    GetCapabilities(Sender<Vec<Capability>>),
    GetType(Sender<String>),
    Stop,
}
