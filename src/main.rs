use simple_logger::SimpleLogger;
use std::error::Error;
use std::future::pending;
use tokio::sync::mpsc;
use tokio::sync::watch;
use zbus::Connection;

use crate::gamepad::manager;
use crate::gamepad::watcher;
use crate::gamepad::watcher::WatchEvent;

mod gamepad;

const BUS_NAME: &str = "org.shadowblip.HanDBus";
const PREFIX: &str = "/org/shadowblip/Gamepads";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().init().unwrap();
    log::info!("Starting hanDBus");

    // Configure the DBus connection
    let connection = Connection::system().await?;

    // Create a watch channel for filesystem events to propagate to other
    // systems
    let (watcher_tx, mut watcher_rx) = watch::channel(WatchEvent::Other {});

    // Create an instance of Gamepad Manager
    let (manager_tx, manager_rx) = mpsc::channel(32);
    let manager_watch_tx = manager_tx.clone();
    let (manager_frontend, mut manager_backend) = manager::new(manager_tx, manager_rx);

    // Listen for watch events and dispatch them to Gamepad Manager
    tokio::spawn(async move {
        // Use the equivalent of a "do-while" loop so the initial value is
        // processed before awaiting the `changed()` future.
        loop {
            let event = watcher_rx.borrow_and_update().clone();
            let _ = manager_watch_tx
                .send(manager::Command::WatchEvent { event })
                .await
                .unwrap();
            if watcher_rx.changed().await.is_err() {
                log::warn!("Error waiting for watch events");
                break;
            }
        }
    });

    // Serve the Gamepad Manager on DBus
    let gamepads_path = format!("{0}", PREFIX);
    connection
        .object_server()
        .at(gamepads_path, manager_frontend)
        .await?;
    connection.request_name(BUS_NAME).await?;

    // Start the backend
    tokio::spawn(async move {
        log::info!("Starting backend");
        manager_backend.run(connection).await;
    });

    // Watch for device change events and send them over the watcher channel
    log::info!("Starting filesystem watcher");
    watcher::watch(String::from("/dev/input"), watcher_tx);

    // Do other things or go to wait forever
    pending::<()>().await;

    Ok(())
}
