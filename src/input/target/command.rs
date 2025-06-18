use tokio::sync::mpsc::Sender;

use crate::input::{
    capability::Capability, composite_device::client::CompositeDeviceClient,
    event::native::NativeEvent,
};

use super::TargetDeviceTypeId;

/// A [TargetCommand] is a message that can be sent to a [TargetDevice] over
/// a channel.
#[derive(Debug, Clone)]
pub enum TargetCommand {
    /// Write the given event to the target device
    WriteEvent(NativeEvent),
    /// Set the given composite device on the target device
    SetCompositeDevice(CompositeDeviceClient),
    /// Return the input capabilities of the target device
    GetCapabilities(Sender<Vec<Capability>>),
    /// Return the type of target input device
    GetType(Sender<TargetDeviceTypeId>),
    /// Clear all local state on the target device
    ClearState,
    /// Stop the target device
    Stop,
}
