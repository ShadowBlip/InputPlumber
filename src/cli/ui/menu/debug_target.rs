use std::{collections::HashMap, error::Error, time::Duration};

use futures::StreamExt;
use thiserror::Error;
use tokio::sync::mpsc;
use zbus::{fdo::ObjectManagerProxy, Connection};

use crate::{
    config::path::get_profiles_path,
    constants::{BUS_NAME, BUS_PREFIX},
    dbus::interface::{
        composite_device::CompositeDeviceInterfaceProxy, target::debug::TargetDebugInterfaceProxy,
        target::TargetInterfaceProxy,
    },
    input::target::TargetDeviceTypeId,
};

/// What can go wrong while setting a device up for testing
#[derive(Debug, Error)]
pub enum DebugTargetError {
    #[error("target device type {0} is not supported")]
    UnsupportedTargetType(String),
}

/// Name a saved target device type, which the daemon reports as a string
fn target_device_type(id: &str) -> Result<TargetDeviceTypeId, DebugTargetError> {
    match TargetDeviceTypeId::try_from(id) {
        Ok(device_type) => Ok(device_type),
        Err(_) => Err(DebugTargetError::UnsupportedTargetType(id.to_string())),
    }
}

/// State of a composite device that a testing menu temporarily replaces.
#[derive(Debug, Default, Clone)]
pub struct SavedState {
    target_device_types: Vec<TargetDeviceTypeId>,
    profile: Option<String>,
    profile_path: Option<String>,
    intercept_mode: u32,
    filtered_events: HashMap<String, Vec<String>>,
}

/// Live streams of the debug target's capability and input reports.
#[derive(Debug)]
pub struct DebugStream {
    pub rx_disconnect: mpsc::Receiver<()>,
    pub rx_reports: mpsc::Receiver<Vec<u8>>,
    pub rx_capabilities: mpsc::Receiver<Vec<u8>>,
}

/// Prepare the given composite device for testing.
///
/// Saves everything that is about to be changed so [restore] can put the
/// device back afterwards, then attaches the debug target, clears event
/// filters so every source capability is visible, and starts listening for
/// reports from the debug target.
pub async fn attach(
    conn: &Connection,
    dbus_path: &str,
) -> Result<(SavedState, DebugStream), Box<dyn Error>> {
    let device = CompositeDeviceInterfaceProxy::builder(conn)
        .path(dbus_path.to_string())?
        .build()
        .await?;

    // Save the current target devices so they can be restored
    let mut target_device_types = vec![];
    for path in device.target_devices().await? {
        let target_device = TargetInterfaceProxy::builder(conn)
            .path(path)?
            .build()
            .await?;
        let device_type = target_device.device_type().await?;
        let device_type = target_device_type(device_type.as_str())?;
        target_device_types.push(device_type);
    }

    // Save the current profile so it can be restored. A profile loaded from a path is
    // restored by path, so only an inline profile needs its text kept.
    let path = device.profile_path().await?;
    let profile_path = if path.is_empty() { None } else { Some(path) };
    let profile = if profile_path.is_some() {
        None
    } else {
        Some(device.get_profile_yaml().await?)
    };

    // Save the current intercept mode so it can be restored
    let intercept_mode = device.intercept_mode().await?;

    // Save the current event filters so they can be restored, then clear
    // them so every source's raw capabilities are visible during testing
    let filtered_events = device.filtered_events().await?;
    device.set_filtered_events(HashMap::new()).await?;

    // Add the debug target device if it does not exist
    if !target_device_types.iter().any(|t| t.as_str() == "debug") {
        let mut target_devices = vec!["debug".to_string()];
        target_devices.extend(
            target_device_types
                .iter()
                .map(|kind| kind.as_str().to_string()),
        );
        device.set_target_devices(target_devices).await?;
    }

    // Disable intercept mode
    if intercept_mode != 0 {
        device.set_intercept_mode(0).await?;
    }

    // Use the debug profile so no capability is remapped while testing
    let debug_profile = get_profiles_path().join("debug.yaml");
    device
        .load_profile_path(debug_profile.to_string_lossy().to_string())
        .await?;

    // Start listening for reports from the debug target
    let (tx_reports, rx_reports) = mpsc::channel(2048);
    let (tx_capabilities, rx_capabilities) = mpsc::channel(16);
    let (tx_disconnect, rx_disconnect) = mpsc::channel(1);

    let conn_clone = conn.clone();
    let path_clone = dbus_path.to_string();
    tokio::task::spawn(async move {
        if let Err(e) = listen_for_signals(
            &conn_clone,
            path_clone,
            tx_disconnect,
            tx_reports,
            tx_capabilities,
        )
        .await
        {
            log::warn!("Stopped listening for debug target signals: {e}");
        }
    });

    let saved = SavedState {
        target_device_types,
        profile,
        profile_path,
        intercept_mode,
        filtered_events,
    };

    Ok((
        saved,
        DebugStream {
            rx_disconnect,
            rx_reports,
            rx_capabilities,
        },
    ))
}

/// Restore a composite device that was prepared with [attach].
pub fn restore(conn: &Connection, dbus_path: &str, saved: &SavedState) {
    let conn = conn.clone();
    let dbus_path = dbus_path.to_string();
    let saved = saved.clone();
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    tokio::task::spawn(async move {
        let Ok(device) = CompositeDeviceInterfaceProxy::builder(&conn)
            .path(dbus_path)
            .unwrap()
            .build()
            .await
        else {
            log::warn!("Failed to create composite device proxy; cannot restore device state");
            if let Err(e) = tx.send(Err("failed to create composite device proxy".into())) {
                log::warn!("Failed to report device state restore failure: {e}");
            }
            return;
        };

        // Restore the profile
        let profile_result = if let Some(profile_path) = saved.profile_path {
            device.load_profile_path(profile_path).await
        } else if let Some(profile) = saved.profile {
            device.load_profile_from_yaml(profile).await
        } else {
            Ok(())
        };
        if let Err(e) = profile_result {
            log::warn!("Failed to restore profile: {e}");
        }

        // Restore the target devices of the device
        let target_devices = saved
            .target_device_types
            .into_iter()
            .map(|kind| kind.as_str().to_string())
            .collect();
        if let Err(e) = device.set_target_devices(target_devices).await {
            log::warn!("Failed to restore target devices: {e}");
        }

        // Restore the intercept mode
        if let Err(e) = device.set_intercept_mode(saved.intercept_mode).await {
            log::warn!("Failed to restore intercept mode: {e}");
        }

        // Restore the event filters
        if let Err(e) = device.set_filtered_events(saved.filtered_events).await {
            log::warn!("Failed to restore event filters: {e}");
        }

        if let Err(e) = tx.send(Ok(())) {
            log::warn!("Failed to report device state restore: {e}");
        }
    });

    // Wait up to 5s for the device state to be restored before exiting
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => log::warn!("Failed to restore device state: {e}"),
        Err(e) => log::warn!("Failed to restore device state within timeout: {e}"),
    }
}

/// Listen for debug target signals over dbus
async fn listen_for_signals(
    conn: &Connection,
    dbus_path: String,
    tx_disconnect: mpsc::Sender<()>,
    tx_reports: mpsc::Sender<Vec<u8>>,
    tx_capabilities: mpsc::Sender<Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    let device = CompositeDeviceInterfaceProxy::builder(conn)
        .path(dbus_path.to_string())?
        .build()
        .await?;

    // Wait until the debug target is available
    let mut debug_target_path = None;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        for path in device.target_devices().await? {
            if path.contains("/debug") {
                debug_target_path = Some(path);
            }
        }
        if debug_target_path.is_some() {
            break;
        }
    }
    let Some(target_path) = debug_target_path else {
        return Err("Failed to find target debug device!".to_string().into());
    };

    let debug_device = TargetDebugInterfaceProxy::builder(conn)
        .path(target_path)?
        .build()
        .await?;

    // Get the current capability report
    let capability_report = debug_device.input_capability_report().await?;
    tx_capabilities.send(capability_report).await?;

    // Listen for input reports
    let mut receive_report = debug_device.receive_input_report().await?;
    tokio::task::spawn(async move {
        while let Some(signal) = receive_report.next().await {
            let Ok(args) = signal.args() else { break };
            let _ = tx_reports.send(args.data).await;
        }
    });

    // Listen for capability changes
    let mut receive_caps = debug_device.receive_input_capability_report_changed().await;
    tokio::task::spawn(async move {
        while let Some(change) = receive_caps.next().await {
            let Ok(value) = change.get().await else { break };
            let _ = tx_capabilities.send(value).await;
        }
    });

    // Listen for the composite device disconnecting
    let object_manager = ObjectManagerProxy::builder(conn)
        .destination(BUS_NAME)?
        .path(BUS_PREFIX)?
        .build()
        .await?;
    let mut ifaces_removed = object_manager.receive_interfaces_removed().await?;
    tokio::task::spawn(async move {
        while let Some(change) = ifaces_removed.next().await {
            let Ok(args) = change.args() else { break };
            if args.object_path.as_str() != dbus_path.as_str() {
                continue;
            }
            let _ = tx_disconnect.send(()).await;
        }
    });

    Ok(())
}
