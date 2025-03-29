use futures::StreamExt;
use zbus::{fdo, object_server::SignalEmitter};
use zbus_macros::interface;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc::Sender,
};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::input::target::websocket::DeviceCommand;

/// The [TargetWebsocketInterface] provides a DBus interface that can be exposed for managing
/// a websocket device.
pub struct TargetWebsocketInterface {
    pub connection: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    pub device_tx: Sender<DeviceCommand>,
}

impl TargetWebsocketInterface {
    pub fn new(device_tx: Sender<DeviceCommand>) -> TargetWebsocketInterface {
        TargetWebsocketInterface {
            connection: None,
            device_tx,
        }
    }
}

#[interface(
    name = "org.shadowblip.Input.Websocket",
    proxy(default_service = "org.shadowblip.InputPlumber",)
)]
impl TargetWebsocketInterface {
    /// Connect to the given InputPlumber websocket server
    pub async fn connect(&mut self, url: String) -> fdo::Result<()> {
        let (ws_stream, _resp) = connect_async(&url)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        log::info!("WebSocket handshake has been successfully completed");
        self.connection = Some(ws_stream);
        self.device_tx
            .send(DeviceCommand::WebsocketConnected)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    #[zbus(property)]
    pub async fn connected(&self) -> fdo::Result<bool> {
        Ok(self.connection.is_some())
    }
}
