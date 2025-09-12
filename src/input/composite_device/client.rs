use std::collections::{HashMap, HashSet};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc::error::SendTimeoutError;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::{channel, error::SendError, Sender};

use crate::config::CompositeDeviceConfig;
use crate::input::event::native::NativeEvent;
use crate::input::info::DeviceInfo;
use crate::input::output_capability::OutputCapability;
use crate::input::target::client::TargetDeviceClient;
use crate::input::target::TargetDeviceTypeId;
use crate::input::{capability::Capability, event::Event, output_event::OutputEvent};
use crate::sync::{ReceiveTimeoutError, TimeoutReceiver};

use super::{CompositeCommand, InterceptMode};

/// Maximum duration to wait for a response from a command. If this timeout
/// is reached, that typically indicates a deadlock somewhere in the code.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Possible errors for a composite device client
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("failed to send command to device: {0}")]
    SendError(SendError<CompositeCommand>),
    #[error("service encountered an error processing the request: {0}")]
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

    /// Send the given command to the composite device. This method uses a timeout
    /// to detect potential deadlocks.
    async fn send(&self, cmd: CompositeCommand) -> Result<(), ClientError> {
        let result = self.tx.send_timeout(cmd, DEFAULT_TIMEOUT).await;
        let Err(err) = result else {
            return Ok(());
        };
        match err {
            SendTimeoutError::Timeout(ref cmd) => {
                log::error!("POSSIBLE DEADLOCK: timed out after {DEFAULT_TIMEOUT:?} sending command to composite device: {cmd:?}");
                Err(ClientError::ServiceError(err.into()))
            }
            SendTimeoutError::Closed(_) => Err(ClientError::ChannelClosed),
        }
    }

    /// Use the given receiver to wait for a response from the composite device.
    /// This method uses a timeout to detect potential deadlocks.
    async fn recv<T>(mut rx: Receiver<T>) -> Option<T>
    where
        T: Send + Sync,
    {
        let result = rx.recv_timeout(DEFAULT_TIMEOUT).await;
        let Err(err) = result else {
            return result.ok();
        };
        match err {
            ReceiveTimeoutError::Timeout => {
                log::error!("POSSIBLE DEADLOCK: timed out after {DEFAULT_TIMEOUT:?} waiting for response from composite device");
                None
            }
            ReceiveTimeoutError::Closed => None,
        }
    }

    /// Get the name of the composite device
    pub async fn get_name(&self) -> Result<String, ClientError> {
        let (tx, rx) = channel(1);
        self.send(CompositeCommand::GetName(tx)).await?;
        if let Some(name) = Self::recv(rx).await {
            return Ok(name);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Process the given event from the given device
    #[allow(dead_code)]
    pub async fn process_event(&self, device_id: String, event: Event) -> Result<(), ClientError> {
        self.send(CompositeCommand::ProcessEvent(device_id, event))
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
        self.send(CompositeCommand::ProcessOutputEvent(event))
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
        let (tx, rx) = channel(1);
        self.send(CompositeCommand::GetCapabilities(tx)).await?;
        if let Some(capabilities) = Self::recv(rx).await {
            return Ok(capabilities);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get capabilities from all source devices (blocking)
    #[allow(dead_code)]
    pub fn blocking_get_capabilities(&self) -> Result<HashSet<Capability>, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx
            .blocking_send(CompositeCommand::GetCapabilities(tx))?;
        if let Some(capabilities) = rx.blocking_recv() {
            return Ok(capabilities);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the output capabilities from all source devices
    pub async fn get_output_capabilities(&self) -> Result<HashSet<OutputCapability>, ClientError> {
        let (tx, rx) = channel(1);
        self.tx
            .send(CompositeCommand::GetOutputCapabilities(tx))
            .await?;
        if let Some(capabilities) = Self::recv(rx).await {
            return Ok(capabilities);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the output capabilities from all source devices (blocking)
    #[allow(dead_code)]
    pub fn blocking_get_output_capabilities(
        &self,
    ) -> Result<HashSet<OutputCapability>, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx
            .blocking_send(CompositeCommand::GetOutputCapabilities(tx))?;
        if let Some(capabilities) = rx.blocking_recv() {
            return Ok(capabilities);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the [CompositeDeviceConfig] from the [CompositeDevice]
    pub async fn get_config(&self) -> Result<CompositeDeviceConfig, ClientError> {
        let (tx, rx) = channel(1);
        self.send(CompositeCommand::GetConfig(tx)).await?;
        if let Some(config) = Self::recv(rx).await {
            return Ok(config);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the [CompositeDeviceConfig] from the [CompositeDevice] (blocking)
    #[allow(dead_code)]
    pub fn blocking_get_config(&self) -> Result<CompositeDeviceConfig, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx.blocking_send(CompositeCommand::GetConfig(tx))?;
        if let Some(config) = rx.blocking_recv() {
            return Ok(config);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get capabilities from all target devices
    pub async fn get_target_capabilities(&self) -> Result<HashSet<Capability>, ClientError> {
        let (tx, rx) = channel(1);
        self.tx
            .send(CompositeCommand::GetTargetCapabilities(tx))
            .await?;
        if let Some(capabilities) = Self::recv(rx).await {
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
        let (tx, rx) = channel(1);
        self.send(CompositeCommand::GetInterceptMode(tx)).await?;
        if let Some(mode) = Self::recv(rx).await {
            return Ok(mode);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the source device paths of the composite device
    pub async fn get_source_device_paths(&self) -> Result<Vec<String>, ClientError> {
        let (tx, rx) = channel(1);
        self.tx
            .send(CompositeCommand::GetSourceDevicePaths(tx))
            .await?;
        if let Some(paths) = Self::recv(rx).await {
            return Ok(paths);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the source device paths of the composite device (blocking)
    #[allow(dead_code)]
    pub fn blocking_get_source_device_paths(&self) -> Result<Vec<String>, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx
            .blocking_send(CompositeCommand::GetSourceDevicePaths(tx))?;
        if let Some(paths) = rx.blocking_recv() {
            return Ok(paths);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the target device paths of the composite device
    pub async fn get_target_device_paths(&self) -> Result<Vec<String>, ClientError> {
        let (tx, rx) = channel(1);
        self.tx
            .send(CompositeCommand::GetTargetDevicePaths(tx))
            .await?;
        if let Some(paths) = Self::recv(rx).await {
            return Ok(paths);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Get the DBus device paths of the composite device
    pub async fn get_dbus_device_paths(&self) -> Result<Vec<String>, ClientError> {
        let (tx, rx) = channel(1);
        self.tx
            .send(CompositeCommand::GetDBusDevicePaths(tx))
            .await?;
        if let Some(paths) = Self::recv(rx).await {
            return Ok(paths);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Add the given source device to the composite device
    pub async fn add_source_device(&self, device: DeviceInfo) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::SourceDeviceAdded(device))
            .await?;
        Ok(())
    }

    /// Remove the given source device from the composite device
    pub async fn remove_source_device(&self, device: DeviceInfo) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::SourceDeviceRemoved(device))
            .await?;
        Ok(())
    }

    /// Set the given target devices on the composite device. This will create
    /// new target devices, attach them to this device, and stop/remove any
    /// existing devices.
    pub async fn set_target_devices(
        &self,
        devices: Vec<TargetDeviceTypeId>,
    ) -> Result<(), ClientError> {
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
    #[allow(dead_code)]
    pub async fn get_profile_name(&self) -> Result<String, ClientError> {
        let (tx, rx) = channel(1);
        self.send(CompositeCommand::GetProfileName(tx)).await?;
        if let Some(name) = Self::recv(rx).await {
            return Ok(name);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Load the device profile from the given path
    pub async fn load_profile_path(&self, path: String) -> Result<(), ClientError> {
        let (tx, rx) = channel(1);
        self.tx
            .send(CompositeCommand::LoadProfilePath(path, tx))
            .await?;
        if let Some(result) = Self::recv(rx).await {
            return match result {
                Ok(_) => Ok(()),
                Err(e) => Err(ClientError::ServiceError(e.into())),
            };
        }
        Err(ClientError::ChannelClosed)
    }

    /// Load the device profile from the given path
    pub async fn load_profile_from_yaml(&self, profile: String) -> Result<(), ClientError> {
        let (tx, rx) = channel(1);
        self.tx
            .send(CompositeCommand::LoadProfileFromYaml(profile, tx))
            .await?;
        if let Some(result) = Self::recv(rx).await {
            return match result {
                Ok(_) => Ok(()),
                Err(e) => Err(ClientError::ServiceError(e.into())),
            };
        }
        Err(ClientError::ChannelClosed)
    }

    /// Update the input capabilities for the given source device
    #[allow(dead_code)]
    pub async fn update_source_capabilities(
        &self,
        device_id: String,
        capabilities: HashSet<Capability>,
    ) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::UpdateSourceCapabilities(
                device_id,
                capabilities,
            ))
            .await?;
        Ok(())
    }

    /// Update the input capabilities for the given source device (blocking)
    #[allow(dead_code)]
    pub fn blocking_update_source_capabilities(
        &self,
        device_id: String,
        capabilities: HashSet<Capability>,
    ) -> Result<(), ClientError> {
        self.tx
            .blocking_send(CompositeCommand::UpdateSourceCapabilities(
                device_id,
                capabilities,
            ))?;
        Ok(())
    }

    /// Update the input capabilities for the given target device
    #[allow(dead_code)]
    pub async fn update_target_capabilities(
        &self,
        dbus_path: String,
        capabilities: HashSet<Capability>,
    ) -> Result<(), ClientError> {
        self.tx
            .send(CompositeCommand::UpdateTargetCapabilities(
                dbus_path,
                capabilities,
            ))
            .await?;
        Ok(())
    }

    /// Update the input capabilities for the given target device (blocking)
    pub fn blocking_update_target_capabilities(
        &self,
        dbus_path: String,
        capabilities: HashSet<Capability>,
    ) -> Result<(), ClientError> {
        self.tx
            .blocking_send(CompositeCommand::UpdateTargetCapabilities(
                dbus_path,
                capabilities,
            ))?;
        Ok(())
    }
    /// Write the given event to the appropriate target device.
    #[allow(dead_code)]
    pub async fn write_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        self.send(CompositeCommand::WriteEvent(event)).await?;
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub async fn handle_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        self.send(CompositeCommand::HandleEvent(event)).await?;
        Ok(())
    }

    /// Remove the given event type from list of recently translated events
    #[allow(dead_code)]
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
        self.send(CompositeCommand::Stop).await?;
        Ok(())
    }

    /// Calls the suspend handler to perform system suspend-related tasks.
    pub async fn suspend(&self) -> Result<(), ClientError> {
        let (tx, rx) = channel(1);
        self.send(CompositeCommand::Suspend(tx)).await?;

        if let Some(result) = Self::recv(rx).await {
            return Ok(result);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Calls the resume handler to perform system wake from suspend-related tasks.
    pub async fn resume(&self) -> Result<(), ClientError> {
        let (tx, rx) = channel(1);
        self.send(CompositeCommand::Resume(tx)).await?;

        if let Some(result) = Self::recv(rx).await {
            return Ok(result);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Returns true if the target devices are suspended
    pub async fn is_suspended(&self) -> Result<bool, ClientError> {
        let (tx, rx) = channel(1);
        self.send(CompositeCommand::IsSuspended(tx)).await?;

        if let Some(result) = Self::recv(rx).await {
            return Ok(result);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Returns whether or not the device should emit force feedback events
    pub async fn get_ff_enabled(&self) -> Result<bool, ClientError> {
        let (tx, rx) = channel(1);
        self.send(CompositeCommand::GetForceFeedbackEnabled(tx))
            .await?;
        if let Some(result) = Self::recv(rx).await {
            return Ok(result);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Enable or disable force feedback output events from being emitted
    pub async fn set_ff_enabled(&self, enabled: bool) -> Result<(), ClientError> {
        self.send(CompositeCommand::SetForceFeedbackEnabled(enabled))
            .await?;
        Ok(())
    }
}
