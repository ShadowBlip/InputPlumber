use std::error::Error;

use clap::Subcommand;
use tabled::settings::{Panel, Style};
use tabled::{Table, Tabled};
use zbus::Connection;

use crate::cli::get_managed_objects;
use crate::dbus::interface::source::udev::SourceUdevDeviceInterfaceProxy;

#[derive(Subcommand, Debug, Clone)]
pub enum SourcesCommand {
    /// List all discovered source devices
    List,
}

#[derive(Tabled)]
struct SourceDeviceRow {
    path: String,
    name: String,
    subsystem: String,
}

pub async fn handle_sources(conn: Connection, cmd: SourcesCommand) -> Result<(), Box<dyn Error>> {
    match cmd {
        SourcesCommand::List => {
            let objects = get_managed_objects(conn.clone()).await?;
            let mut paths: Vec<String> = objects
                .into_iter()
                .filter(|obj| obj.contains("/devices/source/"))
                .collect();
            paths.sort();
            let count = paths.len();

            // Query information about each device
            let mut source_devices = Vec::with_capacity(paths.len());
            for path in paths {
                let device = SourceUdevDeviceInterfaceProxy::builder(&conn)
                    .path(path.clone())
                    .unwrap()
                    .build()
                    .await;
                let Some(device) = device.ok() else {
                    continue;
                };

                let name = device.name().await.unwrap_or_default();
                let subsystem = device.subsystem().await.unwrap_or_default();
                let path = device.device_path().await.unwrap_or_default();

                let row = SourceDeviceRow {
                    path,
                    name,
                    subsystem,
                };

                source_devices.push(row);
            }

            let mut table = Table::new(source_devices);
            table
                .with(Style::modern_rounded())
                .with(Panel::header("Source Devices"));
            println!("{table}");
            println!("Found {count} source device(s)");
        }
    }

    Ok(())
}
