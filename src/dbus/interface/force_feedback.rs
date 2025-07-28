use std::{error::Error, future::Future};

use packed_struct::types::{Integer, SizedInteger};
use zbus::{fdo, message::Header};
use zbus_macros::interface;

use crate::{
    dbus::polkit::check_polkit,
    drivers::steam_deck::hid_report::PackedRumbleReport,
    input::{composite_device::client::CompositeDeviceClient, output_event::OutputEvent},
};

use super::Unregisterable;

/// [ForceFeedbacker] is any device that can implement force feedback
pub trait ForceFeedbacker {
    fn rumble(&mut self, value: f64) -> impl Future<Output = Result<(), Box<dyn Error>>> + Send;
    fn stop(&mut self) -> impl Future<Output = Result<(), Box<dyn Error>>> + Send;
}

impl ForceFeedbacker for CompositeDeviceClient {
    async fn rumble(&mut self, value: f64) -> Result<(), Box<dyn Error>> {
        let value = value.min(1.0);
        let value = value.max(0.0);
        log::debug!("Sending rumble event with value: {value}");
        let report = PackedRumbleReport {
            intensity: (value * u8::MAX as f64) as u8,
            left_speed: Integer::from_primitive((value * u16::MAX as f64) as u16),
            right_speed: Integer::from_primitive((value * u16::MAX as f64) as u16),
            ..Default::default()
        };
        let event = OutputEvent::SteamDeckRumble(report);
        self.process_output_event(event).await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        self.rumble(0.0).await
    }
}

/// The [ForceFeedbackInterface] provides a DBus interface that can be exposed for
/// managing force feedback events over dbus.
pub struct ForceFeedbackInterface<T>
where
    T: ForceFeedbacker + Send + Sync,
{
    device: T,
}

impl<T> ForceFeedbackInterface<T>
where
    T: ForceFeedbacker + Send + Sync + 'static,
{
    /// Create a new dbus interface for the given force feedback device
    pub fn new(device: T) -> Self {
        Self { device }
    }
}

#[interface(
    name = "org.shadowblip.Output.ForceFeedback",
    proxy(default_service = "org.shadowblip.InputPlumber",)
)]
impl<T> ForceFeedbackInterface<T>
where
    T: ForceFeedbacker + Send + Sync + 'static,
{
    /// Send a simple rumble event
    async fn rumble(&mut self, value: f64, #[zbus(header)] hdr: Header<'_>) -> fdo::Result<()> {
        check_polkit(Some(hdr), "org.shadowblip.Output.ForceFeedback.Rumble").await?;
        self.device
            .rumble(value)
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        Ok(())
    }

    /// Stop all currently playing force feedback effects
    async fn stop(&mut self, #[zbus(header)] hdr: Header<'_>) -> fdo::Result<()> {
        check_polkit(Some(hdr), "org.shadowblip.Output.ForceFeedback.Stop").await?;
        self.device
            .stop()
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        Ok(())
    }
}

impl<T> Unregisterable for ForceFeedbackInterface<T> where T: ForceFeedbacker + Send + Sync + 'static
{}
