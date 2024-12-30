use std::env;
use std::error::Error;
use std::process;
use zbus::fdo::ObjectManager;
use zbus::Connection;

use crate::constants::BUS_NAME;
use crate::constants::BUS_PREFIX;
use crate::input::manager::Manager;
use crate::udev::unhide_all;

mod bluetooth;
mod config;
mod constants;
mod dbus;
mod dmi;
mod drivers;
mod iio;
mod input;
mod udev;
mod watcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
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

    let (input_man_result, request_name_result) = tokio::join!(
        // Start the input manager and listen on DBus
        input_manager.run(),
        // Request the named bus
        connection.request_name(BUS_NAME)
    );

    match input_man_result {
        Ok(_) => {
            log::info!("The input manager task has exited");
        }
        Err(input_man_err) => {
            log::error!("Error in joining the input manager task: {input_man_err}");
            return Err(input_man_err);
        }
    }

    match request_name_result {
        Ok(_) => {
            log::info!("The input manager task has exited");
        }
        Err(request_name_err) => {
            log::error!("Error in dbus request name operation: {request_name_err}");
            return Err(Box::new(request_name_err));
        }
    }

    log::info!("InputPlumber stopped");

    Ok(())
}
