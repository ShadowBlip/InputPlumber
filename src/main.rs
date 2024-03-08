use std::env;
use std::error::Error;
use std::future::pending;
use std::process;
use zbus::fdo::ObjectManager;
use zbus::Connection;

use crate::constants::BUS_NAME;
use crate::constants::BUS_PREFIX;
use crate::input::manager::Manager;
use crate::udev::unhide_all;

mod config;
mod constants;
mod dmi;
mod drivers;
mod input;
mod procfs;
mod udev;
mod watcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let log_level = match env::var("LOG_LEVEL") {
        Ok(value) => value,
        Err(_) => "info".to_string(),
    };
    env::set_var("RUST_LOG", log_level);
    env_logger::init();
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    log::info!("Starting InputPlumber v{}", VERSION);

    // Setup CTRL+C handler
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        log::info!("Un-hiding all devices");
        if let Err(e) = unhide_all().await {
            log::error!("Unable to un-hide devices: {:?}", e);
        }
        log::info!("Shutting down");
        process::exit(0);
    });

    // Configure the DBus connection
    let connection = Connection::system().await?;

    // Create an ObjectManager to signal when objects are added/removed
    let object_manager = ObjectManager {};
    let object_manager_path = String::from(BUS_PREFIX);
    connection
        .object_server()
        .at(object_manager_path, object_manager)
        .await?;

    // Create an InputManager instance
    let mut input_manager = Manager::new(connection.clone());

    // Start the input manager and listen on DBus
    tokio::spawn(async move {
        log::debug!("Starting input manager thread");
        if let Err(e) = input_manager.run().await {
            log::error!("Error running input manager: {:?}", e);
        }
    });

    // Request the named bus
    connection.request_name(BUS_NAME).await?;

    // Do other things or go to wait forever
    pending::<()>().await;

    Ok(())
}
