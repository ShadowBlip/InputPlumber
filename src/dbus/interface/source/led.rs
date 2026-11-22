use crate::dbus::{interface::Unregisterable, polkit::check_polkit};
use crate::input::source::led::multicolor::{LedMcError, LedMultiColor};
use crate::udev::device::UdevDevice;
use zbus::Connection;
use zbus::{fdo, message::Header};
use zbus_macros::interface;

impl From<LedMcError> for fdo::Error {
    fn from(value: LedMcError) -> Self {
        fdo::Error::Failed(value.to_string())
    }
}

/// DBusInterface exposing information about a led device
pub struct SourceLedInterface {
    device: UdevDevice,
}

impl SourceLedInterface {
    pub fn new(device: UdevDevice) -> SourceLedInterface {
        SourceLedInterface { device }
    }
}

#[interface(name = "org.shadowblip.Input.Source.LEDDevice")]
impl SourceLedInterface {
    /// Returns the human readable name of the device (e.g. XBox 360 Pad)
    #[zbus(property)]
    async fn id(
        &self,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Option<Header<'_>>,
    ) -> fdo::Result<String> {
        check_polkit(conn, hdr, "org.shadowblip.Input.Source.LEDDevice.Id").await?;
        Ok(self.device.sysname())
    }

    /// Returns the effects the LED supports (e.g. monocolor, breathe, rainbow)
    async fn get_effects(
        &self,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Header<'_>,
    ) -> fdo::Result<Vec<String>> {
        check_polkit(
            conn,
            Some(hdr),
            "org.shadowblip.Input.Source.LEDDevice.GetEffects",
        )
        .await?;
        let led = LedMultiColor::new(self.device.clone())?;
        Ok(led.effects()?)
    }

    /// Sets the LED effect, RGB color, and brightness (0-255)
    async fn set_state(
        &self,
        effect: String,
        color: (u8, u8, u8),
        brightness: u8,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Header<'_>,
    ) -> fdo::Result<()> {
        check_polkit(
            conn,
            Some(hdr),
            "org.shadowblip.Input.Source.LEDDevice.SetState",
        )
        .await?;
        let led = LedMultiColor::new(self.device.clone())?;
        led.set_state(&effect, color, brightness)?;
        Ok(())
    }
}

impl Unregisterable for SourceLedInterface {}
