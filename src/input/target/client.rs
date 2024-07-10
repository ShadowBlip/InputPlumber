use thiserror::Error;
use tokio::sync::mpsc::{
    channel,
    error::{SendError, TrySendError},
    Sender,
};

use crate::input::{
    capability::Capability, composite_device::client::CompositeDeviceClient,
    event::native::NativeEvent,
};

use super::command::TargetCommand;

/// Possible errors for a target device client
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("failed to send command to device")]
    SendError(SendError<TargetCommand>),
    #[error("failed to try to send command to device")]
    TrySendError(TrySendError<TargetCommand>),
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
        self.tx
            .send(TargetCommand::SetCompositeDevice(device))
            .await?;
        Ok(())
    }

    /// Returns the target device input capabilities that the device can handle.
    pub async fn get_capabilities(&self) -> Result<Vec<Capability>, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx.send(TargetCommand::GetCapabilities(tx)).await?;
        if let Some(value) = rx.recv().await {
            return Ok(value);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Returns a string identifier of the type of target device. This identifier
    /// should be the same text identifier used in device and input configs.
    pub async fn get_type(&self) -> Result<String, ClientError> {
        let (tx, mut rx) = channel(1);
        self.tx.send(TargetCommand::GetType(tx)).await?;
        if let Some(value) = rx.recv().await {
            return Ok(value);
        }
        Err(ClientError::ChannelClosed)
    }

    /// Stop the target device.
    pub async fn stop(&self) -> Result<(), ClientError> {
        self.tx.send(TargetCommand::Stop).await?;
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
