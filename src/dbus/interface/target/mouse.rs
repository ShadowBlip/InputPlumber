use zbus::{fdo, message::Header, Connection};
use zbus_macros::interface;

use crate::{
    dbus::{interface::Unregisterable, polkit::check_polkit},
    input::{
        capability::{Capability, Mouse},
        event::{native::NativeEvent, value::InputValue},
        target::client::TargetDeviceClient,
    },
};

/// The [TargetMouseInterface] provides a DBus interface that can be exposed for managing
/// a [MouseDevice]. It works by sending command messages to a channel that the
/// [MouseDevice] is listening on.
pub struct TargetMouseInterface {
    target_device: TargetDeviceClient,
}

impl TargetMouseInterface {
    pub fn new(target_device: TargetDeviceClient) -> TargetMouseInterface {
        TargetMouseInterface { target_device }
    }
}

#[interface(name = "org.shadowblip.Input.Mouse")]
impl TargetMouseInterface {
    /// Name of the composite device
    #[zbus(property)]
    async fn name(
        &self,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Option<Header<'_>>,
    ) -> fdo::Result<String> {
        check_polkit(conn, hdr, "org.shadowblip.Input.Mouse.Name").await?;
        Ok("Mouse".into())
    }

    /// Move the virtual mouse by the given amount relative to the cursor's
    /// current position.
    async fn move_cursor(
        &self,
        x: i32,
        y: i32,
        #[zbus(connection)] conn: &Connection,
        #[zbus(header)] hdr: Header<'_>,
    ) -> fdo::Result<()> {
        check_polkit(conn, Some(hdr), "org.shadowblip.Input.Mouse.MoveCursor").await?;
        // Create a mouse motion event
        let value = InputValue::Vector2 {
            x: Some(x as f64),
            y: Some(y as f64),
        };
        let event = NativeEvent::new(Capability::Mouse(Mouse::Motion), value);

        // Write the event to the virtual mouse
        self.target_device
            .write_event(event)
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;

        Ok(())
    }
}

impl Unregisterable for TargetMouseInterface {}
