//! The GenericGamepad target provides a simple generic virtual gamepad based
//! on the XBox 360 gamepad.
use std::{
    collections::HashMap,
    error::Error,
    ops::DerefMut,
    os::fd::AsRawFd,
    sync::{Arc, Mutex},
    thread,
};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, EventSummary, FFEffectCode, FFStatusCode, InputEvent,
    KeyCode, SynchronizationCode, SynchronizationEvent, UInputCode, UinputAbsSetup,
};
use nix::fcntl::{FcntlArg, OFlag};
use tokio::{
    sync::{
        broadcast,
        mpsc::{self, error::TryRecvError},
    },
    time::Duration,
};
use zbus::{fdo, Connection};
use zbus_macros::dbus_interface;

use crate::input::{
    capability::Capability,
    composite_device::Command,
    event::{evdev::EvdevEvent, native::NativeEvent},
    output_event::{OutputEvent, UinputOutputEvent},
};

use super::TargetCommand;

/// Size of the [TargetCommand] buffer for receiving input events
const BUFFER_SIZE: usize = 2048;
/// How long to sleep before polling for events.
const POLL_RATE: Duration = Duration::from_micros(1666);

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
    composite_tx: Option<broadcast::Sender<Command>>,
}

impl GenericGamepad {
    pub fn new(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            conn,
            dbus_path: None,
            tx,
            rx,
            composite_tx: None,
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

    /// Configures the device to send output events to the given composite device
    /// channel.
    pub fn set_composite_device(&mut self, tx: broadcast::Sender<Command>) {
        self.composite_tx = Some(tx);
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
        let device = self.create_virtual_device()?;

        // Put the device behind an Arc Mutex so it can be shared between the
        // read and write threads
        let device = Arc::new(Mutex::new(device));

        // Query information about the device to get the absolute ranges
        let axes_map = self.get_abs_info();

        // Listen for events from source devices
        log::debug!("Started listening for events");
        while let Some(command) = self.rx.recv().await {
            match command {
                TargetCommand::SetCompositeDevice(tx) => {
                    self.set_composite_device(tx.clone());

                    // Spawn a thread to listen for force feedback events
                    let ff_device = device.clone();
                    GenericGamepad::spawn_ff_thread(ff_device, tx);
                }
                TargetCommand::WriteEvent(event) => {
                    log::trace!("Got event to emit: {:?}", event);
                    let evdev_events = self.translate_event(event, axes_map.clone());
                    if let Ok(mut dev) = device.lock() {
                        dev.emit(evdev_events.as_slice())?;
                        dev.emit(&[
                            SynchronizationEvent::new(SynchronizationCode::SYN_REPORT, 0).into(),
                        ])?;
                    }
                }
                TargetCommand::Stop => break,
            }
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
        let mut ff = AttributeSet::<FFEffectCode>::new();
        ff.insert(FFEffectCode::FF_RUMBLE);
        ff.insert(FFEffectCode::FF_PERIODIC);
        ff.insert(FFEffectCode::FF_SQUARE);
        ff.insert(FFEffectCode::FF_TRIANGLE);
        ff.insert(FFEffectCode::FF_SINE);
        ff.insert(FFEffectCode::FF_GAIN);

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
            .with_ff(&ff)?
            .with_ff_effects_max(16)
            .build()?;

        // Set the device to do non-blocking reads
        // TODO: use epoll to wake up when data is available
        // https://github.com/emberian/evdev/blob/main/examples/evtest_nonblocking.rs
        let raw_fd = device.as_raw_fd();
        nix::fcntl::fcntl(raw_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

        Ok(device)
    }

    /// Spawns the force-feedback handler thread
    fn spawn_ff_thread(ff_device: Arc<Mutex<VirtualDevice>>, tx: broadcast::Sender<Command>) {
        tokio::task::spawn_blocking(move || {
            loop {
                // Read any events
                if let Err(e) = GenericGamepad::process_ff(&ff_device, &tx) {
                    log::warn!("Error processing FF events: {:?}", e);
                }

                // Sleep for the poll rate interval
                thread::sleep(POLL_RATE);
            }
        });
    }

    /// Process force feedback events from the given device
    fn process_ff(
        device: &Arc<Mutex<VirtualDevice>>,
        composite_dev: &broadcast::Sender<Command>,
    ) -> Result<(), Box<dyn Error>> {
        // Listen for events (Force Feedback Events)
        let events = match device.lock() {
            Ok(mut dev) => {
                let res = dev.deref_mut().fetch_events();
                match res {
                    Ok(events) => events.collect(),
                    Err(err) => match err.kind() {
                        // Do nothing if this would block
                        std::io::ErrorKind::WouldBlock => vec![],
                        _ => {
                            log::trace!("Failed to fetch events: {:?}", err);
                            return Err(err.into());
                        }
                    },
                }
            }
            Err(err) => {
                log::trace!("Failed to lock device mutex: {:?}", err);
                return Err(err.to_string().into());
            }
        };

        const STOPPED: i32 = FFStatusCode::FF_STATUS_STOPPED.0 as i32;
        const PLAYING: i32 = FFStatusCode::FF_STATUS_PLAYING.0 as i32;

        // Process the events
        for event in events {
            match event.destructure() {
                EventSummary::UInput(event, UInputCode::UI_FF_UPLOAD, ..) => {
                    log::debug!("Got FF upload event");
                    // Claim ownership of the FF upload and convert it to a FF_UPLOAD
                    // event
                    let mut event = device
                        .lock()
                        .map_err(|e| e.to_string())?
                        .process_ff_upload(event)?;
                    let effect_id = event.effect_id();

                    log::debug!("Upload effect: {:?} with id {}", event.effect(), effect_id);

                    // Send the effect data to be uploaded to the device and wait
                    // for an effect ID to be generated.
                    let (tx, rx) = std::sync::mpsc::channel::<Option<i16>>();
                    let upload = OutputEvent::Uinput(UinputOutputEvent::FFUpload(
                        effect_id,
                        event.effect(),
                        tx,
                    ));
                    if let Err(e) = composite_dev.send(Command::ProcessOutputEvent(upload)) {
                        event.set_retval(-1);
                        return Err(e.into());
                    }
                    let effect_id = match rx.recv_timeout(Duration::from_secs(1)) {
                        Ok(id) => id,
                        Err(e) => {
                            event.set_retval(-1);
                            return Err(e.into());
                        }
                    };

                    // Set the effect ID for the FF effect
                    if let Some(id) = effect_id {
                        event.set_effect_id(id);
                        event.set_retval(0);
                    } else {
                        log::warn!("Failed to get effect ID to upload FF effect");
                        event.set_retval(-1);
                    }
                }
                EventSummary::UInput(event, UInputCode::UI_FF_ERASE, ..) => {
                    log::debug!("Got FF erase event");
                    // Claim ownership of the FF erase event and convert it to a FF_ERASE
                    // event.
                    let event = device
                        .lock()
                        .map_err(|e| e.to_string())?
                        .process_ff_erase(event)?;
                    log::debug!("Erase effect: {:?}", event.effect_id());

                    let erase = OutputEvent::Uinput(UinputOutputEvent::FFErase(event.effect_id()));
                    composite_dev.send(Command::ProcessOutputEvent(erase))?;
                }
                EventSummary::ForceFeedback(.., effect_id, STOPPED) => {
                    log::debug!("Stopped effect ID: {}", effect_id.0);
                    log::debug!("Stopping event: {:?}", event);
                    composite_dev.send(Command::ProcessOutputEvent(OutputEvent::Evdev(event)))?;
                }
                EventSummary::ForceFeedback(.., effect_id, PLAYING) => {
                    log::debug!("Playing effect ID: {}", effect_id.0);
                    log::debug!("Playing event: {:?}", event);
                    composite_dev.send(Command::ProcessOutputEvent(OutputEvent::Evdev(event)))?;
                }
                _ => {
                    log::debug!("Unhandled event: {:?}", event);
                }
            }
        }

        Ok(())
    }
}
