use std::error::Error;

use clap::Subcommand;
use tabled::settings::{Panel, Style};
use tabled::{Table, Tabled};
use zbus::Connection;

use crate::cli::get_managed_objects;
use crate::input::target::TargetDeviceTypeId;

#[derive(Subcommand, Debug, Clone)]
pub enum TargetsCommand {
    /// List all discovered target devices
    List,
    /// List all supported target devices
    SupportedDevices,
}

#[derive(Tabled)]
struct TargetDeviceRow {
    #[tabled(rename = "DBus Path")]
    path: String,
}

#[derive(Tabled)]
struct SupportedTargetRow {
    #[tabled(rename = "Id")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
}

pub async fn handle_targets(conn: Connection, cmd: TargetsCommand) -> Result<(), Box<dyn Error>> {
    match cmd {
        TargetsCommand::List => {
            let objects = get_managed_objects(conn).await?;
            let mut target_devices: Vec<String> = objects
                .into_iter()
                .filter(|obj| obj.contains("/devices/target/"))
                .collect();
            target_devices.sort();
            let count = target_devices.len();

            let target_devices: Vec<TargetDeviceRow> = target_devices
                .into_iter()
                .map(|path| {
                    let dbus_path = path;
                    TargetDeviceRow { path: dbus_path }
                })
                .collect();

            let mut table = Table::new(target_devices);
            table
                .with(Style::modern_rounded())
                .with(Panel::header("Target Devices"));
            println!("{table}");
            println!("Found {count} target device(s)");
        }
        TargetsCommand::SupportedDevices => {
            let supported = TargetDeviceTypeId::supported_types();
            let supported: Vec<SupportedTargetRow> = supported
                .into_iter()
                .map(|id| SupportedTargetRow {
                    name: id.name().to_string(),
                    id: id.to_string(),
                })
                .collect();
            let count = supported.len();

            let mut table = Table::new(supported);
            table
                .with(Style::modern_rounded())
                .with(Panel::header("Supported Target Devices"));
            println!("{table}");
            println!("Found {count} supported target devices");
        }
    }

    Ok(())
}
