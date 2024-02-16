use std::{collections::HashMap, error::Error};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, InputEvent, RelativeAxisCode, SynchronizationCode,
    SynchronizationEvent,
};
use tokio::sync::{broadcast, mpsc};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::input::{
    composite_device,
    event::{evdev::EvdevEvent, native::NativeEvent},
};

const BUFFER_SIZE: usize = 2048;

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [MouseDevice]. It works by sending command messages to a channel that the
/// [MouseDevice] is listening on.
pub struct DBusInterface {}

impl DBusInterface {
    fn new() -> DBusInterface {
        DBusInterface {}
    }
}

#[dbus_interface(name = "org.shadowblip.Input.Mouse")]
impl DBusInterface {
    /// Name of the composite device
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok("Mouse".into())
    }
}

#[derive(Debug)]
pub struct MouseDevice {
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<NativeEvent>,
    rx: mpsc::Receiver<NativeEvent>,
    _composite_tx: Option<broadcast::Sender<composite_device::Command>>,
}

impl MouseDevice {
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

    /// Creates a new instance of the device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());
        tokio::spawn(async move {
            let iface = DBusInterface::new();
            if let Err(e) = conn.object_server().at(path, iface).await {
                log::error!("Failed to setup DBus interface for device: {:?}", e);
            }
        });
        Ok(())
    }

    /// Creates and runs the target device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Creating virtual mouse");
        let mut device = self.create_virtual_device()?;
        let axis_map = HashMap::new();

        // Listen for send events
        log::debug!("Started listening for events to send");
        while let Some(event) = self.rx.recv().await {
            //log::debug!("Got event to emit: {:?}", event);
            let evdev_events = self.translate_event(event, axis_map.clone());
            device.emit(evdev_events.as_slice())?;
            device.emit(&[SynchronizationEvent::new(SynchronizationCode::SYN_REPORT, 0).into()])?;
        }

        Ok(())
    }

    /// Translate the given native event into an evdev event
    fn translate_event(
        &self,
        event: NativeEvent,
        axis_map: HashMap<AbsoluteAxisCode, AbsInfo>,
    ) -> Vec<InputEvent> {
        EvdevEvent::from_native_event(event, axis_map)
            .into_iter()
            .map(|event| event.as_input_event())
            .collect()
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(&self) -> Result<VirtualDevice, Box<dyn Error>> {
        let device = VirtualDeviceBuilder::new()?
            .name("InputPlumber Mouse")
            .with_relative_axes(&AttributeSet::from_iter([
                RelativeAxisCode::REL_X,
                RelativeAxisCode::REL_Y,
                RelativeAxisCode::REL_WHEEL,
                RelativeAxisCode::REL_HWHEEL,
            ]))?
            .build()?;

        Ok(device)
    }
}
