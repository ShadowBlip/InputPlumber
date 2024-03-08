//! The GenericGamepad target provides a simple generic virtual gamepad based
//! on the XBox 360 gamepad.
use std::{collections::HashMap, error::Error};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, InputEvent, KeyCode, SynchronizationCode,
    SynchronizationEvent, UinputAbsSetup,
};
use tokio::sync::{broadcast, mpsc};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::input::{
    capability::Capability,
    composite_device,
    event::{evdev::EvdevEvent, native::NativeEvent},
};

use super::TargetCommand;

const BUFFER_SIZE: usize = 2048;

/// The [DBusInterface] provides a DBus interface that can be exposed for managing
/// a [GenericGamepad].
pub struct DBusInterface {}

impl DBusInterface {
    fn new() -> DBusInterface {
        DBusInterface {}
    }
}

#[dbus_interface(name = "org.shadowblip.Input.Gamepad")]
impl DBusInterface {
    /// Name of the DBus device
    #[dbus_interface(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok("Gamepad".into())
    }
}

#[derive(Debug)]
pub struct GenericGamepad {
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
    _composite_tx: Option<broadcast::Sender<composite_device::Command>>,
}

impl GenericGamepad {
    pub fn new(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            conn,
            dbus_path: None,
            tx,
            rx,
            _composite_tx: None,
        }
    }

    /// Returns all the native capabilities that the device can emit
    pub fn _get_capabilities() -> Vec<Capability> {
        use crate::input::capability::{Gamepad, GamepadButton};
        vec![
            Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
        ]
    }

    /// Returns the DBus path of this device
    pub fn get_dbus_path(&self) -> Option<String> {
        self.dbus_path.clone()
    }

    /// Returns a transmitter channel that can be used to send events to this device
    pub fn transmitter(&self) -> mpsc::Sender<TargetCommand> {
        self.tx.clone()
    }

    /// Creates a new instance of the dbus device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());
        tokio::spawn(async move {
            let iface = DBusInterface::new();
            if let Err(e) = conn.object_server().at(path, iface).await {
                log::error!("Failed to setup DBus interface for Gamepad device: {:?}", e);
            }
        });
        Ok(())
    }

    /// Creates and runs the target device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        log::debug!("Creating virtual gamepad");
        let mut device = self.create_virtual_device()?;

        // Query information about the device to get the absolute ranges
        let axes_map = self.get_abs_info();

        // TODO: Listen for events (Force Feedback Events)
        //let event_stream = device.into_event_stream()?;

        // Listen for send events
        log::debug!("Started listening for events to send");
        while let Some(command) = self.rx.recv().await {
            match command {
                TargetCommand::WriteEvent(event) => {
                    log::trace!("Got event to emit: {:?}", event);
                    let evdev_events = self.translate_event(event, axes_map.clone());
                    device.emit(evdev_events.as_slice())?;
                    device.emit(&[SynchronizationEvent::new(
                        SynchronizationCode::SYN_REPORT,
                        0,
                    )
                    .into()])?;
                }
                TargetCommand::Stop => break,
            };
        }

        log::debug!("Stopping device");

        // Remove the DBus interface
        if let Some(path) = self.dbus_path.clone() {
            log::debug!("Removing DBus interface");
            self.conn
                .object_server()
                .remove::<DBusInterface, String>(path)
                .await?;
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

    /// Return a hashmap of ABS information for this virtual device. This information
    /// is used to denormalize input event values.
    fn get_abs_info(&self) -> HashMap<AbsoluteAxisCode, AbsInfo> {
        let mut axes_info = HashMap::new();

        let joystick_setup = AbsInfo::new(0, -32768, 32767, 16, 128, 1);
        axes_info.insert(AbsoluteAxisCode::ABS_X, joystick_setup);
        axes_info.insert(AbsoluteAxisCode::ABS_Y, joystick_setup);
        axes_info.insert(AbsoluteAxisCode::ABS_RX, joystick_setup);
        axes_info.insert(AbsoluteAxisCode::ABS_RY, joystick_setup);

        let triggers_setup = AbsInfo::new(0, 0, 255, 0, 0, 1);
        axes_info.insert(AbsoluteAxisCode::ABS_Z, triggers_setup);
        axes_info.insert(AbsoluteAxisCode::ABS_RZ, triggers_setup);

        let dpad_setup = AbsInfo::new(0, -1, 1, 0, 0, 1);
        axes_info.insert(AbsoluteAxisCode::ABS_HAT0X, dpad_setup);
        axes_info.insert(AbsoluteAxisCode::ABS_HAT0Y, dpad_setup);

        axes_info
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(&self) -> Result<VirtualDevice, Box<dyn Error>> {
        // Setup Key inputs
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_SOUTH);
        keys.insert(KeyCode::BTN_EAST);
        keys.insert(KeyCode::BTN_NORTH);
        keys.insert(KeyCode::BTN_WEST);
        keys.insert(KeyCode::BTN_TL);
        keys.insert(KeyCode::BTN_TR);
        keys.insert(KeyCode::BTN_SELECT);
        keys.insert(KeyCode::BTN_START);
        keys.insert(KeyCode::BTN_MODE);
        keys.insert(KeyCode::BTN_THUMBL);
        keys.insert(KeyCode::BTN_THUMBR);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY1);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY2);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY3);
        keys.insert(KeyCode::BTN_TRIGGER_HAPPY4);

        // Setup ABS inputs
        let joystick_setup = AbsInfo::new(0, -32768, 32767, 16, 128, 1);
        let abs_x = UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, joystick_setup);
        let abs_y = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, joystick_setup);
        let abs_rx = UinputAbsSetup::new(AbsoluteAxisCode::ABS_RX, joystick_setup);
        let abs_ry = UinputAbsSetup::new(AbsoluteAxisCode::ABS_RY, joystick_setup);
        let triggers_setup = AbsInfo::new(0, 0, 255, 0, 0, 1);
        let abs_z = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Z, triggers_setup);
        let abs_rz = UinputAbsSetup::new(AbsoluteAxisCode::ABS_RZ, triggers_setup);
        let dpad_setup = AbsInfo::new(0, -1, 1, 0, 0, 1);
        let abs_hat0x = UinputAbsSetup::new(AbsoluteAxisCode::ABS_HAT0X, dpad_setup);
        let abs_hat0y = UinputAbsSetup::new(AbsoluteAxisCode::ABS_HAT0Y, dpad_setup);

        // Setup Force Feedback
        //let mut ff = AttributeSet::<FFEffectCode>::new();
        //ff.insert(FFEffectCode::FF_RUMBLE);
        //ff.insert(FFEffectCode::FF_PERIODIC);
        //ff.insert(FFEffectCode::FF_SQUARE);
        //ff.insert(FFEffectCode::FF_TRIANGLE);
        //ff.insert(FFEffectCode::FF_SINE);
        //ff.insert(FFEffectCode::FF_GAIN);

        // Build the device
        let device = VirtualDeviceBuilder::new()?
            .name("InputPlumber Gamepad")
            .with_keys(&keys)?
            .with_absolute_axis(&abs_x)?
            .with_absolute_axis(&abs_y)?
            .with_absolute_axis(&abs_rx)?
            .with_absolute_axis(&abs_ry)?
            .with_absolute_axis(&abs_z)?
            .with_absolute_axis(&abs_rz)?
            .with_absolute_axis(&abs_hat0x)?
            .with_absolute_axis(&abs_hat0y)?
            //.with_ff(&ff)?
            .build()?;

        Ok(device)
    }
}
