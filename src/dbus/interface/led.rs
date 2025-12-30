use std::{error::Error, future::Future};

use zbus::{fdo, message::Header, Connection};
use zbus_macros::interface;

use crate::dbus::polkit::check_polkit;

use super::Unregisterable;

/// [LedEmitter] is any device that can implement changing LED colors
pub trait LedEmitter {
    fn name(&self) -> impl Future<Output = Result<String, Box<dyn Error>>> + Send;
    fn get_colors_available(
        &self,
    ) -> impl Future<Output = Result<Vec<String>, Box<dyn Error>>> + Send;
    fn set_enabled(
        &mut self,
        enabled: bool,
    ) -> impl Future<Output = Result<(), Box<dyn Error>>> + Send;
    fn rumble(&mut self, value: f64) -> impl Future<Output = Result<(), Box<dyn Error>>> + Send;
    fn stop(&mut self) -> impl Future<Output = Result<(), Box<dyn Error>>> + Send;
}

/// DBus interface for a particular LED zone
pub struct LedInterface<T>
where
    T: LedEmitter + Send + Sync,
{
    device: T,
    name: String,
    colors: Vec<String>,
}

impl<T> LedInterface<T>
where
    T: LedEmitter + Send + Sync,
{
    /// Create a new dbus interface for the given LED zone
    pub fn new(device: T, name: String, colors: Vec<String>) -> Self {
        Self {
            device,
            name,
            colors,
        }
    }
}

#[interface(
    name = "org.shadowblip.Output.LED",
    proxy(default_service = "org.shadowblip.InputPlumber")
)]
impl<T> LedInterface<T>
where
    T: LedEmitter + Send + Sync + 'static,
{
    /// Name of the LED zone
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.name.clone())
    }

    /// Colors available for the LED zone
    #[zbus(property)]
    async fn colors(&self) -> fdo::Result<Vec<String>> {
        Ok(self.colors.clone())
    }
}

impl<T> Unregisterable for LedInterface<T> where T: LedEmitter + Send + Sync {}

/// DBus interface for controlling a single LED color
pub struct LedColorInterface {
    color: String,
    value: f64,
    brightness: f64,
}

impl LedColorInterface {
    /// Create a new dbus interface for the given color
    pub fn new(color: &str) -> Self {
        Self {
            color: color.to_string(),
            value: 0.0,
            brightness: 0.0,
        }
    }

    /// Update the color value
    pub fn update_value(conn: &Connection, path: &str, value: f64) {
        let conn = conn.clone();
        let path = path.to_string();
        tokio::task::spawn(async move {
            // Get the object instance at the given path so we can send DBus signal
            // updates
            let iface_ref = match conn
                .object_server()
                .interface::<_, Self>(path.clone())
                .await
            {
                Ok(iface) => iface,
                Err(e) => {
                    log::error!("Failed to get DBus interface {path}: {e}");
                    return;
                }
            };

            let mut iface = iface_ref.get_mut().await;
            iface.value = value;
            let result = iface.value_changed(iface_ref.signal_emitter()).await;
            if let Err(e) = result {
                log::error!("Failed to signal property changed: {e}");
            }
        });
    }

    /// Update the color brightness
    pub fn update_brightness(conn: &Connection, path: &str, value: f64) {
        let conn = conn.clone();
        let path = path.to_string();
        tokio::task::spawn(async move {
            // Get the object instance at the given path so we can send DBus signal
            // updates
            let iface_ref = match conn
                .object_server()
                .interface::<_, Self>(path.clone())
                .await
            {
                Ok(iface) => iface,
                Err(e) => {
                    log::error!("Failed to get DBus interface {path}: {e}");
                    return;
                }
            };

            let mut iface = iface_ref.get_mut().await;
            iface.brightness = value;
            let result = iface.brightness_changed(iface_ref.signal_emitter()).await;
            if let Err(e) = result {
                log::error!("Failed to signal property changed: {e}");
            }
        });
    }
}

#[interface(
    name = "org.shadowblip.Output.LED.Color",
    proxy(default_service = "org.shadowblip.InputPlumber")
)]
impl LedColorInterface {
    /// Name of the LED color
    #[zbus(property)]
    async fn color(&self) -> fdo::Result<String> {
        Ok(self.color.clone())
    }

    /// Intensity value of the LED color. A value of 1.0 is maximum intensity, and 0.0 is minimum
    /// intensity.
    #[zbus(property)]
    async fn value(&self) -> fdo::Result<f64> {
        Ok(self.value)
    }

    #[zbus(property)]
    async fn set_value(
        &mut self,
        value: f64,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Option<Header<'_>>,
    ) -> fdo::Result<()> {
        check_polkit(conn, hdr, "org.shadowblip.Output.LED.Color.Value").await?;
        self.value = value.clamp(0.0, 1.0);
        Ok(())
    }

    /// Brightness value of the LED color. A value of 1.0 is maximum brightness, and 0.0 is minimum
    /// brightness.
    #[zbus(property)]
    async fn brightness(&self) -> fdo::Result<f64> {
        Ok(0.0)
    }

    #[zbus(property)]
    async fn set_brightness(
        &mut self,
        value: f64,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Option<Header<'_>>,
    ) -> fdo::Result<()> {
        check_polkit(conn, hdr, "org.shadowblip.Output.LED.Color.Brightness").await?;
        self.brightness = value.clamp(0.0, 1.0);
        Ok(())
    }
}

impl Unregisterable for LedColorInterface {}
