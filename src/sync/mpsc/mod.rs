use std::time::Duration;

use tokio::{sync::mpsc::Receiver, time::timeout};

use super::{ReceiveTimeoutError, TimeoutReceiver};

impl<T> TimeoutReceiver<T> for Receiver<T>
where
    T: Send + Sync,
{
    async fn recv_timeout(&mut self, duration: Duration) -> Result<T, ReceiveTimeoutError> {
        let result = timeout(duration, self.recv()).await;
        match result {
            Ok(value) => match value {
                Some(v) => Ok(v),
                None => Err(ReceiveTimeoutError::Closed),
            },
            Err(_) => Err(ReceiveTimeoutError::Timeout),
        }
    }
}
