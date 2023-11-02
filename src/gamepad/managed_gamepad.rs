// https://github.com/emberian/evdev/blob/main/examples/evtest_nonblocking.rs
use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AttributeSet, Device, EventSummary, EventType, InputEvent, KeyCode,
};
use tokio::sync::{mpsc, oneshot};
use zbus::fdo;
use zbus_macros::dbus_interface;

use super::{manager, source_device::SourceDevice};

/// Commands that can be sent to a managed gamepad
pub enum Command {
    GetInterceptMode { resp: Responder<String> },
    GetSourceDevices { resp: Responder<Vec<String>> },
    //SetInterceptMode { mode: String, resp: Responder<()> },
    InputEvent { event: InputEvent },
    AddSourceDevice { device: SourceDevice },
    RemoveSourceDevice { path: String },
    SourceDeviceClosed { path: String },
    CreateVirtualDevice { resp: Responder<String> },
}

/// Provided by the requester and used by the manager task to send
/// the command response back to the requester.
type Responder<T> = oneshot::Sender<Result<T, String>>;

/// Returns a new managed gamepad frontend and backend
pub fn new(
    tx: mpsc::Sender<Command>,
    rx: mpsc::Receiver<Command>,
    manager_tx: mpsc::Sender<manager::Command>,
) -> (DBusInterface, ManagedGamepad) {
    let backend = ManagedGamepad::new(rx, tx.clone(), manager_tx);
    let frontend = DBusInterface::new(tx);

    (frontend, backend)
}

/// A ManagedGamepad is a physical/virtual gamepad pair for processing input
/// ManagedGamepad will convert physical gamepad input into virtual gamepad input.
pub struct ManagedGamepad {
    source_device_paths: Vec<String>,
    virtual_device_paths: Vec<String>,
    virtual_gamepad: Option<VirtualDevice>,
    intercept_mode: String,
    rx: mpsc::Receiver<Command>,
    tx: mpsc::Sender<Command>,
    manager_tx: mpsc::Sender<manager::Command>,
}

impl ManagedGamepad {
    /// Creates a new instance of a managed gamepad
    pub fn new(
        rx: mpsc::Receiver<Command>,
        tx: mpsc::Sender<Command>,
        manager_tx: mpsc::Sender<manager::Command>,
    ) -> ManagedGamepad {
        ManagedGamepad {
            source_device_paths: Vec::new(),
            virtual_device_paths: Vec::new(),
            virtual_gamepad: None,
            intercept_mode: String::from("pass"),
            rx,
            tx,
            manager_tx,
        }
    }

    /// Start the backend and listen for command messages from the frontend
    /// and other clients.
    pub async fn run(&mut self) {
        log::debug!("Starting managed gamepad");
        // Continue listening for events
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                Command::InputEvent { event } => {
                    //log::debug!("Got input event: {:?}", event);
                    self.process_input(event);
                }
                Command::GetSourceDevices { resp } => {
                    let res: Result<Vec<String>, String> = Ok(self.source_device_paths.clone());
                    // Ignore errors
                    let _ = resp.send(res);
                }
                Command::GetInterceptMode { resp } => {
                    let res: Result<String, String> = Ok(self.intercept_mode.clone());
                    // Ignore errors
                    let _ = resp.send(res);
                }
                Command::AddSourceDevice { device } => {
                    log::debug!("Add source device: {}", device.path);
                    self.add_source_device(device).await;
                }
                Command::RemoveSourceDevice { path } => todo!(),
                Command::SourceDeviceClosed { path } => {
                    log::debug!("Source device was closed: {}", path);
                    self.source_device_paths
                        .retain(|p| p.as_str() != path.as_str());
                    log::debug!("Current devices: {:?}", self.source_device_paths);

                    // If no more source devices are open, stop running.
                    if self.source_device_paths.is_empty() {
                        log::debug!("No more source devices remain. Stop the managed gamepad?");
                        break;
                    }
                }
                Command::CreateVirtualDevice { resp } => {
                    let path = self.create_virtual_device().await;
                    let _ = match path {
                        Some(v) => resp.send(Ok(v)),
                        None => resp.send(Err("No path returned".to_string())),
                    };
                }
            }
        }
        log::debug!("Stopped managed gamepad");
    }

    /// Creates a virtual device that source device inputs will send their
    /// inputs to.
    async fn create_virtual_device(&mut self) -> Option<String> {
        let mut device_name = String::from("Managed Gamepad");
        let mut keys = AttributeSet::from_iter([]);
        let mut abs = AttributeSet::from_iter([]);

        // TODO: Translate the source device inputs from a profile

        // Create a virtual device builder
        let mut builder = VirtualDeviceBuilder::new().unwrap();

        // Look at all the source devices to duplicate from
        for path in &self.source_device_paths {
            log::debug!("Found source device: {}", path);
            let device = Device::open(path.clone());
            if device.is_err() {
                log::warn!("Error opening source device");
                continue;
            }

            // Copy the name from the source device
            // TODO: How do we handle this in cases with multiple source devices?
            let device = device.unwrap();
            device_name = String::from(device.name().unwrap());

            // Copy the supported keys
            let source_keys = &AttributeSet::from_iter(device.supported_keys().unwrap().iter());
            keys = source_keys.clone();

            //let foo = device.get_absinfo().unwrap();
            log::error!("Props: {:?}", device.properties());
            let source_abs =
                &AttributeSet::from_iter(device.supported_absolute_axes().unwrap().iter());
            abs = source_abs.clone();
        }

        // Create the virtual device
        let mut device = VirtualDeviceBuilder::new()
            .unwrap()
            .name(device_name.as_str())
            .with_keys(&keys)
            .unwrap()
            .build()
            .unwrap();

        // Find the path to the device in /dev/input
        let mut device_path: Option<String> = None;
        for path in device.enumerate_dev_nodes_blocking().unwrap() {
            let path = path.unwrap();
            log::debug!("Available as {}", path.display());
            let name = path
                .display()
                .to_string()
                .split('/')
                .last()
                .unwrap()
                .to_string();
            let name = format!("/dev/input/{}", name);
            device_path = Some(name);
        }

        //// Run an event loop for the device
        //// TODO: Open a separate file descriptor to send FF events
        //let device_handle = tokio::spawn(async move {
        //    loop {
        //        let events = device.fetch_events();
        //        if events.is_ok() {
        //            for event in events.unwrap() {
        //                log::debug!("Got event: {}", event.code());
        //            }
        //        }
        //    }
        //});

        // Set the virtual gamepad device
        self.virtual_gamepad = Some(device);

        device_path
    }

    /// Adds the given device to the managed gamepad and processes input events
    /// from it.
    async fn add_source_device(&mut self, mut source_device: SourceDevice) {
        // Create a channel for the device to send events over.
        let gamepad_tx = self.tx.clone();

        // Add it to the list of source devices
        self.source_device_paths.push(source_device.path.clone());
        log::debug!("Current devices: {:?}", self.source_device_paths);

        // If this device supports force feedback, get a second copy of it
        // to upload FF events to
        if source_device.device.supported_ff().is_some() {
            log::debug!("Device supports force feedback: {}", source_device.path);
            let ff_device = Device::open(source_device.path.clone());
            if ff_device.is_ok() {
                let ff_device = ff_device.unwrap();

                //ff_device.send_events(events);
            }
        }

        // Grab exclusive access over the device
        // TODO: This should be configurable
        match source_device.device.grab() {
            Ok(()) => log::debug!("Successfully grabbed source device"),
            Err(e) => log::debug!("Failed to grab source device: {}", e),
        }

        // Run the devices in their own thread to listen for input events
        // and send them to the managed gamepad.
        let device_handle = tokio::spawn(async move {
            log::debug!("Starting gamepad input loop");
            let mut device = source_device.device;
            let path = source_device.path;

            // Convert to an event stream
            let events = device.into_event_stream();
            if events.is_err() {
                log::error!("Error reading gamepad event stream");
                return;
            }

            // Loop over all input events and send them to the managed gamepad
            // for processing.
            let mut events = events.unwrap();
            loop {
                let ev = events.next_event().await;
                match ev {
                    Ok(event) => {
                        let cmd = Command::InputEvent { event };
                        let _ = gamepad_tx.send(cmd).await;
                    }
                    Err(e) => {
                        log::debug!("Error processing event: {}", e);
                        break;
                    }
                }
            }

            // Send a command to notify the managed gamepad that this source device
            // was closed
            let cmd = Command::SourceDeviceClosed { path };
            gamepad_tx.send(cmd).await;
        });
    }

    /// Sets the gamepad input mode
    pub fn set_mode(&mut self) {
        todo!()
    }

    /// Processes all physical and virtual inputs for this controller. This
    /// should be called in a tight loop to process input events.
    fn process_input(&mut self, event: InputEvent) {
        log::debug!("Process input for event: {:?}", event);
        self.process_phys_event(event);
    }

    /// Processes a single physical gamepad event. Depending on the intercept mode,
    /// this usually means forwarding events from the physical gamepad to the
    /// virtual gamepad. In other cases we want to translate physical input into
    /// DBus events that only an overlay will respond to.
    fn process_phys_event(&mut self, event: InputEvent) {
        // Get the virtual gamepad to send events to
        let Some(gamepad) = &mut self.virtual_gamepad else {
            log::warn!("No virtual gamepad was created");
            return;
        };

        // Always skip passing FF events to the virtual gamepad
        if event.event_type() == EventType::FORCEFEEDBACK {
            return;
        }

        // Intercept mode "none" will pass all input to the virtual gamepad
        if self.intercept_mode == "none" {
            gamepad.emit(&[event]).unwrap();
            return;
        }

        // Intercept mode "pass" will pass all input to the virtual gamepad
        // except for guide button presses.
        if self.intercept_mode == "pass" {
            match event.destructure() {
                EventSummary::Key(KeyEvent, KeyCode::BTN_MODE, 1) => {
                    log::debug!("Intercepted guide button press");
                    log::debug!("Setting intercept mode to ALL");
                    self.intercept_mode = String::from("all");
                }
                EventSummary::Key(KeyEvent, KeyCode::BTN_MODE, 0) => {
                    log::debug!("Setting intercept mode to PASS");
                    self.intercept_mode = String::from("pass");
                }
                _ => (),
            }
            gamepad.emit(&[event]).unwrap();
            return;
        }
    }
}

/// DBus interface implementation of the managed gamepad
pub struct DBusInterface {
    tx: mpsc::Sender<Command>,
}

impl DBusInterface {
    pub fn new(tx: mpsc::Sender<Command>) -> DBusInterface {
        DBusInterface { tx }
    }
}

#[dbus_interface(name = "org.shadowblip.Gamepad")]
impl DBusInterface {
    #[dbus_interface(property)]
    async fn intercept_mode(&self) -> fdo::Result<String> {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::GetInterceptMode { resp: resp_tx };
        self.tx.send(cmd).await.unwrap();

        // Await the response
        resp_rx
            .await
            .map_err(|err| fdo::Error::NoReply(err.to_string()))?
            .map_err(|err| fdo::Error::Failed(err.to_string()))
    }

    #[dbus_interface(property)]
    async fn source_devices(&self) -> fdo::Result<Vec<String>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::GetSourceDevices { resp: resp_tx };
        self.tx.send(cmd).await.unwrap();

        // Wait for the response
        resp_rx
            .await
            .map_err(|err| fdo::Error::NoReply(err.to_string()))?
            .map_err(|err| fdo::Error::Failed(err.to_string()))
    }
}
