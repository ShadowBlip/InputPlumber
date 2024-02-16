use std::error::Error;

use tokio::sync::{broadcast, mpsc};
use zbus::{fdo, Connection, SignalContext};
use zbus_macros::dbus_interface;

use crate::input::{
    composite_device,
    event::{
        dbus::{Action, DBusEvent},
        native::NativeEvent,
    },
};

const BUFFER_SIZE: usize = 2048;

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [DBusDevice].
pub struct DBusInterface {}

impl DBusInterface {
    fn new() -> DBusInterface {
        DBusInterface {}
    }
}

#[dbus_interface(name = "org.shadowblip.Input.DBusDevice")]
impl DBusInterface {
    /// Name of the DBus device
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok("DBusDevice".into())
    }

    /// Emitted when an input event occurs
    #[dbus_interface(signal)]
    async fn input_event(ctxt: &SignalContext<'_>, event: String, value: f64) -> zbus::Result<()>;
}

/// The [DBusDevice] is a virtual input device that can emit input events. It
/// is primarily used when a [CompositeDevice] is using input interception to
/// divert inputs to an overlay over DBus.
#[derive(Debug)]
pub struct DBusDevice {
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<NativeEvent>,
    rx: mpsc::Receiver<NativeEvent>,
    _composite_tx: Option<broadcast::Sender<composite_device::Command>>,
}

impl DBusDevice {
    // Create a new [DBusDevice] instance.
    pub fn new(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            conn,
            dbus_path: None,
            _composite_tx: None,
            tx,
            rx,
        }
    }

    /// Returns the DBus path of this device
    pub fn get_dbus_path(&self) -> Option<String> {
        self.dbus_path.clone()
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn transmitter(&self) -> mpsc::Sender<NativeEvent> {
        self.tx.clone()
    }

    /// Creates a new instance of the dbus device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());
        tokio::spawn(async move {
            let iface = DBusInterface::new();
            if let Err(e) = conn.object_server().at(path, iface).await {
                log::error!("Failed to setup DBus interface for DBus device: {:?}", e);
            }
        });
        Ok(())
    }

    /// Creates and runs the target device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Creating virtual dbus device");

        // Listen for send events
        log::debug!("Started listening for events to send");
        while let Some(event) = self.rx.recv().await {
            //log::debug!("Got event to emit: {:?}", event);
            let dbus_event = self.translate_event(event);
            self.write_dbus_event(dbus_event).await?;
        }

        Ok(())
    }

    /// Translate the given native event into a dbus event
    fn translate_event(&self, event: NativeEvent) -> DBusEvent {
        event.into()
    }

    /// Writes the given event to DBus
    async fn write_dbus_event(&self, event: DBusEvent) -> Result<(), Box<dyn Error>> {
        // Only send valid events
        let valid = !matches!(event.action, Action::None);
        if !valid {
            return Ok(());
        }

        // DBus events can only be written if there is a DBus path reference.
        let Some(path) = self.dbus_path.clone() else {
            return Err("No dbus path exists to send events to".into());
        };

        // Get the object instance at the given path so we can send DBus signal
        // updates
        let iface_ref = self
            .conn
            .object_server()
            .interface::<_, DBusInterface>(path)
            .await?;

        // Send the input event signal
        DBusInterface::input_event(
            iface_ref.signal_context(),
            event.action.as_string(),
            event.value,
        )
        .await?;

        Ok(())
    }
}
