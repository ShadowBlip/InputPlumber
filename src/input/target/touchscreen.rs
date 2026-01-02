use std::{error::Error, os::fd::AsRawFd, time::Duration};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode,
    MiscCode, PropType, UinputAbsSetup,
};
use nix::fcntl::{FcntlArg, OFlag};
use tokio::sync::mpsc::{channel, Receiver};

use crate::{
    input::{
        capability::{Capability, Touch},
        composite_device::client::CompositeDeviceClient,
        event::{native::NativeEvent, value::denormalize_unsigned_value_u16, value::InputValue},
        output_event::OutputEvent,
    },
    udev::device::UdevDevice,
};

use super::{InputError, OutputError, TargetInputDevice, TargetOutputDevice};

/// Describes the touchscreen orientation. Used to translate touch inputs based
/// on whether the screen is rotated.
#[derive(Debug, Clone, Default)]
pub enum TouchscreenOrientation {
    #[default]
    Normal,
    UpsideDown,
    RotateLeft,
    RotateRight,
}

impl From<&str> for TouchscreenOrientation {
    fn from(value: &str) -> Self {
        match value {
            "normal" => Self::Normal,
            "left" => Self::RotateLeft,
            "right" => Self::RotateRight,
            "upsidedown" => Self::UpsideDown,
            _ => Self::Normal,
        }
    }
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
    config: TouchscreenConfig,
    config_rx: Option<Receiver<TouchscreenConfig>>,
    device: Option<VirtualDevice>,
    is_touching: bool,
    should_set_timestamp: bool,
    timestamp: i32,
    tracking_id_next: u16,
    touch_state: [TouchEvent; 10],
}

impl TouchscreenDevice {
    /// Create a new emulated touchscreen device with the default configuration.
    pub fn new() -> Result<Self, Box<dyn Error>> {
        TouchscreenDevice::new_with_config(TouchscreenConfig::default())
    }

    /// Create a new emulated touchscreen device with the given configuration.
    pub fn new_with_config(config: TouchscreenConfig) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            config,
            config_rx: None,
            device: None,
            is_touching: false,
            should_set_timestamp: true,
            timestamp: 0,
            tracking_id_next: 0,
            touch_state: [TouchEvent::default(); 10],
        })
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(config: &TouchscreenConfig) -> Result<VirtualDevice, Box<dyn Error>> {
        // Setup Key inputs
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_TOUCH);

        // Get the size based on orientation
        let (width, height) = match config.orientation {
            TouchscreenOrientation::Normal => (config.width, config.height),
            TouchscreenOrientation::UpsideDown => (config.width, config.height),
            TouchscreenOrientation::RotateLeft => (config.height, config.width),
            TouchscreenOrientation::RotateRight => (config.height, config.width),
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
        let name = config.name.as_str();
        let vendor = config.vendor_id;
        let product = config.product_id;
        let version = config.version;
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
                    self.is_touching = true;
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
                self.is_touching = false;
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
        let x = x.map(|val| denormalize_unsigned_value_u16(val, width as f64));
        let y = y.map(|val| denormalize_unsigned_value_u16(val, height as f64));

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
        let value = self.timestamp;
        let event = InputEvent::new(EventType::MISC.0, MiscCode::MSC_TIMESTAMP.0, value);
        events.push(event);
        self.timestamp = self.timestamp.wrapping_add(10000);
        self.should_set_timestamp = false;

        events
    }
}

impl TargetInputDevice for TouchscreenDevice {
    /// Start the driver when attached to a composite device.
    fn on_composite_device_attached(
        &mut self,
        composite_device: CompositeDeviceClient,
    ) -> Result<(), InputError> {
        let (tx, rx) = channel(1);
        let mut device_config = self.config.clone();

        // Spawn a task to wait for the composite device config. This is done
        // to prevent potential deadlocks if the composite device and target
        // device are both waiting for a response from each other.
        tokio::task::spawn(async move {
            // Wait to ensure the composite device has grabbed all sources
            // NOTE: We should look at other ways of signalling to target devices
            // that a new source device has been added.
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Get the source devices attached to the composite device.
            let paths = match composite_device.get_source_device_paths().await {
                Ok(paths) => paths,
                Err(e) => {
                    log::error!("Failed to get source devices from composite device: {e:?}");
                    return;
                }
            };
            log::debug!("Found source devices: {paths:?}");

            // Check all the source devices attached to the composite device and
            // check to see if any of them are touchscreen devices.
            let mut info_x = None;
            let mut info_y = None;
            for path in paths {
                let device = UdevDevice::from_devnode_path(path.as_str());
                let is_touchscreen = device
                    .get_property_from_tree("ID_INPUT_TOUCHSCREEN")
                    .is_some();
                if !is_touchscreen {
                    continue;
                }

                // If the device is a touchscreen, open it to query the touchscreen
                // resolution.
                log::debug!("Opening source touchscreen device {path} to query dimensions");
                let device = match evdev::Device::open(path.clone()) {
                    Ok(dev) => dev,
                    Err(e) => {
                        log::warn!(
                            "Unable to open device {path} to check touchscreen settings: {e:?}"
                        );
                        continue;
                    }
                };

                // Query the ABS info to get the touchscreen resolution.
                log::debug!("Querying touchscreen device {path} for dimensions");
                let abs_info = match device.get_absinfo() {
                    Ok(info) => info,
                    Err(e) => {
                        log::warn!("Unable to get ABS info for device {path}: {e:?}");
                        continue;
                    }
                };
                for (code, info) in abs_info {
                    if code.0 == AbsoluteAxisCode::ABS_MT_POSITION_X.0 {
                        info_x = Some(info);
                    }
                    if code.0 == AbsoluteAxisCode::ABS_MT_POSITION_Y.0 {
                        info_y = Some(info);
                    }
                }
            }

            // Update the configuration for the target touchscreen based on the detected
            // dimensions of the source device.
            if let Some(info) = info_x.as_ref() {
                log::debug!("Detected source X axis info: {info:?}");
                device_config.width = info.maximum() as u16;
            }
            if let Some(info) = info_y.as_ref() {
                log::debug!("Detected source Y axis info: {info:?}");
                device_config.height = info.maximum() as u16;
            }
            let detected_from_source = info_x.is_some() || info_y.is_some();

            // Get the configuration from the composite device
            log::debug!("Querying Composite Device for configuration");
            let composite_config = match composite_device.get_config().await {
                Ok(config) => config,
                Err(e) => {
                    log::error!("Failed to get config from composite device: {e:?}");
                    return;
                }
            };

            // Check to see if the composite device configuration has a touchscreen
            let mut screen_config = None;
            for src in composite_config.source_devices.into_iter() {
                let Some(src_config) = src.config else {
                    continue;
                };

                let Some(touch_config) = src_config.touchscreen else {
                    continue;
                };

                screen_config = Some(touch_config);
                break;
            }

            // Build the config to use for the virtual touchscreen device based on
            // the touchscreen configuration from the composite device.
            if let Some(screen_config) = screen_config {
                if let Some(orientation) = screen_config.orientation {
                    // Set the target screen orientation based on the source screen.
                    // If the display is rotated, the target display must be rotated
                    // in the opposite direction.
                    let orientation = TouchscreenOrientation::from(orientation.as_str());
                    let new_orientation = match orientation {
                        TouchscreenOrientation::Normal => TouchscreenOrientation::Normal,
                        TouchscreenOrientation::UpsideDown => TouchscreenOrientation::UpsideDown,
                        TouchscreenOrientation::RotateLeft => TouchscreenOrientation::RotateRight,
                        TouchscreenOrientation::RotateRight => TouchscreenOrientation::RotateLeft,
                    };
                    device_config.orientation = new_orientation;

                    // If the dimensions of the screen were detected from a source device,
                    // flip the values based on orientation.
                    if detected_from_source {
                        let width = device_config.width;
                        let height = device_config.height;
                        match device_config.orientation {
                            TouchscreenOrientation::RotateLeft
                            | TouchscreenOrientation::RotateRight => {
                                device_config.width = height;
                                device_config.height = width;
                            }
                            _ => (),
                        }
                    }
                }
                if let Some(width) = screen_config.width {
                    device_config.width = width as u16;
                }
                if let Some(height) = screen_config.height {
                    device_config.height = height as u16;
                }
            }

            log::debug!("Sending touchscreen configuration to target device");
            if let Err(e) = tx.send(device_config).await {
                log::error!("Failed to send touchscreen config: {e:?}");
            }
        });

        // Save the receiver to wait for the touchscreen config.
        self.config_rx = Some(rx);

        Ok(())
    }

    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");
        let evdev_events = self.translate_event(event);

        let Some(device) = self.device.as_mut() else {
            log::trace!("Touchscreen was never started");
            return Ok(());
        };
        device.emit(evdev_events.as_slice())?;

        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![Capability::Touchscreen(Touch::Motion)])
    }
}

impl TargetOutputDevice for TouchscreenDevice {
    // Check to see if MSC_TIMESTAMP events should be sent. Timestamp events
    // should be sent continuously during active touches.
    fn poll(&mut self, _: &Option<CompositeDeviceClient>) -> Result<Vec<OutputEvent>, OutputError> {
        // Create and start the device if needed
        if let Some(rx) = self.config_rx.as_mut() {
            if rx.is_empty() {
                // If the queue is empty, we're still waiting for a response from
                // the composite device.
                return Ok(vec![]);
            }
            let config = match rx.blocking_recv() {
                Some(config) => config,
                None => self.config.clone(),
            };

            let device = TouchscreenDevice::create_virtual_device(&config)?;
            self.device = Some(device);
            self.config = config;
        }

        let Some(device) = self.device.as_mut() else {
            log::trace!("Touchscreen not started");
            return Ok(vec![]);
        };

        // Send timestamp events whenever a touch is active
        let touching = self.is_touching;
        let set_timestamp = self.should_set_timestamp;
        if touching {
            // By default, always send a timestamp event, unless one
            // was sent with touch events.
            if set_timestamp {
                let value = self.timestamp;
                let event = InputEvent::new(EventType::MISC.0, MiscCode::MSC_TIMESTAMP.0, value);
                device.emit(&[event])?;
                self.timestamp = self.timestamp.wrapping_add(10000);
            } else {
                self.should_set_timestamp = true;
            }
        } else {
            // Reset the timestamp to zero when no touches are active
            self.timestamp = 0;
        }

        Ok(vec![])
    }
}
