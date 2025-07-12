pub mod mpsc;

use std::{future::Future, time::Duration};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReceiveTimeoutError {
    #[error("timed out waiting for message")]
    Timeout,
    #[error("channel closed")]
    Closed,
}

/// Channel receivers that can timeout when receiving messages
pub trait TimeoutReceiver<T> {
    fn recv_timeout(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = Result<T, ReceiveTimeoutError>> + Send;
}
