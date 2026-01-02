use std::{error::Error, os::fd::AsRawFd, time::Instant};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode,
    MiscCode, PropType, UinputAbsSetup,
};
use nix::fcntl::{FcntlArg, OFlag};

use crate::input::{
    capability::{Capability, Touch, TouchButton, Touchpad},
    composite_device::client::CompositeDeviceClient,
    event::{native::NativeEvent, value::denormalize_unsigned_value_u16, value::InputValue},
    output_event::OutputEvent,
};

use super::{InputError, OutputError, TargetInputDevice, TargetOutputDevice};

/// Configuration of the target touchpad device.
#[derive(Debug, Clone)]
pub struct TouchpadConfig {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    pub width: u16,
    pub height: u16,
}

impl Default for TouchpadConfig {
    fn default() -> Self {
        Self {
            name: "InputPlumber Touchpad".to_string(),
            vendor_id: 0x0000,
            product_id: 0xffff,
            version: 0x001,
            // NOTE: The width and height of the touchpad are important to how the
            // touchpad will behave. The width/height ratio of the touchpad MUST
            // match the ABS resolution ratio. I.e. if the touchpad is a square
            // with a (width, height) of (1024, 1024), then the ABS resolution
            // should use the same ratio like (32, 32). If the touchpad is a rectangle
            // with a size of (2048, 1024), then the resolution for the ABS axes
            // must use the same 2:1 ratio like (64, 32).
            width: 1024,
            height: 1024,
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

/// Generic touchpad implementation using evdev. When creating the touchpad,
/// a [TouchpadConfig] can be passed to configure the size and orientation of
/// the touchpad.
#[derive(Debug)]
pub struct TouchpadDevice {
    config: TouchpadConfig,
    device: VirtualDevice,
    is_touching: bool,
    should_set_timestamp: bool,
    timestamp: Instant,
    tracking_id_next: u16,
    touch_state: [TouchEvent; 10],
}

impl TouchpadDevice {
    /// Create a new emulated touchpad device with the default configuration.
    pub fn new() -> Result<Self, Box<dyn Error>> {
        TouchpadDevice::new_with_config(TouchpadConfig::default())
    }

    /// Create a new emulated touchpad device with the given configuration.
    pub fn new_with_config(config: TouchpadConfig) -> Result<Self, Box<dyn Error>> {
        let device = TouchpadDevice::create_virtual_device(&config)?;
        Ok(Self {
            config,
            device,
            is_touching: false,
            should_set_timestamp: true,
            timestamp: Instant::now(),
            tracking_id_next: 0,
            touch_state: [TouchEvent::default(); 10],
        })
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(config: &TouchpadConfig) -> Result<VirtualDevice, Box<dyn Error>> {
        // Setup Key inputs
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_LEFT);
        keys.insert(KeyCode::BTN_RIGHT);
        keys.insert(KeyCode::BTN_TOOL_FINGER);
        keys.insert(KeyCode::BTN_TOUCH);
        keys.insert(KeyCode::BTN_TOOL_DOUBLETAP);

        // Setup ABS inputs
        let pad_width_setup = AbsInfo::new(0, 0, config.width as i32, 0, 0, 36);
        let pad_height_setup = AbsInfo::new(0, 0, config.height as i32, 0, 0, 36);
        let abs_x = UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, pad_width_setup);
        let abs_y = UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, pad_height_setup);
        let abs_mt_pos_x =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_POSITION_X, pad_width_setup);
        let abs_mt_pos_y =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_POSITION_Y, pad_height_setup);

        let pad_tool_setup = AbsInfo::new(0, 0, 2, 0, 0, 0);
        let abs_mt_tool_type =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_TOOL_TYPE, pad_tool_setup);

        let slot_setup = AbsInfo::new(0, 0, 9, 0, 0, 0);
        let abs_mt_slot = UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_SLOT, slot_setup);

        let tracking_id_setup = AbsInfo::new(0, 0, u16::MAX.into(), 0, 0, 0);
        let abs_mt_tracking_id =
            UinputAbsSetup::new(AbsoluteAxisCode::ABS_MT_TRACKING_ID, tracking_id_setup);

        // Setup MSC inputs
        let mut mscs = AttributeSet::<MiscCode>::new();
        mscs.insert(MiscCode::MSC_TIMESTAMP);

        // Setup properties
        let mut properties = AttributeSet::<PropType>::new();
        properties.insert(PropType::POINTER);
        properties.insert(PropType::BUTTONPAD);

        // Identify to the kernel as a touchpad
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
            .with_absolute_axis(&abs_mt_pos_x)?
            .with_absolute_axis(&abs_mt_pos_y)?
            .with_absolute_axis(&abs_mt_tracking_id)?
            .with_absolute_axis(&abs_mt_tool_type)?
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

        let button_events = [
            Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))),
            Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press))),
            Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
        ];

        let motion_events = [
            Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)),
            Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
            Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
        ];

        if button_events.contains(&cap) {
            let event = self.translate_button(event.clone());
            events.push(event);
        };

        if motion_events.contains(&cap) {
            let mut event_list = self.translate_motion(event);
            events.append(&mut event_list);
        }

        events
    }

    /// Translate the given native [Touch::Motion] event into a sereis of evdev events
    fn translate_motion(&mut self, event: NativeEvent) -> Vec<InputEvent> {
        let mut events = Vec::with_capacity(10);

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
                // send a BTN_TOUCH 1 event and a BTN_TOOL_FINGER 1 event.
                if last_num_touches == 0 {
                    let touch_event = InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOUCH.0, 1);
                    events.push(touch_event);
                    let touch_event =
                        InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOOL_FINGER.0, 1);
                    events.push(touch_event);

                    // Reset the timestamp if the state is going from "not touching" -> "touching"
                    if !self.is_touching {
                        self.timestamp = Instant::now();
                    }
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
                    let touch_event =
                        InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOOL_FINGER.0, 0);
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

        // Denormalize the x, y values based on the pad size
        let x = x.map(|val| denormalize_unsigned_value_u16(val, self.config.width as f64));
        let y = y.map(|val| denormalize_unsigned_value_u16(val, self.config.height as f64));

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
        let value = self.timestamp.elapsed().as_micros() as i32;
        let event = InputEvent::new(EventType::MISC.0, MiscCode::MSC_TIMESTAMP.0, value);
        events.push(event);
        self.should_set_timestamp = false;

        events
    }

    fn translate_button(&mut self, event: NativeEvent) -> InputEvent {
        // Destructure the input value
        let value: InputValue = event.get_value();
        InputEvent::new(
            EventType::KEY.0,
            KeyCode::BTN_LEFT.0,
            value.pressed() as i32,
        )
    }
}

impl TargetInputDevice for TouchpadDevice {
    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");
        let evdev_events = self.translate_event(event);
        self.device.emit(evdev_events.as_slice())?;

        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![
            Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))),
            Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)),
            Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press))),
            Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
            Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
            Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
        ])
    }
}

impl TargetOutputDevice for TouchpadDevice {
    // Check to see if MSC_TIMESTAMP events should be sent. Timestamp events
    // should be sent continuously during active touches.
    fn poll(&mut self, _: &Option<CompositeDeviceClient>) -> Result<Vec<OutputEvent>, OutputError> {
        // Send timestamp events whenever a touch is active
        let touching = self.is_touching;
        let set_timestamp = self.should_set_timestamp;
        if !touching {
            return Ok(vec![]);
        }

        // By default, always send a timestamp event, unless one
        // was sent with touch events.
        if set_timestamp {
            let value = self.timestamp.elapsed().as_micros() as i32;
            let event = InputEvent::new(EventType::MISC.0, MiscCode::MSC_TIMESTAMP.0, value);
            self.device.emit(&[event])?;
        } else {
            self.should_set_timestamp = true;
        }

        Ok(vec![])
    }
}
