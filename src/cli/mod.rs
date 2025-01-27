pub mod device;
pub mod source;
pub mod target;

use std::error::Error;

use clap::{Parser, Subcommand};
use device::{handle_device, handle_devices, DeviceCommand, DevicesCommand};
use source::{handle_sources, SourcesCommand};
use target::{handle_targets, TargetsCommand};
use zbus::fdo::ObjectManagerProxy;
use zbus::{names::BusName, Connection};

use crate::constants::{BUS_NAME, BUS_PREFIX};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Start the InputPlumber daemon (default)
    Run,
    /// Manage source input devices
    Sources {
        #[command(subcommand)]
        cmd: SourcesCommand,
    },
    /// Manage a composite device
    Device {
        /// Composite device id
        id: u32,
        #[command(subcommand)]
        cmd: DeviceCommand,
    },
    /// Manage composite devices
    Devices {
        #[command(subcommand)]
        cmd: DevicesCommand,
    },
    /// Manage target input devices
    Targets {
        #[command(subcommand)]
        cmd: TargetsCommand,
    },
}

pub async fn main_cli(args: Args) -> Result<(), Box<dyn Error>> {
    let Some(cmd) = args.cmd else {
        return Ok(());
    };

    // Connect to DBus
    let connection = Connection::system().await?;
    if !is_running(&connection).await {
        return Err("InputPlumber daemon is not currently running".into());
    }

    match cmd {
        Commands::Run => (),
        Commands::Sources { cmd } => handle_sources(connection, cmd).await?,
        Commands::Device { id: number, cmd } => handle_device(connection, cmd, number).await?,
        Commands::Devices { cmd } => handle_devices(connection, cmd).await?,
        Commands::Targets { cmd } => handle_targets(connection, cmd).await?,
    }

    Ok(())
}

/// Returns true if InputPlumber is currently running
async fn is_running(conn: &Connection) -> bool {
    let bus = BusName::from_static_str(BUS_NAME).unwrap();
    let dbus = zbus::fdo::DBusProxy::new(conn).await.ok();
    let Some(dbus) = dbus else {
        return false;
    };
    dbus.name_has_owner(bus.clone()).await.unwrap_or_default()
}

/// Returns a list of dbus paths of all objects
pub async fn get_managed_objects(conn: Connection) -> Result<Vec<String>, Box<dyn Error>> {
    let bus = BusName::from_static_str(BUS_NAME).unwrap();
    let object_manager: ObjectManagerProxy = ObjectManagerProxy::builder(&conn)
        .destination(bus)?
        .path(BUS_PREFIX)?
        .build()
        .await?;

    let objects: Vec<String> = object_manager
        .get_managed_objects()
        .await?
        .keys()
        .map(|v| v.to_string())
        .collect();

    Ok(objects)
}
