use std::error::Error;

use crate::{iio::device::Device, input::source::client::SourceDeviceClient};
use zbus::{fdo, Connection};
use zbus_macros::interface;

use crate::input::source::iio::get_dbus_path;

/// DBusInterface exposing information about a HIDRaw device
pub struct SourceIioImuInterface {
    _info: Device,
    source_device: SourceDeviceClient,
}

impl SourceIioImuInterface {
    pub fn new(info: Device, source_device: SourceDeviceClient) -> SourceIioImuInterface {
        SourceIioImuInterface {
            _info: info,
            source_device,
        }
    }

    /// Creates a new instance of the source hidraw interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(
        conn: Connection,
        info: Device,
        source_device: SourceDeviceClient,
    ) -> Result<(), Box<dyn Error>> {
        let Some(id) = info.id.clone() else {
            return Err("Failed to get ID of IIO device".into());
        };
        let path = get_dbus_path(id);

        let iface = SourceIioImuInterface::new(info, source_device);
        tokio::task::spawn(async move {
            log::debug!("Starting dbus interface: {path}");
            let result = conn.object_server().at(path.clone(), iface).await;
            if let Err(e) = result {
                log::debug!("Failed to start dbus interface {path}: {e:?}");
            } else {
                log::debug!("Started dbus interface: {path}");
            }
        });
        Ok(())
    }
}

#[interface(name = "org.shadowblip.Input.Source.IIOIMUDevice")]
impl SourceIioImuInterface {
    #[zbus(property)]
    async fn accel_sample_rate(&self) -> fdo::Result<f64> {
        match self.source_device.get_sample_rate("accel").await {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn accel_sample_rates_avail(&self) -> fdo::Result<Vec<f64>> {
        match self.source_device.get_sample_rates_avail("accel").await {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn angvel_sample_rate(&self) -> fdo::Result<f64> {
        match self.source_device.get_sample_rate("gyro").await {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn angvel_sample_rates_avail(&self) -> fdo::Result<Vec<f64>> {
        match self.source_device.get_sample_rates_avail("gyro").await {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn set_accel_sample_rate(&self, sample_rate: f64) -> zbus::Result<()> {
        match self
            .source_device
            .set_sample_rate("accel", sample_rate)
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn set_angvel_sample_rate(&self, sample_rate: f64) -> zbus::Result<()> {
        match self
            .source_device
            .set_sample_rate("gyro", sample_rate)
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }
    //
    #[zbus(property)]
    async fn accel_scale(&self) -> fdo::Result<f64> {
        match self.source_device.get_scale("accel").await {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn accel_scales_avail(&self) -> fdo::Result<Vec<f64>> {
        match self.source_device.get_scales_available("accel").await {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn angvel_scale(&self) -> fdo::Result<f64> {
        match self.source_device.get_scale("gyro").await {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn angvel_scales_avail(&self) -> fdo::Result<Vec<f64>> {
        match self.source_device.get_scales_available("gyro").await {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn set_accel_scale(&self, scale: f64) -> zbus::Result<()> {
        match self.source_device.set_scale("accel", scale).await {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn set_angvel_scale(&self, scale: f64) -> zbus::Result<()> {
        match self.source_device.set_scale("gyro", scale).await {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }
}
