use std::time::Duration;

use tokio::sync::mpsc;
use zbus::{fdo, Connection};
use zbus_macros::interface;

use crate::{
    config::CompositeDeviceConfig,
    constants::BUS_PREFIX,
    input::{manager::ManagerCommand, target::TargetDeviceTypeId},
};

/// The [ManagerInterface] provides a DBus interface that can be exposed for managing
/// a [Manager]. It works by sending command messages to a channel that the
/// [Manager] is listening on.
pub struct ManagerInterface {
    tx: mpsc::Sender<ManagerCommand>,
    gamepad_order: Vec<String>,
}

impl ManagerInterface {
    pub fn new(tx: mpsc::Sender<ManagerCommand>) -> ManagerInterface {
        ManagerInterface {
            tx,
            gamepad_order: Default::default(),
        }
    }
}

#[interface(
    name = "org.shadowblip.InputManager",
    proxy(
        default_service = "org.shadowblip.InputPlumber",
        default_path = "/org/shadowblip/InputPlumber/Manager"
    )
)]
impl ManagerInterface {
    #[zbus(property)]
    async fn version(&self) -> fdo::Result<String> {
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        Ok(VERSION.to_string())
    }

    #[zbus(property)]
    async fn gamepad_order(&self) -> fdo::Result<Vec<String>> {
        Ok(self.gamepad_order.clone())
    }

    #[zbus(property)]
    async fn set_gamepad_order(&mut self, order: Vec<String>) -> fdo::Result<()> {
        self.tx
            .send_timeout(
                ManagerCommand::SetGamepadOrder {
                    dbus_paths: order.clone(),
                },
                Duration::from_millis(500),
            )
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        self.gamepad_order = order;

        Ok(())
    }

    /// If set to 'true', InputPlumber will try to manage all input devices
    /// on the system that have a Composite Device configuration.
    #[zbus(property)]
    async fn manage_all_devices(&self) -> fdo::Result<bool> {
        let (sender, mut receiver) = mpsc::channel(1);
        self.tx
            .send_timeout(
                ManagerCommand::GetManageAllDevices { sender },
                Duration::from_millis(500),
            )
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;

        // Read the response from the manager
        let Some(response) = receiver.recv().await else {
            return Err(fdo::Error::Failed("No response from manager".to_string()));
        };
        Ok(response)
    }
    #[zbus(property)]
    async fn set_manage_all_devices(&self, value: bool) -> zbus::Result<()> {
        self.tx
            .send_timeout(
                ManagerCommand::SetManageAllDevices(value),
                Duration::from_millis(500),
            )
            .await
            .map_err(|err| zbus::Error::Failure(err.to_string()))?;
        Ok(())
    }

    /// Returns a list of supported target device names. E.g. ["InputPlumber Mouse", "Microsoft
    /// XBox 360 Gamepad"]
    #[zbus(property)]
    fn supported_target_devices(&self) -> fdo::Result<Vec<String>> {
        let supported = TargetDeviceTypeId::supported_types();
        Ok(supported.iter().map(|id| id.name().to_string()).collect())
    }

    /// Returns a list of supported target device ids. E.g. ["xb360", "deck"]
    #[zbus(property)]
    fn supported_target_device_ids(&self) -> fdo::Result<Vec<String>> {
        let supported = TargetDeviceTypeId::supported_types();
        Ok(supported.iter().map(|id| id.to_string()).collect())
    }

    /// Create a composite device using the give composite device config. The
    /// path should be the absolute path to a composite device configuration file.
    async fn create_composite_device(&self, config_path: String) -> fdo::Result<String> {
        let device = CompositeDeviceConfig::from_yaml_file(config_path)
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        self.tx
            .send_timeout(
                ManagerCommand::CreateCompositeDevice { config: device },
                Duration::from_millis(500),
            )
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        Ok("".to_string())
    }

    /// Create a target device of the given type. Returns the DBus path to
    /// the created target device.
    async fn create_target_device(&self, kind: String) -> fdo::Result<String> {
        let (sender, mut receiver) = mpsc::channel(1);
        self.tx
            .send_timeout(
                ManagerCommand::CreateTargetDevice { kind, sender },
                Duration::from_millis(500),
            )
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;

        // Read the response from the manager
        let Some(response) = receiver.recv().await else {
            return Err(fdo::Error::Failed("No response from manager".to_string()));
        };
        let device_path = match response {
            Ok(path) => path,
            Err(e) => {
                let err = format!("Failed to create target device: {e:?}");
                return Err(fdo::Error::Failed(err));
            }
        };

        Ok(device_path)
    }

    /// Stop the given target device
    async fn stop_target_device(&self, path: String) -> fdo::Result<()> {
        self.tx
            .send_timeout(
                ManagerCommand::StopTargetDevice { path },
                Duration::from_millis(500),
            )
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        Ok(())
    }

    /// Attach the given target device to the given composite device
    async fn attach_target_device(
        &self,
        target_path: String,
        composite_path: String,
    ) -> fdo::Result<()> {
        let (sender, mut receiver) = mpsc::channel(1);
        self.tx
            .send_timeout(
                ManagerCommand::AttachTargetDevice {
                    target_path: target_path.clone(),
                    composite_path: composite_path.clone(),
                    sender,
                },
                Duration::from_millis(500),
            )
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;

        // Read the response from the manager
        let Some(response) = receiver.recv().await else {
            return Err(fdo::Error::Failed("No response from manager".to_string()));
        };
        if let Err(e) = response {
            let err = format!("Failed to attach target device {target_path} to composite device {composite_path}: {e:?}");
            return Err(fdo::Error::Failed(err));
        }

        Ok(())
    }

    /// Used to prepare InputPlumber for system suspend
    async fn hook_sleep(&self) -> fdo::Result<()> {
        let (sender, mut receiver) = mpsc::channel(1);
        self.tx
            .send_timeout(
                ManagerCommand::SystemSleep { sender },
                Duration::from_secs(5),
            )
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;

        // Read the response from the manager
        if receiver.recv().await.is_none() {
            return Err(fdo::Error::Failed("No response from manager".to_string()));
        }

        Ok(())
    }

    /// Used to prepare InputPlumber for resume from system suspend
    async fn hook_wake(&self) -> fdo::Result<()> {
        let (sender, mut receiver) = mpsc::channel(1);
        self.tx
            .send_timeout(
                ManagerCommand::SystemWake { sender },
                Duration::from_secs(5),
            )
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;

        // Read the response from the manager
        if receiver.recv().await.is_none() {
            return Err(fdo::Error::Failed("No response from manager".to_string()));
        }

        Ok(())
    }
}

impl ManagerInterface {
    /// Update the target gamepad order property and emit property changed signal
    pub async fn update_target_gamepad_order(
        conn: &Connection,
        order: Vec<String>,
    ) -> Result<(), zbus::Error> {
        let path = format!("{BUS_PREFIX}/Manager");
        let iface_ref = conn.object_server().interface::<_, Self>(path).await?;

        let mut iface = iface_ref.get_mut().await;
        iface.gamepad_order = order;
        iface
            .gamepad_order_changed(iface_ref.signal_emitter())
            .await?;

        Ok(())
    }
}
