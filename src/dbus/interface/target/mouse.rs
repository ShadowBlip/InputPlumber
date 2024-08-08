use zbus::{fdo, message::Header, names::BusName, Connection};
use zbus_macros::interface;

use crate::{
    input::{
        capability::{Capability, Mouse},
        event::{native::NativeEvent, value::InputValue},
        target::client::TargetDeviceClient,
    },
    polkit::is_polkit_authorized,
};

/// The [TargetMouseInterface] provides a DBus interface that can be exposed for managing
/// a [MouseDevice]. It works by sending command messages to a channel that the
/// [MouseDevice] is listening on.
pub struct TargetMouseInterface {
    conn: Connection,
    target_device: TargetDeviceClient,
}

impl TargetMouseInterface {
    pub fn new(conn: Connection, target_device: TargetDeviceClient) -> TargetMouseInterface {
        TargetMouseInterface {
            conn,
            target_device,
        }
    }
}

#[interface(name = "org.shadowblip.Input.Mouse")]
impl TargetMouseInterface {
    /// Name of the composite device
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok("Mouse".into())
    }

    /// Move the virtual mouse by the given amount relative to the cursor's
    /// relative position.
    async fn move_cursor(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        x: i32,
        y: i32,
    ) -> fdo::Result<()> {
        // Validate that the sender is authorized to send input
        let Some(sender) = hdr.sender() else {
            return Err(fdo::Error::Failed("Unable to determine sender".to_string()));
        };
        const ACTION_ID: &str = "org.shadowblip.InputPlumber.SendInput";
        let authorized = match is_polkit_authorized(&self.conn, sender, ACTION_ID).await {
            Ok(authorized) => authorized,
            Err(e) => {
                let err = format!("Failed to validate authorization: {e:?}");
                return Err(fdo::Error::AuthFailed(err));
            }
        };
        if !authorized {
            let err = "Sender not authorized for this request".to_string();
            return Err(fdo::Error::AccessDenied(err));
        }

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
