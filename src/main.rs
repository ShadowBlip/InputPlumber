use clap::Parser;
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
mod cli;
mod config;
mod constants;
mod dbus;
mod dmi;
mod drivers;
mod input;
mod sync;
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

    // If there are any subcommands, run as a CLI client instead.
    let args = cli::Args::parse();
    if let Some(cmd) = args.cmd.as_ref() {
        if !matches!(cmd, cli::Commands::Run) {
            cli::main_cli(args).await?;
            return Ok(());
        }
    }

    log::info!("Starting InputPlumber v{}", VERSION);

    // Unhide any devices previously hidden by InputPlumber. This can happen
    // if InputPlumber is killed before it can restore the devices.
    if let Err(e) = unhide_all().await {
        log::debug!("Failed to unhide devices at startup: {e}");
    }

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

    let (ctrl_c_result, input_man_result, request_name_result) = tokio::join!(
        // Setup CTRL+C handler
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            log::info!("Un-hiding all devices");
            if let Err(e) = unhide_all().await {
                log::error!("Unable to un-hide devices: {:?}", e);
            }
            log::info!("Shutting down");
            process::exit(0);
        }),
        // Start the input manager and listen on DBus
        input_manager.run(),
        // Request the named bus
        connection.request_name(BUS_NAME)
    );

    match ctrl_c_result {
        Ok(_) => {
            log::info!("The input manager task has exited");
        }
        Err(err) => {
            log::error!("Error in joining ctrl+C watcher: {err}");
            return Err(Box::new(err) as Box<dyn Error>);
        }
    }

    match request_name_result {
        Ok(_) => {
            log::info!("The input manager task has exited");
        }
        Err(err) => {
            log::error!("Error in joining dbus request name operation: {err}");
            return Err(Box::new(err));
        }
    };

    match input_man_result {
        Ok(_) => {
            log::info!("The input manager task has exited");
        }
        Err(err) => {
            log::error!("Error in joining ctrl+C watcher: {err}");
            return Err(err);
        }
    };

    log::info!("InputPlumber stopped");

    Ok(())
}
