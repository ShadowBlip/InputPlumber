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
use opentelemetry::global;
use opentelemetry_sdk::propagation::TraceContextPropagator;

use std::{fs::File, io::BufWriter};
use tracing_flame::FlameLayer;
use tracing_subscriber::{fmt, prelude::*, registry::Registry};

mod config;
mod constants;
mod dbus;
mod dmi;
mod drivers;
mod iio;
mod input;
mod procfs;
mod udev;
mod watcher;

fn setup_tracing() {
    let (flame_layer, _guard) = FlameLayer::with_file("/tmp/tracing.folded").unwrap();
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(flame_layer)
        .init();
    //tracing_subscriber::fmt()
    //    // enable everything
    //    .with_max_level(tracing::Level::TRACE)
    //    .compact()
    //    // Display source code file paths
    //    .with_file(true)
    //    // Display source code line numbers
    //    .with_line_number(true)
    //    // Display the thread ID an event was recorded on
    //    .with_thread_ids(true)
    //    // Don't display the event's target (module path)
    //    .with_target(false)
    //    // sets this to be the default, global collector for this application.
    //    .init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    setup_tracing();
    //let log_level = match env::var("LOG_LEVEL") {
    //    Ok(value) => value,
    //    Err(_) => "info".to_string(),
    //};
    //env::set_var("RUST_LOG", log_level);
    //env_logger::init();
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

    log::info!("InputPlumber stopped");

    Ok(())
}
