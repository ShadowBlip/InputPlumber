use std::{
    error::Error,
    os::fd::AsRawFd,
    sync::{Arc, Mutex},
};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode,
    MiscCode, PropType, UinputAbsSetup,
};
use nix::fcntl::{FcntlArg, OFlag};
use tokio::{sync::mpsc, time::Duration};
use zbus::Connection;

use crate::{
    dbus::interface::target::gamepad::TargetGamepadInterface,
    input::{
        capability::{Capability, Touch},
        composite_device::client::CompositeDeviceClient,
        event::{native::NativeEvent, value::InputValue},
    },
};

use super::{client::TargetDeviceClient, command::TargetCommand};

/// Size of the [TargetCommand] buffer for receiving input events
const BUFFER_SIZE: usize = 2048;

/// Describes the touchscreen orientation. Used to translate touch inputs based
/// on whether the screen is rotated.
#[derive(Debug, Clone, Default)]
pub enum TouchscreenOrientation {
    #[allow(dead_code)]
    Normal,
    #[allow(dead_code)]
    UpsideDown,
    #[default]
    RotateLeft,
    #[allow(dead_code)]
    RotateRight,
}

/// Configuration of the target touchscreen device.
#[derive(Debug, Clone)]
pub struct TouchscreenConfig {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    pub width: u16,
    pub height: u16,
    pub orientation: TouchscreenOrientation,
}

impl Default for TouchscreenConfig {
    fn default() -> Self {
        Self {
            name: "InputPlumber Touchscreen".to_string(),
            vendor_id: 0x2808,
            product_id: 0x1015,
            version: 0x100,
            width: 1280,
            height: 800,
            orientation: TouchscreenOrientation::default(),
        }
    }
}

/// Structure for storing the state of touch events
#[derive(Debug, Copy, Clone, Default)]
pub struct TouchEvent {
    is_touching: bool,
    x: u16,
    y: u16,
}

/// Generic touchscreen implementation using evdev. When creating the touchscreen,
/// a [TouchscreenConfig] can be passed to configure the size and orientation of
/// the touchscreen.
#[derive(Debug)]
pub struct TouchscreenDevice {
    conn: Connection,
    config: TouchscreenConfig,
    dbus_path: Option<String>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
    composite_device: Option<CompositeDeviceClient>,
    is_touching: Arc<Mutex<bool>>,
    should_set_timestamp: Arc<Mutex<bool>>,
    timestamp: Arc<Mutex<i32>>,
    tracking_id_next: u16,
    touch_state: [TouchEvent; 10],
}

impl TouchscreenDevice {
    /// Create a new emulated touchscreen device with the default configuration.
    pub fn new(conn: Connection) -> Self {
        TouchscreenDevice::new_with_config(conn, TouchscreenConfig::default())
    }

    /// Create a new emulated touchscreen device with the given configuration.
    pub fn new_with_config(conn: Connection, config: TouchscreenConfig) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            conn,
            config,
            dbus_path: None,
            tx,
            rx,
            composite_device: None,
            is_touching: Arc::new(Mutex::new(false)),
            should_set_timestamp: Arc::new(Mutex::new(true)),
            timestamp: Arc::new(Mutex::new(0)),
            tracking_id_next: 0,
            touch_state: [TouchEvent::default(); 10],
        }
    }

    /// Returns the DBus path of this device
    pub fn _get_dbus_path(&self) -> Option<String> {
        self.dbus_path.clone()
    }

    /// Returns a client channel that can be used to send events to this device
    pub fn client(&self) -> TargetDeviceClient {
        self.tx.clone().into()
    }

    /// Configures the device to send output events to the given composite device
    /// channel.
    pub fn set_composite_device(&mut self, composite_device: CompositeDeviceClient) {
        self.composite_device = Some(composite_device);
    }

    /// Creates a new instance of the dbus device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting dbus interface on {path}");
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());
        tokio::spawn(async move {
            log::debug!("Starting dbus interface: {path}");
            let iface = TargetGamepadInterface::new("Gamepad".into());
            if let Err(e) = conn.object_server().at(path.clone(), iface).await {
                log::debug!("Failed to start dbus interface {path}: {e:?}");
            } else {
                log::debug!("Started dbus interface on {path}");
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

        // Spawn a task that checks to see if MSC_TIMESTAMP events should
        // be sent. Timestamp events should be sent continuously during active
        // touches.
        let timestamp = self.timestamp.clone();
        let should_set_timestamp = self.should_set_timestamp.clone();
        let is_touching = self.is_touching.clone();
        let dev = device.clone();
        tokio::task::spawn(async move {
            let duration = Duration::from_micros(13605);
            loop {
                // Check to see if the main input thread still has a reference
                // to the virtual device. If it does not, it means the device
                // has stopped.
                let num_refs = Arc::strong_count(&dev);
                if num_refs == 1 {
                    log::debug!("Virtual device stopped. Stopping timestamp thread.");
                    break;
                }

                // Send timestamp events whenever a touch is active
                let touching = *is_touching.lock().unwrap();
                let set_timestamp = *should_set_timestamp.lock().unwrap();
                if touching {
                    // By default, always send a timestamp event, unless one
                    // was sent with touch events.
                    if set_timestamp {
                        let value = *timestamp.lock().unwrap();
                        let event =
                            InputEvent::new(EventType::MISC.0, MiscCode::MSC_TIMESTAMP.0, value);
                        if let Ok(mut d) = dev.lock() {
                            if let Err(e) = d.emit(&[event]) {
                                log::error!("Failed to emit timestamp event: {e:?}");
                            }
                        }
                        let ts = *timestamp.lock().unwrap();
                        *timestamp.lock().unwrap() = ts.wrapping_add(10000);
                    } else {
                        *should_set_timestamp.lock().unwrap() = true;
                    }
                } else {
                    // Reset the timestamp to zero when no touches are active
                    *timestamp.lock().unwrap() = 0;
                }

                tokio::time::sleep(duration).await;
            }
        });

        // Listen for events from source devices
        log::debug!("Started listening for events");
        while let Some(command) = self.rx.recv().await {
            match command {
                TargetCommand::SetCompositeDevice(composite_device) => {
                    self.set_composite_device(composite_device.clone());
                }
                TargetCommand::WriteEvent(event) => {
                    log::trace!("Got event to emit: {:?}", event);
                    let evdev_events = self.translate_event(event);
                    if let Ok(mut dev) = device.lock() {
                        dev.emit(evdev_events.as_slice())?;
                    }
                }
                TargetCommand::GetCapabilities(tx) => {
                    let caps = self.get_capabilities();
                    if let Err(e) = tx.send(caps).await {
                        log::error!("Failed to send target capabilities: {e:?}");
                    }
                }
                TargetCommand::GetType(tx) => {
                    if let Err(e) = tx.send("touchscreen".to_string()).await {
                        log::error!("Failed to send target type: {e:?}");
                    }
                }
                TargetCommand::Stop => break,
            }
        }

        log::debug!(
            "Stopping device {}",
            self.dbus_path.clone().unwrap_or_default()
        );

        // Remove the DBus interface
        if let Some(path) = self.dbus_path.clone() {
            let conn = self.conn.clone();
            let path = path.clone();
            tokio::task::spawn(async move {
                log::debug!("Stopping dbus interface for {path}");
                let result = conn
                    .object_server()
                    .remove::<TargetGamepadInterface, String>(path.clone())
                    .await;
                if let Err(e) = result {
                    log::error!("Failed to stop dbus interface {path}: {e:?}");
                } else {
                    log::debug!("Stopped dbus interface for {path}");
                }
            });
        }

        Ok(())
    }

    /// Translate the given native event into a series of evdev events
    fn translate_event(&mut self, event: NativeEvent) -> Vec<InputEvent> {
        let mut events = vec![];
        let cap = event.as_capability();
        if cap != Capability::Touchscreen(Touch::Motion) {
            return events;
        }

        // Destructure the input value
        let InputValue::Touch {
            index,
            is_touching,
            pressure: _,
            x,
            y,
        } = event.get_value()
        else {
            return events;
        };

        // Adjust the values based on configured orientation
        let (x, y) = match self.config.orientation {
            TouchscreenOrientation::Normal => (x, y),
            TouchscreenOrientation::UpsideDown => (x.map(|v| 1.0 - v), y.map(|v| 1.0 - v)),
            TouchscreenOrientation::RotateLeft => (y.map(|v| 1.0 - v), x),
            TouchscreenOrientation::RotateRight => (y, x.map(|v| 1.0 - v)),
        };
        let (width, height) = match self.config.orientation {
            TouchscreenOrientation::Normal => (self.config.width, self.config.height),
            TouchscreenOrientation::UpsideDown => (self.config.width, self.config.height),
            TouchscreenOrientation::RotateLeft => (self.config.height, self.config.width),
            TouchscreenOrientation::RotateRight => (self.config.height, self.config.width),
        };

        // Get the last number of active touches. This is used to determine
        // whether or not BTN_TOUCH or MSC_TIMESTAMP events need to be sent.
        let last_num_touches = {
            let mut touch_count = 0;
            for touch in self.touch_state {
                if touch.is_touching {
                    touch_count += 1;
                }
            }
            touch_count
        };

        // Create a slot event based on the touch index
        let slot_event = InputEvent::new(
            EventType::ABSOLUTE.0,
            AbsoluteAxisCode::ABS_MT_SLOT.0,
            index as i32,
        );
        events.push(slot_event);

        // Ensure that the touch index isn't greater than the number of fingers
        // humans have (normally).
        let i = index as usize;
        if i > self.touch_state.len() - 1 {
            log::error!(
                "Got touch index {i} greater than supported max index {}!",
                self.touch_state.len() - 1
            );
            return events;
        }

        // Check to see if this is a touch "up" or "down"
        if is_touching != self.touch_state[i].is_touching {
            // Get the tracking id based on the state of the touch
            let tracking_id = if is_touching {
                // If no touches are active, but a new touch event was received,
                // send a BTN_TOUCH 1 event.
                if last_num_touches == 0 {
                    let touch_event = InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOUCH.0, 1);
                    events.push(touch_event);
                    let mut touching = self.is_touching.lock().unwrap();
                    *touching = true;
                }
                let tracking_id = self.tracking_id_next;
                self.tracking_id_next = self.tracking_id_next.wrapping_add(1);
                tracking_id as i32
            } else {
                // If one touch is active and a new touch up event was received,
                // send a BTN_TOUCH 0 event to indicate that no touches remain.
                if last_num_touches == 1 {
                    let touch_event = InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOUCH.0, 0);
                    events.push(touch_event);
                }
                let mut touching = self.is_touching.lock().unwrap();
                *touching = false;
                -1
            };
            let tracking_event = InputEvent::new(
                EventType::ABSOLUTE.0,
                AbsoluteAxisCode::ABS_MT_TRACKING_ID.0,
                tracking_id,
            );
            events.push(tracking_event);
        }

        // Denormalize the x, y values based on the screen size
        let x = x.map(|val| denormalize_unsigned_value(val, width as f64));
        let y = y.map(|val| denormalize_unsigned_value(val, height as f64));

        // Send events for x values
        if let Some(x) = x {
            if x != self.touch_state[i].x {
                let x = x as i32;
                let event = InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_MT_POSITION_X.0,
                    x,
                );
                events.push(event);
                let event =
                    InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_MT_TOOL_X.0, x);
                events.push(event);
                if index == 0 {
                    let event =
                        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, x);
                    events.push(event);
                }
            }
        }

        // Send events for y values
        if let Some(y) = y {
            if y != self.touch_state[i].y {
                let y = y as i32;
                let event = InputEvent::new(
                    EventType::ABSOLUTE.0,
                    AbsoluteAxisCode::ABS_MT_POSITION_Y.0,
                    y,
                );
                events.push(event);
                let event =
                    InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_MT_TOOL_Y.0, y);
                events.push(event);
                if index == 0 {
                    let event =
                        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_Y.0, y);
                    events.push(event);
                }
            }
        }

        // Update the internal touch state
        self.touch_state[i].is_touching = is_touching;
        if let Some(x) = x {
            self.touch_state[i].x = x;
        }
        if let Some(y) = y {
            self.touch_state[i].y = y;
        }

        // Update and handle timestamps
        let value = *self.timestamp.lock().unwrap();
        let event = InputEvent::new(EventType::MISC.0, MiscCode::MSC_TIMESTAMP.0, value);
        events.push(event);
        let ts = *self.timestamp.lock().unwrap();
        *self.timestamp.lock().unwrap() = ts.wrapping_add(10000);
        *self.should_set_timestamp.lock().unwrap() = false;

        events
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(&self) -> Result<VirtualDevice, Box<dyn Error>> {
        // Setup Key inputs
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_TOUCH);

        // Get the size based on orientation
        let (width, height) = match self.config.orientation {
            TouchscreenOrientation::Normal => (self.config.width, self.config.height),
            TouchscreenOrientation::UpsideDown => (self.config.width, self.config.height),
            TouchscreenOrientation::RotateLeft => (self.config.height, self.config.width),
            TouchscreenOrientation::RotateRight => (self.config.height, self.config.width),
        };

        // Setup ABS inputs
        let screen_width_setup = AbsInfo::new(0, 0, width as i32, 0, 0, 3);
        let screen_height_setup = AbsInfo::new(0, 0, height as i32, 0, 0, 9);
        let abs_x = UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, screen_width_setup);
        let abs_y = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, screen_height_setup);
        let abs_mt_pos_x =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_POSITION_X, screen_width_setup);
        let abs_mt_pos_y =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_POSITION_Y, screen_height_setup);
        let abs_mt_tool_x =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_TOOL_X, screen_width_setup);
        let abs_mt_tool_y =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_TOOL_Y, screen_height_setup);

        let slot_setup = AbsInfo::new(0, 0, 9, 0, 0, 0);
        let abs_mt_slot = UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_SLOT, slot_setup);

        let touch_min_maj_setup = AbsInfo::new(0, 0, 255, 0, 0, 10);
        let abs_mt_touch_major =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_TOUCH_MAJOR, touch_min_maj_setup);
        let abs_mt_touch_minor =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_TOUCH_MINOR, touch_min_maj_setup);

        let orientation_setup = AbsInfo::new(0, 0, 1, 0, 0, 0);
        let abs_mt_orientation =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_ORIENTATION, orientation_setup);

        let tracking_id_setup = AbsInfo::new(0, 0, u16::MAX.into(), 0, 0, 0);
        let abs_mt_tracking_id =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_TRACKING_ID, tracking_id_setup);

        // Setup MSC inputs
        let mut mscs = AttributeSet::<MiscCode>::new();
        mscs.insert(MiscCode::MSC_TIMESTAMP);

        // Setup properties
        let mut properties = AttributeSet::<PropType>::new();
        properties.insert(PropType::DIRECT);

        // Identify to the kernel as a touchscreen
        let name = self.config.name.as_str();
        let vendor = self.config.vendor_id;
        let product = self.config.product_id;
        let version = self.config.version;
        let id = InputId::new(BusType(3), vendor, product, version);

        // Build the device
        let device = VirtualDeviceBuilder::new()?
            .name(name)
            .input_id(id)
            .with_properties(&properties)?
            .with_keys(&keys)?
            .with_msc(&mscs)?
            .with_absolute_axis(&abs_x)?
            .with_absolute_axis(&abs_y)?
            .with_absolute_axis(&abs_mt_slot)?
            .with_absolute_axis(&abs_mt_touch_major)?
            .with_absolute_axis(&abs_mt_touch_minor)?
            .with_absolute_axis(&abs_mt_orientation)?
            .with_absolute_axis(&abs_mt_pos_x)?
            .with_absolute_axis(&abs_mt_pos_y)?
            .with_absolute_axis(&abs_mt_tracking_id)?
            .with_absolute_axis(&abs_mt_tool_x)?
            .with_absolute_axis(&abs_mt_tool_y)?
            .build()?;

        // Set the device to do non-blocking reads
        // TODO: use epoll to wake up when data is available
        // https://github.com/emberian/evdev/blob/main/examples/evtest_nonblocking.rs
        let raw_fd = device.as_raw_fd();
        nix::fcntl::fcntl(raw_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

        Ok(device)
    }

    /// Returns capabilities of the target device
    fn get_capabilities(&self) -> Vec<Capability> {
        vec![Capability::Touchscreen(Touch::Motion)]
    }
}

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_unsigned_value(normal_value: f64, max: f64) -> u16 {
    (normal_value * max).round() as u16
}
