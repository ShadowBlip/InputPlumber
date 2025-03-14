use std::net::SocketAddr;

use futures::{stream::SplitSink, StreamExt};
use tokio::{
    fs,
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use crate::{
    config::{path::get_devices_paths, CompositeDeviceConfig, Websocket},
    input::manager::ManagerCommand,
};

/// Write/Read halfs of a websocket stream
pub type SplitStream = (
    SplitSink<WebSocketStream<TcpStream>, Message>,
    Receiver<Message>,
);
pub type StreamRequestor = Sender<Sender<Option<SplitStream>>>;

/// Websocket client information and channels
#[derive(Debug, Clone)]
pub struct WebsocketClient {
    pub addr: SocketAddr,
    pub server_addr: SocketAddr,
    tx: StreamRequestor,
}

impl WebsocketClient {
    /// Create a new [WebsocketClient] instance
    pub fn new(addr: SocketAddr, server_addr: SocketAddr, tx: StreamRequestor) -> Self {
        Self {
            addr,
            server_addr,
            tx,
        }
    }

    /// Return a unique identifier for this websocket client connection. E.g.
    /// "ws://127.0.0.1:8080::192.168.0.2:12345"
    pub fn get_id(&self) -> String {
        format!(
            "ws://{}:{}::{}:{}",
            self.server_addr.ip(),
            self.server_addr.port(),
            self.addr.ip(),
            self.addr.port()
        )
    }

    /// Open the [WebSocketStream] associated with this client. This will return
    /// `None` if the stream is unavailable or has already been opened.
    pub async fn open(&self) -> Option<SplitStream> {
        let (tx, mut rx) = mpsc::channel(1);
        self.tx.send(tx).await.ok()?;
        rx.recv().await?
    }
}

pub async fn watch_websockets(cmd_tx: Sender<ManagerCommand>) {
    // Look at all composite device configs for any websocket configurations
    let mut websocket_configs = vec![];
    let paths = get_devices_paths();
    for path in paths.iter() {
        log::trace!("Checking {path:?} for websocket config");
        let Ok(mut files) = fs::read_dir(path).await else {
            continue;
        };

        while let Ok(Some(entry)) = files.next_entry().await {
            let filename = entry.file_name().to_string_lossy().to_string();
            if !filename.ends_with(".yaml") {
                continue;
            }

            let Ok(config) =
                CompositeDeviceConfig::from_yaml_file(entry.path().display().to_string())
            else {
                continue;
            };

            // Look for any 'websocket' config entries
            for source_config in config.source_devices {
                let Some(websocket_config) = source_config.websocket else {
                    continue;
                };
                websocket_configs.push(websocket_config);
            }
        }
    }

    // Spawn a websocket listener for each websocket config found
    for config in websocket_configs {
        let address = config.address.clone().unwrap_or("127.0.0.1".to_string());
        let port = config.port.clone().unwrap_or(12907);
        let Ok(server_addr) = format!("{address}:{port}").parse::<SocketAddr>() else {
            log::warn!("Invalid websocket listen address: {address}:{port}");
            continue;
        };

        let cmd_tx = cmd_tx.clone();
        tokio::spawn(async move {
            // Create the event loop and TCP listener we'll accept connections on.
            let listener = match TcpListener::bind(server_addr).await {
                Ok(result) => result,
                Err(e) => {
                    log::error!("Failed to bind to `{server_addr}`: {e}");
                    return;
                }
            };
            log::info!("Listening on: {server_addr}");

            // Listen for client connections
            while let Ok((stream, addr)) = listener.accept().await {
                tokio::spawn(accept_connection(
                    cmd_tx.clone(),
                    stream,
                    addr,
                    server_addr,
                    config.clone(),
                ));
            }
        });
    }
}

async fn accept_connection(
    cmd_tx: Sender<ManagerCommand>,
    stream: TcpStream,
    addr: SocketAddr,
    server_addr: SocketAddr,
    config: Websocket,
) {
    // TODO: Verify in the config if the connection should be accepted

    log::info!("Client connected: {}", addr);

    // Create a websocket stream from the connection
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(stream) => stream,
        Err(e) => {
            log::warn!("Error during websocket handshake: {e}");
            return;
        }
    };

    log::info!("Client successfully connected: {}", addr);

    // Notify the manager that a new client device has connected
    let (open_tx, mut open_rx) = mpsc::channel(1);
    let client = WebsocketClient::new(addr, server_addr, open_tx);
    let result = cmd_tx
        .send(ManagerCommand::DeviceAdded {
            device: client.clone().into(),
        })
        .await;
    if let Err(err) = result {
        log::error!("Failed to notify manager of new client device: {err}");
        return;
    }

    // Split the stream into the read and write halfs
    let (write, mut read) = ws_stream.split();

    // Create a channel to read websocket messages from
    let (read_tx, read_rx) = mpsc::channel(2048);
    let mut stream = Some((write, read_rx));

    // Listen for messages
    loop {
        tokio::select! {
            // Send the stream receiver if a consumer opened the stream
            opener = open_rx.recv() => {
                let Some(opener) = opener else {
                    break;
                };
                log::debug!("Received request to open stream");
                if let Err(e) = opener.send(stream.take()).await {
                    log::debug!("Failed to send websocket message: {e}");
                }
            },
            // Read messages from the stream and send them through the read
            // channel
            msg = read.next() => {
                let Some(msg) = msg else {
                    break;
                };
                log::trace!("Received websocket message");
                let msg = match msg {
                    Ok(msg) => msg,
                    Err(e) => {
                        log::warn!("Error processing websocket message: {e}");
                        break;
                    }
                };
                if let Err(e) = read_tx.send(msg).await {
                    log::debug!("Failed to send websocket message: {e}");
                }
            },
        }
    }

    log::info!("Closing websocket connection: {}", addr);

    // Notify the manager that the client was removed
    let result = cmd_tx
        .send(ManagerCommand::DeviceRemoved {
            device: client.into(),
        })
        .await;
    if let Err(err) = result {
        log::error!("Failed to notify manager of client device removal: {err}");
    }
}
