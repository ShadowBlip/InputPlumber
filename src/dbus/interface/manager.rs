use tokio::sync::{broadcast, mpsc};
use zbus::fdo;
use zbus_macros::interface;

use crate::{config::CompositeDeviceConfig, input::manager::ManagerCommand};

/// The [ManagerInterface] provides a DBus interface that can be exposed for managing
/// a [Manager]. It works by sending command messages to a channel that the
/// [Manager] is listening on.
pub struct ManagerInterface {
    tx: broadcast::Sender<ManagerCommand>,
}

impl ManagerInterface {
    pub fn new(tx: broadcast::Sender<ManagerCommand>) -> ManagerInterface {
        ManagerInterface { tx }
    }
}

#[interface(name = "org.shadowblip.InputManager")]
impl ManagerInterface {
    #[zbus(property)]
    async fn intercept_mode(&self) -> fdo::Result<String> {
        Ok("InputPlumber".to_string())
    }

    /// Create a composite device using the give composite device config. The
    /// path should be the absolute path to a composite device configuration file.
    async fn create_composite_device(&self, config_path: String) -> fdo::Result<String> {
        let device = CompositeDeviceConfig::from_yaml_file(config_path)
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        self.tx
            .send(ManagerCommand::CreateCompositeDevice { config: device })
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        Ok("".to_string())
    }

    /// Create a target device of the given type. Returns the DBus path to
    /// the created target device.
    async fn create_target_device(&self, kind: String) -> fdo::Result<String> {
        let (sender, mut receiver) = mpsc::channel(1);
        self.tx
            .send(ManagerCommand::CreateTargetDevice { kind, sender })
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
            .send(ManagerCommand::StopTargetDevice { path })
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
            .send(ManagerCommand::AttachTargetDevice {
                target_path: target_path.clone(),
                composite_path: composite_path.clone(),
                sender,
            })
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
}
