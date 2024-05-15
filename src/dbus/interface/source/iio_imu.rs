use std::error::Error;

use crate::iio::device::Device;
use tokio::sync::mpsc::Sender;
use zbus::{fdo, Connection};
use zbus_macros::interface;

use crate::input::source::{iio::get_dbus_path, SourceCommand};

/// DBusInterface exposing information about a HIDRaw device
pub struct SourceIioImuInterface {
    info: Device,
    tx: Sender<SourceCommand>,
}

impl SourceIioImuInterface {
    pub fn new(info: Device, tx: Sender<SourceCommand>) -> SourceIioImuInterface {
        SourceIioImuInterface { info, tx }
    }

    /// Creates a new instance of the source hidraw interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(
        conn: Connection,
        info: Device,
        tx: Sender<SourceCommand>,
    ) -> Result<(), Box<dyn Error>> {
        let Some(id) = info.id.clone() else {
            return Err("Failed to get ID of IIO device".into());
        };
        let path = get_dbus_path(id);

        let iface = SourceIioImuInterface::new(info, tx);
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
        let (tx, rx) = std::sync::mpsc::channel();
        if let Err(e) = self
            .tx
            .send(SourceCommand::GetSampleRate("accel".to_string(), tx))
            .await
        {
            return Err(fdo::Error::Failed(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(fdo::Error::Failed(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn accel_sample_rates_avail(&self) -> fdo::Result<Vec<f64>> {
        let (tx, rx) = std::sync::mpsc::channel();
        if let Err(e) = self
            .tx
            .send(SourceCommand::GetSampleRatesAvail("accel".to_string(), tx))
            .await
        {
            return Err(fdo::Error::Failed(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(fdo::Error::Failed(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn angvel_sample_rate(&self) -> fdo::Result<f64> {
        let (tx, rx) = std::sync::mpsc::channel();
        if let Err(e) = self
            .tx
            .send(SourceCommand::GetSampleRate("gyro".to_string(), tx))
            .await
        {
            return Err(fdo::Error::Failed(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(fdo::Error::Failed(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn angvel_sample_rates_avail(&self) -> fdo::Result<Vec<f64>> {
        let (tx, rx) = std::sync::mpsc::channel();
        if let Err(e) = self
            .tx
            .send(SourceCommand::GetSampleRatesAvail("gyro".to_string(), tx))
            .await
        {
            return Err(fdo::Error::Failed(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(fdo::Error::Failed(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn set_accel_sample_rate(&self, sample_rate: f64) -> zbus::Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();

        if let Err(e) = self
            .tx
            .send(SourceCommand::SetSampleRate(
                "accel".to_string(),
                sample_rate,
                tx,
            ))
            .await
        {
            return Err(zbus::Error::Failure(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(zbus::Error::Failure(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn set_angvel_sample_rate(&self, sample_rate: f64) -> zbus::Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();

        if let Err(e) = self
            .tx
            .send(SourceCommand::SetSampleRate(
                "gyro".to_string(),
                sample_rate,
                tx,
            ))
            .await
        {
            return Err(zbus::Error::Failure(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(zbus::Error::Failure(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }
    //
    #[zbus(property)]
    async fn accel_scale(&self) -> fdo::Result<f64> {
        let (tx, rx) = std::sync::mpsc::channel();
        if let Err(e) = self
            .tx
            .send(SourceCommand::GetScale("accel".to_string(), tx))
            .await
        {
            return Err(fdo::Error::Failed(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(fdo::Error::Failed(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn accel_scales_avail(&self) -> fdo::Result<Vec<f64>> {
        let (tx, rx) = std::sync::mpsc::channel();
        if let Err(e) = self
            .tx
            .send(SourceCommand::GetScalesAvail("accel".to_string(), tx))
            .await
        {
            return Err(fdo::Error::Failed(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(fdo::Error::Failed(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn angvel_scale(&self) -> fdo::Result<f64> {
        let (tx, rx) = std::sync::mpsc::channel();
        if let Err(e) = self
            .tx
            .send(SourceCommand::GetScale("gyro".to_string(), tx))
            .await
        {
            return Err(fdo::Error::Failed(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(fdo::Error::Failed(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn angvel_scales_avail(&self) -> fdo::Result<Vec<f64>> {
        let (tx, rx) = std::sync::mpsc::channel();
        if let Err(e) = self
            .tx
            .send(SourceCommand::GetScalesAvail("gyro".to_string(), tx))
            .await
        {
            return Err(fdo::Error::Failed(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(fdo::Error::Failed(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(fdo::Error::Failed(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn set_accel_scale(&self, scale: f64) -> zbus::Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();

        if let Err(e) = self
            .tx
            .send(SourceCommand::SetScale("accel".to_string(), scale, tx))
            .await
        {
            return Err(zbus::Error::Failure(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(zbus::Error::Failure(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn set_angvel_scale(&self, scale: f64) -> zbus::Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();

        if let Err(e) = self
            .tx
            .send(SourceCommand::SetScale("gyro".to_string(), scale, tx))
            .await
        {
            return Err(zbus::Error::Failure(e.to_string()));
        }
        let Ok(response) = rx.recv() else {
            return Err(zbus::Error::Failure(
                "Channel closed with no response.".to_string(),
            ));
        };
        match response {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }
}
