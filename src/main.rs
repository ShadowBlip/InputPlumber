use simple_logger::SimpleLogger;
use tokio::sync::watch;
use std::error::Error;
use std::future::pending;

use crate::gamepad::watcher::{Watcher, WatchEventType, WatchEvent};

mod gamepad;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().init().unwrap();
    log::info!("Starting hanDBus");

    // Create a message channel that the [Watcher] will send events to.
    let (tx, mut rx) = watch::channel(&WatchEvent::new(String::from("/"), WatchEventType::Added));

    // Spawn the watcher to watch for gamepad changes
    let watcher_handle = tokio::spawn(async {
        log::debug!("Starting watcher");
        let watcher = Watcher::new(tx);
        watcher.watch().await;
    });

    // Listen for watch events
    tokio::spawn(async move {
        // Use the equivalent of a "do-while" loop so the initial value is
        // processed before awaiting the `changed()` future.
        loop {
            println!("Got message: {} ", *rx.borrow_and_update());
            if rx.changed().await.is_err() {
                break;
            }
        }
    });

    // Wait for the watcher to finish
    let _ = watcher_handle.await.unwrap();
        
    // Do other things or go to wait forever
    pending::<()>().await;

    Ok(())
}
