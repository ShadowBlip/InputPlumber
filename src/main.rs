use clap::Parser;
use std::env;
use std::error::Error;
use std::process;
use tokio::signal::unix::SignalKind;
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

    // Request the named bus
    if let Err(err) = connection.request_name(BUS_NAME).await {
        log::error!("Error requesting dbus name: {err}");
        process::exit(-1);
    }

    // Create an InputManager instance
    let mut input_manager = Manager::new(connection.clone());

    // Setup signal handlers
    let mut sig_term = tokio::signal::unix::signal(SignalKind::terminate())?;
    let mut sig_int = tokio::signal::unix::signal(SignalKind::interrupt())?;

    // Start the main run loop
    let mut exit_code = 0;
    tokio::select! {
        // Start the input manager and listen on DBus
        result = input_manager.run() => {
            if let Err(err) = result {
                log::error!("Error running input manager: {err}");
                exit_code = -1;
            }
        },
        // Setup CTRL+C handler
        _ = tokio::signal::ctrl_c() => {
            log::info!("Received CTRL+C. Shutting down.");
        },
        // Setup SIGINT handler
        _ = sig_int.recv() => {
            log::info!("Received SIGINT. Shutting down.");
        },
        // Setup SIGTERM handler
        _ = sig_term.recv() => {
            log::info!("Received SIGTERM. Shutting down.");
        }
    }

    // Unhide all devices on shutdown
    if let Err(e) = unhide_all().await {
        log::error!("Unable to un-hide devices: {:?}", e);
    }

    log::info!("InputPlumber stopped");
    process::exit(exit_code);
}
