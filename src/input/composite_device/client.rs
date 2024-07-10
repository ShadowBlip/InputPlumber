use std::collections::{HashMap, HashSet};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc::{channel, error::SendError, Sender};
use tracing::instrument;

use crate::input::event::native::NativeEvent;
use crate::input::message::Message;
use crate::input::{
    capability::Capability, event::Event, manager::SourceDeviceInfo, output_event::OutputEvent,
    target::TargetCommand,
};

use super::{CompositeCommand, InterceptMode};

/// Possible errors for a composite device client
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("failed to send command to device")]
    SendError(SendError<Message<CompositeCommand>>),
    #[error("service encountered an error processing the request")]
    ServiceError(Box<dyn std::error::Error>),
    #[error("device no longer exists")]
    ChannelClosed,
}

impl From<SendError<Message<CompositeCommand>>> for ClientError {
    fn from(err: SendError<Message<CompositeCommand>>) -> Self {
        Self::SendError(err)
    }
}

/// A client for a composite device
#[derive(Debug, Clone)]
pub struct CompositeDeviceClient {
    tx: Sender<Message<CompositeCommand>>,
}

impl From<Sender<Message<CompositeCommand>>> for CompositeDeviceClient {
    fn from(tx: Sender<Message<CompositeCommand>>) -> Self {
        Self { tx }
    }
}

impl CompositeDeviceClient {
    pub fn new(tx: Sender<Message<CompositeCommand>>) -> Self {
        Self { tx }
    }

    /// Get the name of the composite device
    #[instrument(skip_all)]
    pub async fn get_name(&self) -> Result<String, ClientError> {
        let (tx, mut rx) = channel(1);
        let msg = Message::new(CompositeCommand::GetName(tx));
        self.tx.send(msg).await?;
        if let Some(name) = rx.recv().await {
            return Ok(name);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Process the given event from the given device
    #[instrument(skip_all)]
    pub async fn process_event(&self, device_id: String, event: Event) -> Result<(), ClientError> {
        tracing::trace!("Process event from {device_id} for {event:?}");
        let msg = Message::new(CompositeCommand::ProcessEvent(device_id.clone(), event));
        msg.span.record("source_device", device_id);
        log::debug!("Trace: {:?}", msg.span);
        self.tx.send(msg).await?;
        log::trace!("A log msg");
        tracing::trace!("Sent message to composite device");
        Ok(())
    }

    /// Process the given event from the given device (blocking)
    #[instrument(skip_all)]
    pub fn blocking_process_event(
        &self,
        device_id: String,
        event: Event,
    ) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::ProcessEvent(device_id, event));
        self.tx.blocking_send(msg)?;
        Ok(())
    }

    /// Process the given output event
    #[instrument(skip_all)]
    pub async fn process_output_event(&self, event: OutputEvent) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::ProcessOutputEvent(event));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Process the given output event (blocking)
    #[instrument(skip_all)]
    pub fn blocking_process_output_event(&self, event: OutputEvent) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::ProcessOutputEvent(event));
        self.tx.blocking_send(msg)?;
        Ok(())
    }

    /// Get capabilities from all source devices
    #[instrument(skip_all)]
    pub async fn get_capabilities(&self) -> Result<HashSet<Capability>, ClientError> {
        let (tx, mut rx) = channel(1);
        let msg = Message::new(CompositeCommand::GetCapabilities(tx));
        self.tx.send(msg).await?;
        if let Some(capabilities) = rx.recv().await {
            return Ok(capabilities);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get capabilities from all target devices
    #[instrument(skip_all)]
    pub async fn get_target_capabilities(&self) -> Result<HashSet<Capability>, ClientError> {
        let (tx, mut rx) = channel(1);
        let msg = Message::new(CompositeCommand::GetTargetCapabilities(tx));
        self.tx.send(msg).await?;
        if let Some(capabilities) = rx.recv().await {
            return Ok(capabilities);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Set the intercept mode of the composite device
    #[instrument(skip_all)]
    pub async fn set_intercept_mode(&self, mode: InterceptMode) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::SetInterceptMode(mode));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Get the intercept mode of the composite device
    #[instrument(skip_all)]
    pub async fn get_intercept_mode(&self) -> Result<InterceptMode, ClientError> {
        let (tx, mut rx) = channel(1);
        let msg = Message::new(CompositeCommand::GetInterceptMode(tx));
        self.tx.send(msg).await?;
        if let Some(mode) = rx.recv().await {
            return Ok(mode);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the source device paths of the composite device
    #[instrument(skip_all)]
    pub async fn get_source_device_paths(&self) -> Result<Vec<String>, ClientError> {
        let (tx, mut rx) = channel(1);
        let msg = Message::new(CompositeCommand::GetSourceDevicePaths(tx));
        self.tx.send(msg).await?;
        if let Some(paths) = rx.recv().await {
            return Ok(paths);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the target device paths of the composite device
    #[instrument(skip_all)]
    pub async fn get_target_device_paths(&self) -> Result<Vec<String>, ClientError> {
        let (tx, mut rx) = channel(1);
        let msg = Message::new(CompositeCommand::GetTargetDevicePaths(tx));
        self.tx.send(msg).await?;
        if let Some(paths) = rx.recv().await {
            return Ok(paths);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the DBus device paths of the composite device
    #[instrument(skip_all)]
    pub async fn get_dbus_device_paths(&self) -> Result<Vec<String>, ClientError> {
        let (tx, mut rx) = channel(1);
        let msg = Message::new(CompositeCommand::GetDBusDevicePaths(tx));
        self.tx.send(msg).await?;
        if let Some(paths) = rx.recv().await {
            return Ok(paths);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Add the given source device to the composite device
    #[instrument(skip_all)]
    pub async fn add_source_device(&self, info: SourceDeviceInfo) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::SourceDeviceAdded(info));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Remove the given source device from the composite device
    #[instrument(skip_all)]
    pub async fn remove_source_device(&self, path: String) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::SourceDeviceRemoved(path));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Set the given target devices on the composite device. This will create
    /// new target devices, attach them to this device, and stop/remove any
    /// existing devices.
    #[instrument(skip_all)]
    pub async fn set_target_devices(&self, devices: Vec<String>) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::SetTargetDevices(devices));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Attach the given target devices to the composite device
    #[instrument(skip_all)]
    pub async fn attach_target_devices(
        &self,
        devices: HashMap<String, Sender<TargetCommand>>,
    ) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::AttachTargetDevices(devices));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Get the name of the currently loaded profile
    #[instrument(skip_all)]
    pub async fn get_profile_name(&self) -> Result<String, ClientError> {
        let (tx, mut rx) = channel(1);
        let msg = Message::new(CompositeCommand::GetProfileName(tx));
        self.tx.send(msg).await?;
        if let Some(name) = rx.recv().await {
            return Ok(name);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Load the device profile from the given path
    #[instrument(skip_all)]
    pub async fn load_profile_path(&self, path: String) -> Result<(), ClientError> {
        let (tx, mut rx) = channel(1);
        let msg = Message::new(CompositeCommand::LoadProfilePath(path, tx));
        self.tx.send(msg).await?;
        if let Some(result) = rx.recv().await {
            return match result {
                Ok(_) => Ok(()),
                Err(e) => Err(ClientError::ServiceError(e.into())),
            };
        }
        Err(ClientError::ChannelClosed)
    }

    /// Write the given event to the appropriate target device.
    #[instrument(skip_all)]
    pub async fn write_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::WriteEvent(event));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Write the given set of events as a button chord
    #[instrument(skip_all)]
    pub async fn write_chord(&self, events: Vec<NativeEvent>) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::WriteChordEvent(events));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Write the given event to the appropriate target device, bypassing intercept
    /// logic.
    #[instrument(skip_all)]
    pub async fn write_send_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::WriteSendEvent(event));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Write the given event to the appropriate target device, bypassing intercept
    /// logic. (blocking)
    #[instrument(skip_all)]
    pub fn blocking_write_send_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::WriteSendEvent(event));
        self.tx.blocking_send(msg)?;
        Ok(())
    }

    /// Translate and write the given event to the appropriate target devices
    #[instrument(skip_all)]
    pub async fn handle_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::HandleEvent(event));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Remove the given event type from list of recently translated events
    #[instrument(skip_all)]
    pub async fn remove_recent_event(&self, capability: Capability) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::RemoveRecentEvent(capability));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Set the events to look for to activate input interception while in
    /// "PASS" mode.
    #[instrument(skip_all)]
    pub async fn set_intercept_activation(
        &self,
        activation_caps: Vec<Capability>,
        target_cap: Capability,
    ) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::SetInterceptActivation(
            activation_caps,
            target_cap,
        ));
        self.tx.send(msg).await?;
        Ok(())
    }

    /// Stop the composite device
    #[instrument(skip_all)]
    pub async fn stop(&self) -> Result<(), ClientError> {
        let msg = Message::new(CompositeCommand::Stop);
        self.tx.send(msg).await?;
        Ok(())
    }
}
