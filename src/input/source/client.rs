use std::{sync::mpsc::channel, time::Duration};

use evdev::FFEffectData;
use thiserror::Error;
use tokio::sync::mpsc::{
    error::{SendError, TrySendError},
    Sender,
};

use crate::input::output_event::OutputEvent;

use super::command::SourceCommand;

/// Possible errors for a source device client
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("failed to send command to device")]
    SendError(SendError<SourceCommand>),
    #[error("failed to try to send command to device")]
    TrySendError(TrySendError<SourceCommand>),
    #[error("service encountered an error processing the request")]
    ServiceError(Box<dyn std::error::Error + Send + Sync>),
    #[error("device no longer exists")]
    ChannelClosed,
}

impl From<SendError<SourceCommand>> for ClientError {
    fn from(err: SendError<SourceCommand>) -> Self {
        Self::SendError(err)
    }
}

impl From<TrySendError<SourceCommand>> for ClientError {
    fn from(err: TrySendError<SourceCommand>) -> Self {
        Self::TrySendError(err)
    }
}

/// A client for communicating with a source device
#[derive(Debug, Clone)]
pub struct SourceDeviceClient {
    tx: Sender<SourceCommand>,
}

impl From<Sender<SourceCommand>> for SourceDeviceClient {
    fn from(tx: Sender<SourceCommand>) -> Self {
        SourceDeviceClient::new(tx)
    }
}

impl SourceDeviceClient {
    pub fn new(tx: Sender<SourceCommand>) -> Self {
        Self { tx }
    }

    /// Write the given output event to the source device. Output events are
    /// events that flow from an application (like a game) to the physical
    /// input device, such as force feedback events.
    pub async fn write_event(&self, event: OutputEvent) -> Result<(), ClientError> {
        self.tx.send(SourceCommand::WriteEvent(event)).await?;
        Ok(())
    }

    /// Upload the given force feedback effect data to the source device. Returns
    /// a device-specific id of the uploaded effect if it is successful.
    pub async fn upload_effect(&self, effect: FFEffectData) -> Result<i16, ClientError> {
        let (tx, rx) = channel();
        self.tx.try_send(SourceCommand::UploadEffect(effect, tx))?;
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(result) => match result {
                Ok(id) => Ok(id),
                Err(err) => Err(ClientError::ServiceError(err)),
            },
            Err(_err) => Err(ClientError::ChannelClosed),
        }
    }

    /// Update the effect with the given id using the given effect data.
    pub async fn update_effect(
        &self,
        effect_id: i16,
        effect: FFEffectData,
    ) -> Result<(), ClientError> {
        self.tx
            .send(SourceCommand::UpdateEffect(effect_id, effect))
            .await?;
        Ok(())
    }

    /// Erase the effect with the given id from the source device.
    pub async fn erase_effect(&self, effect_id: i16) -> Result<(), ClientError> {
        let (tx, rx) = channel();
        self.tx
            .try_send(SourceCommand::EraseEffect(effect_id, tx))?;
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(result) => match result {
                Ok(_) => Ok(()),
                Err(err) => Err(ClientError::ServiceError(err)),
            },
            Err(_err) => Err(ClientError::ChannelClosed),
        }
    }

    /// Stop the source device.
    pub async fn stop(&self) -> Result<(), ClientError> {
        self.tx.send(SourceCommand::Stop).await?;
        Ok(())
    }
}
