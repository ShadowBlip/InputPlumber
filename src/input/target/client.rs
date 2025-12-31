use std::{collections::HashSet, time::Duration};

use thiserror::Error;
use tokio::sync::mpsc::{
    channel,
    error::{SendError, SendTimeoutError, TrySendError},
    Receiver, Sender,
};

use crate::{
    input::{
        capability::Capability, composite_device::client::CompositeDeviceClient,
        event::native::NativeEvent, output_capability::OutputCapability,
    },
    sync::{ReceiveTimeoutError, TimeoutReceiver},
};

use super::{command::TargetCommand, TargetDeviceTypeId};

/// Maximum duration to wait for a response from a command. If this timeout
/// is reached, that typically indicates a deadlock somewhere in the code.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Possible errors for a target device client
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("failed to send command to device: {0}")]
    SendError(SendError<TargetCommand>),
    #[error("failed to try to send command to device: {0}")]
    TrySendError(TrySendError<TargetCommand>),
    #[error("service encountered an error processing the request: {0}")]
    ServiceError(Box<dyn std::error::Error + Send + Sync>),
    #[error("device no longer exists")]
    ChannelClosed,
}

impl From<SendError<TargetCommand>> for ClientError {
    fn from(err: SendError<TargetCommand>) -> Self {
        Self::SendError(err)
    }
}

impl From<TrySendError<TargetCommand>> for ClientError {
    fn from(err: TrySendError<TargetCommand>) -> Self {
        Self::TrySendError(err)
    }
}

/// A client for communicating with a target device
#[derive(Debug, Clone)]
pub struct TargetDeviceClient {
    tx: Sender<TargetCommand>,
}

impl From<Sender<TargetCommand>> for TargetDeviceClient {
    fn from(tx: Sender<TargetCommand>) -> Self {
        TargetDeviceClient::new(tx)
    }
}

impl TargetDeviceClient {
    /// Create a new [TargetDeviceClient] from the given channel
    pub fn new(tx: Sender<TargetCommand>) -> Self {
        Self { tx }
    }

    /// Send the given command to the target device. This method uses a timeout
    /// to detect potential deadlocks.
    async fn send(&self, cmd: TargetCommand) -> Result<(), ClientError> {
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

    /// Use the given receiver to wait for a response from the target device.
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
                log::error!("POSSIBLE DEADLOCK: timed out after {DEFAULT_TIMEOUT:?} waiting for response from target device");
                None
            }
            ReceiveTimeoutError::Closed => None,
        }
    }

    /// Write the given input event to the target device.
    pub async fn write_event(&self, event: NativeEvent) -> Result<(), ClientError> {
        self.tx.try_send(TargetCommand::WriteEvent(event))?;
        Ok(())
    }

    /// Configure the target device with the given CompositeDevice. Target devices
    /// may need to communicate with the composite device in order to send output
    /// events (like force feedback events) back to source devices.
    pub async fn set_composite_device(
        &self,
        device: CompositeDeviceClient,
    ) -> Result<(), ClientError> {
        self.send(TargetCommand::SetCompositeDevice(device)).await?;
        Ok(())
    }

    /// Returns the target device input capabilities that the device can handle.
    pub async fn get_capabilities(&self) -> Result<Vec<Capability>, ClientError> {
        let (tx, rx) = channel(1);
        self.send(TargetCommand::GetCapabilities(tx)).await?;
        if let Some(value) = Self::recv(rx).await {
            return Ok(value);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Returns a string identifier of the type of target device. This identifier
    /// should be the same text identifier used in device and input configs.
    pub async fn get_type(&self) -> Result<TargetDeviceTypeId, ClientError> {
        let (tx, rx) = channel(1);
        self.send(TargetCommand::GetType(tx)).await?;
        if let Some(value) = Self::recv(rx).await {
            return Ok(value);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Clear any local state on the target device. This is typically called
    /// whenever the composite device has entered intercept mode to indicate
    /// that the target device should stop sending input.
    pub async fn clear_state(&self) -> Result<(), ClientError> {
        self.send(TargetCommand::ClearState).await?;
        Ok(())
    }

    /// Notifies the target device that input capabilities of the source device(s)
    /// have changed.
    pub async fn notify_capabilities_changed(
        &self,
        capabilities: HashSet<Capability>,
    ) -> Result<(), ClientError> {
        self.tx
            .send(TargetCommand::NotifyCapabilitiesChanged(capabilities))
            .await?;
        Ok(())
    }

    /// Notifies the target device that output capabilities of the source device(s)
    /// have changed.
    pub async fn notify_output_capabilities_changed(
        &self,
        capabilities: HashSet<OutputCapability>,
    ) -> Result<(), ClientError> {
        self.tx
            .send(TargetCommand::NotifyOutputCapabilitiesChanged(capabilities))
            .await?;
        Ok(())
    }

    /// Stop the target device.
    pub async fn stop(&self) -> Result<(), ClientError> {
        self.send(TargetCommand::Stop).await?;
        Ok(())
    }

    /// Completes when the receiver has dropped.
    ///
    /// This allows the producers to get notified when interest in the produced
    /// values is canceled and immediately stop doing work.
    pub async fn closed(&self) {
        self.tx.closed().await
    }
}
