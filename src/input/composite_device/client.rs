use std::collections::{HashMap, HashSet};
use thiserror::Error;
use tokio::sync::mpsc::{channel, error::SendError, Sender};

use crate::input::event::native::NativeEvent;
use crate::input::target::client::TargetDeviceClient;
use crate::input::{
    capability::Capability, event::Event, manager::SourceDeviceInfo, output_event::OutputEvent,
};

use super::{CompositeCommand, InterceptMode};

/// Possible errors for a composite device client
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("failed to send command to device")]
    SendError(SendError<CompositeCommand>),
    #[error("service encountered an error processing the request")]
    ServiceError(Box<dyn std::error::Error>),
    #[error("device no longer exists")]
    ChannelClosed,
}

impl From<SendError<CompositeCommand>> for ClientError {
    fn from(err: SendError<CompositeCommand>) -> Self {
        Self::SendError(err)
    }
}

/// A client for a composite device
#[derive(Debug, Clone)]
pub struct CompositeDeviceClient {
    tx: Sender<CompositeCommand>,
}

impl From<Sender<CompositeCommand>> for CompositeDeviceClient {
    fn from(tx: Sender<CompositeCommand>) -> Self {
        CompositeDeviceClient::new(tx)
    }
}

impl CompositeDeviceClient {
    pub fn new(tx: Sender<CompositeCommand>) -> Self {
        Self { tx }
    }

    /// Get the name of the composite device
    pub async fn get_name(&self) -> Result<String, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx.send(CompositeCommand::GetName(tx)).await?;
        if let Some(name) = rx.recv().await {
            return Ok(name);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Process the given event from the given device
    pub async fn process_event(&self, device_id: String, event: Event) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::ProcessEvent(device_id, event))
            .await?;
        Ok(())
    }

    /// Process the given event from the given device (blocking)
    pub fn blocking_process_event(
        &self,
        device_id: String,
        event: Event,
    ) -> Result<(), ClientError> {
        self.tx
            .blocking_send(CompositeCommand::ProcessEvent(device_id, event))?;
        Ok(())
    }

    /// Process the given output event
    pub async fn process_output_event(&self, event: OutputEvent) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::ProcessOutputEvent(event))
            .await?;
        Ok(())
    }

    /// Process the given output event (blocking)
    pub fn blocking_process_output_event(&self, event: OutputEvent) -> Result<(), ClientError> {
        self.tx
            .blocking_send(CompositeCommand::ProcessOutputEvent(event))?;
        Ok(())
    }

    /// Get capabilities from all source devices
    pub async fn get_capabilities(&self) -> Result<HashSet<Capability>, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx.send(CompositeCommand::GetCapabilities(tx)).await?;
        if let Some(capabilities) = rx.recv().await {
            return Ok(capabilities);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get capabilities from all target devices
    pub async fn get_target_capabilities(&self) -> Result<HashSet<Capability>, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx
            .send(CompositeCommand::GetTargetCapabilities(tx))
            .await?;
        if let Some(capabilities) = rx.recv().await {
            return Ok(capabilities);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Set the intercept mode of the composite device
    pub async fn set_intercept_mode(&self, mode: InterceptMode) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::SetInterceptMode(mode))
            .await?;
        Ok(())
    }

    /// Get the intercept mode of the composite device
    pub async fn get_intercept_mode(&self) -> Result<InterceptMode, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx.send(CompositeCommand::GetInterceptMode(tx)).await?;
        if let Some(mode) = rx.recv().await {
            return Ok(mode);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the source device paths of the composite device
    pub async fn get_source_device_paths(&self) -> Result<Vec<String>, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx
            .send(CompositeCommand::GetSourceDevicePaths(tx))
            .await?;
        if let Some(paths) = rx.recv().await {
            return Ok(paths);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the target device paths of the composite device
    pub async fn get_target_device_paths(&self) -> Result<Vec<String>, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx
            .send(CompositeCommand::GetTargetDevicePaths(tx))
            .await?;
        if let Some(paths) = rx.recv().await {
            return Ok(paths);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the DBus device paths of the composite device
    pub async fn get_dbus_device_paths(&self) -> Result<Vec<String>, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx
            .send(CompositeCommand::GetDBusDevicePaths(tx))
            .await?;
        if let Some(paths) = rx.recv().await {
            return Ok(paths);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Add the given source device to the composite device
    pub async fn add_source_device(&self, info: SourceDeviceInfo) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::SourceDeviceAdded(info))
            .await?;
        Ok(())
    }

    /// Remove the given source device from the composite device
    pub async fn remove_source_device(&self, path: String) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::SourceDeviceRemoved(path))
            .await?;
        Ok(())
    }

    /// Set the given target devices on the composite device. This will create
    /// new target devices, attach them to this device, and stop/remove any
    /// existing devices.
    pub async fn set_target_devices(&self, devices: Vec<String>) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::SetTargetDevices(devices))
            .await?;
        Ok(())
    }

    /// Attach the given target devices to the composite device
    pub async fn attach_target_devices(
        &self,
        devices: HashMap<String, TargetDeviceClient>,
    ) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::AttachTargetDevices(devices))
            .await?;
        Ok(())
    }

    /// Get the name of the currently loaded profile
    pub async fn get_profile_name(&self) -> Result<String, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx.send(CompositeCommand::GetProfileName(tx)).await?;
        if let Some(name) = rx.recv().await {
            return Ok(name);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Load the device profile from the given path
    pub async fn load_profile_path(&self, path: String) -> Result<(), ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx
            .send(CompositeCommand::LoadProfilePath(path, tx))
            .await?;
        if let Some(result) = rx.recv().await {
            return match result {
                Ok(_) => Ok(()),
                Err(e) => Err(ClientError::ServiceError(e.into())),
            };
        }
        Err(ClientError::ChannelClosed)
    }

    /// Write the given event to the appropriate target device.
    pub async fn write_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        self.tx.send(CompositeCommand::WriteEvent(event)).await?;
        Ok(())
    }

    /// Write the given set of events as a button chord
    pub async fn write_chord(&self, events: Vec<NativeEvent>) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::WriteChordEvent(events))
            .await?;
        Ok(())
    }

    /// Write the given event to the appropriate target device, bypassing intercept
    /// logic.
    pub async fn write_send_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::WriteSendEvent(event))
            .await?;
        Ok(())
    }

    /// Write the given event to the appropriate target device, bypassing intercept
    /// logic. (blocking)
    pub fn blocking_write_send_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        self.tx
            .blocking_send(CompositeCommand::WriteSendEvent(event))?;
        Ok(())
    }

    /// Translate and write the given event to the appropriate target devices
    pub async fn handle_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        self.tx.send(CompositeCommand::HandleEvent(event)).await?;
        Ok(())
    }

    /// Remove the given event type from list of recently translated events
    pub async fn remove_recent_event(&self, capability: Capability) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::RemoveRecentEvent(capability))
            .await?;
        Ok(())
    }

    /// Set the events to look for to activate input interception while in
    /// "PASS" mode.
    pub async fn set_intercept_activation(
        &self,
        activation_caps: Vec<Capability>,
        target_cap: Capability,
    ) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::SetInterceptActivation(
                activation_caps,
                target_cap,
            ))
            .await?;
        Ok(())
    }

    /// Stop the composite device
    pub async fn stop(&self) -> Result<(), ClientError> {
        self.tx.send(CompositeCommand::Stop).await?;
        Ok(())
    }
}
