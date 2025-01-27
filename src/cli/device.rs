use std::error::Error;
use std::fmt::Display;
use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};
use tabled::settings::{Panel, Style};
use tabled::{Table, Tabled};
use zbus::Connection;

use crate::cli::get_managed_objects;
use crate::dbus::interface::composite_device::CompositeDeviceInterfaceProxy;
use crate::dbus::interface::manager::ManagerInterfaceProxy;

#[derive(Subcommand, Debug, Clone)]
pub enum DeviceCommand {
    /// Display information about the composite device
    Info,
    /// Get the capabilities of the composite device
    Capabilities,
    /// Load the input profile from the given path
    LoadProfile { path: String },
    /// Stop InputPlumber from managing the device
    Stop,
    /// Manage the intercept mode of the composite device.
    InterceptMode { mode: Option<InterceptMode> },
}

#[derive(ValueEnum, Debug, Clone)]
pub enum InterceptMode {
    /// No inputs are intercepted and re-routed
    None,
    /// No inputs are intercepted and re-routed except for gamepad Guide events. Upon receiving a gamepad Guide event, the device is automatically switched to intercept mode ALL.
    Pass,
    /// All inputs are intercepted and re-routed over DBus
    All,
    /// All gamepad inputs are intercepted and re-routed over DBus
    GamepadOnly,
}

impl From<u32> for InterceptMode {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Pass,
            2 => Self::All,
            3 => Self::GamepadOnly,
            _ => Self::None,
        }
    }
}

impl From<InterceptMode> for u32 {
    fn from(value: InterceptMode) -> Self {
        match value {
            InterceptMode::None => 0,
            InterceptMode::Pass => 1,
            InterceptMode::All => 2,
            InterceptMode::GamepadOnly => 3,
        }
    }
}

impl Display for InterceptMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            InterceptMode::None => "none",
            InterceptMode::Pass => "pass",
            InterceptMode::All => "all",
            InterceptMode::GamepadOnly => "gamepad-only",
        };
        write!(f, "{}", value)
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum DevicesCommand {
    /// List all running composite devices
    List,
    /// Enable/disable managing all supported input devices
    ManageAll {
        #[arg(long, action)]
        enable: bool,
    },
}

#[derive(Tabled)]
struct DeviceRow {
    #[tabled(rename = "Id")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
}

#[derive(Tabled)]
struct DeviceInfo {
    #[tabled(rename = "Id")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Profile Name")]
    profile_name: String,
    #[tabled(rename = "Source Devices")]
    sources: String,
}

pub async fn handle_device(
    conn: Connection,
    cmd: DeviceCommand,
    num: u32,
) -> Result<(), Box<dyn Error>> {
    // Build the path to the composite device
    let path = format!("/org/shadowblip/InputPlumber/CompositeDevice{num}");
    let paths = get_managed_objects(conn.clone()).await?;
    if !paths.contains(&path) {
        return Err(format!("Composite device does not exist with number: {num}").into());
    }

    let device = CompositeDeviceInterfaceProxy::builder(&conn)
        .path(path.clone())
        .unwrap()
        .build()
        .await;
    let Some(device) = device.ok() else {
        return Ok(());
    };

    match cmd {
        DeviceCommand::Info => {
            // Get the source devices for this device
            let name = device.name().await.unwrap_or_default();
            let sources = device.source_device_paths().await.unwrap_or_default();
            let profile_name = device.profile_name().await.unwrap_or_default();

            let entry = DeviceInfo {
                id: format!("{num}"),
                name,
                profile_name,
                sources: format!("{sources:?}"),
            };
            let mut table = Table::new(vec![entry]);
            table
                .with(Style::modern_rounded())
                .with(Panel::header("Composite Device"));
            println!("{table}");
        }
        DeviceCommand::Capabilities => {
            let caps = device.capabilities().await.unwrap_or_default();
            println!("{caps:?}");
        }
        DeviceCommand::LoadProfile { path } => {
            let path_buf = PathBuf::from(path.clone());
            if !path_buf.exists() {
                return Err(format!("No input profile exists at path: {path}").into());
            }
            let abs_path = std::fs::canonicalize(&path_buf)
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if let Err(e) = device.load_profile_path(abs_path).await {
                return Err(format!("Failed to load input profile {path}: {e:?}").into());
            }
            println!("Successfully loaded profile: {path}");
        }
        DeviceCommand::Stop => {
            device.stop().await?;
            println!("Stopped device {num}");
        }
        DeviceCommand::InterceptMode { mode } => {
            if let Some(mode) = mode {
                device.set_intercept_mode(mode.clone().into()).await?;
                println!("Set intercept mode to: {mode}");
                return Ok(());
            }
            let mode: InterceptMode = device.intercept_mode().await.unwrap_or_default().into();
            println!("Current intercept mode: {mode}");
        }
    }

    Ok(())
}

pub async fn handle_devices(conn: Connection, cmd: DevicesCommand) -> Result<(), Box<dyn Error>> {
    match cmd {
        DevicesCommand::List => {
            let paths = get_managed_objects(conn.clone()).await?;
            let mut device_paths: Vec<String> = paths
                .into_iter()
                .filter(|obj| obj.contains("/CompositeDevice"))
                .collect();
            device_paths.sort();
            let count = device_paths.len();

            // Query information about each device
            let mut devices = Vec::with_capacity(device_paths.len());
            for path in device_paths {
                let device = CompositeDeviceInterfaceProxy::builder(&conn)
                    .path(path.clone())
                    .unwrap()
                    .build()
                    .await;
                let Some(device) = device.ok() else {
                    continue;
                };

                let number = path.replace("/org/shadowblip/InputPlumber/CompositeDevice", "");
                let name = device.name().await.unwrap_or_default();

                let row = DeviceRow { id: number, name };

                devices.push(row);
            }

            let mut table = Table::new(devices);
            table
                .with(Style::modern_rounded())
                .with(Panel::header("Composite Devices"));
            println!("{table}");
            println!("Found {count} composite device(s)");
        }
        DevicesCommand::ManageAll { enable } => {
            let manager = ManagerInterfaceProxy::builder(&conn).build().await?;
            manager.set_manage_all_devices(enable).await?;
            let verb = if enable { "Enabled" } else { "Disabled" };
            println!("{verb} management of all supported devices");
        }
    }

    Ok(())
}
