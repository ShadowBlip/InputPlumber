use std::{collections::HashMap, error::Error, time::Instant};

use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AbsInfo, AbsoluteAxisCode, AttributeSet, InputEvent, KeyCode, RelativeAxisCode,
};
use zbus::Connection;

use crate::{
    dbus::interface::target::mouse::TargetMouseInterface,
    input::{
        capability::{Capability, Mouse, MouseButton},
        composite_device::client::CompositeDeviceClient,
        event::{evdev::EvdevEvent, native::NativeEvent, value::InputValue},
        output_event::OutputEvent,
    },
};

use super::{
    client::TargetDeviceClient, InputError, OutputError, TargetInputDevice, TargetOutputDevice,
};

/// The [MouseMotionState] keeps track of the mouse velocity from translated
/// input events (like a joystick), and sends mouse motion events to the
/// [MouseDevice] based on the current velocity.
#[derive(Debug, Default)]
pub struct MouseMotionState {
    mouse_remainder: (f64, f64),
    mouse_velocity: (f64, f64),
}

/// [MouseDevice] is a target virtual mouse that can be used to send mouse input
#[derive(Debug)]
pub struct MouseDevice {
    device: VirtualDevice,
    state: MouseMotionState,
    axis_map: HashMap<AbsoluteAxisCode, AbsInfo>,
    last_poll: Instant,
}

impl MouseDevice {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let device = MouseDevice::create_virtual_device()?;
        Ok(Self {
            device,
            state: MouseMotionState::default(),
            axis_map: HashMap::new(),
            last_poll: Instant::now(),
        })
    }

    /// Translate the given native event into an evdev event
    fn translate_event(&self, event: NativeEvent) -> Vec<InputEvent> {
        EvdevEvent::from_native_event(event, self.axis_map.clone())
            .into_iter()
            .map(|event| event.as_input_event())
            .collect()
    }

    /// Create the virtual device to emulate
    fn create_virtual_device() -> Result<VirtualDevice, Box<dyn Error>> {
        let mut buttons = AttributeSet::<KeyCode>::new();
        buttons.insert(KeyCode::BTN_LEFT);
        buttons.insert(KeyCode::BTN_RIGHT);
        buttons.insert(KeyCode::BTN_MIDDLE);
        buttons.insert(KeyCode::BTN_SIDE);
        buttons.insert(KeyCode::BTN_EXTRA);
        let device = VirtualDeviceBuilder::new()?
            .name("InputPlumber Mouse")
            .with_keys(&buttons)?
            .with_relative_axes(&AttributeSet::from_iter([
                RelativeAxisCode::REL_X,
                RelativeAxisCode::REL_Y,
                RelativeAxisCode::REL_WHEEL,
                RelativeAxisCode::REL_HWHEEL,
            ]))?
            .build()?;

        Ok(device)
    }

    /// Processes the given mouse motion or button input event.
    fn update_state(&mut self, event: NativeEvent) {
        // Get the mouse position from the event value
        let value = event.get_value();
        let (x, y) = match value {
            InputValue::Vector2 { x, y } => (x, y),
            InputValue::Vector3 { x, y, z: _ } => (x, y),
            _ => (None, None),
        };

        // Update the mouse velocity
        if let Some(x) = x {
            self.state.mouse_velocity.0 = x;
            log::trace!("Updating mouse state: {:?}", self.state.mouse_velocity);
        }
        if let Some(y) = y {
            self.state.mouse_velocity.1 = y;
            log::trace!("Updating mouse state: {:?}", self.state.mouse_velocity);
        }
    }
}

impl TargetInputDevice for MouseDevice {
    fn start_dbus_interface(
        &mut self,
        dbus: Connection,
        path: String,
        _client: TargetDeviceClient,
    ) {
        log::debug!("Starting dbus interface: {path}");
        tokio::task::spawn(async move {
            let iface = TargetMouseInterface::new();
            if let Err(e) = dbus.object_server().at(path.clone(), iface).await {
                log::debug!("Failed to start dbus interface {path}: {e:?}");
            } else {
                log::debug!("Started dbus interface on {path}");
            };
        });
    }

    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");

        // Check if this event needs to be processed by the
        // mouse state.
        if event.is_translated()
            && matches!(event.as_capability(), Capability::Mouse(Mouse::Motion))
        {
            log::trace!("Got translated mouse motion event: {:?}", event);
            self.update_state(event);
            return Ok(());
        }

        // Translate and emit the event(s)
        let evdev_events = self.translate_event(event);
        if let Err(e) = self.device.emit(evdev_events.as_slice()) {
            return Err(e.to_string().into());
        }

        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<crate::input::capability::Capability>, InputError> {
        Ok(vec![
            Capability::Mouse(Mouse::Button(MouseButton::Left)),
            Capability::Mouse(Mouse::Button(MouseButton::Right)),
            Capability::Mouse(Mouse::Button(MouseButton::Middle)),
            Capability::Mouse(Mouse::Button(MouseButton::Side)),
            Capability::Mouse(Mouse::Button(MouseButton::Extra)),
            Capability::Mouse(Mouse::Button(MouseButton::WheelUp)),
            Capability::Mouse(Mouse::Button(MouseButton::WheelDown)),
            Capability::Mouse(Mouse::Motion),
        ])
    }

    fn stop_dbus_interface(&mut self, dbus: Connection, path: String) {
        log::debug!("Stopping dbus interface for {path}");
        tokio::task::spawn(async move {
            let result = dbus
                .object_server()
                .remove::<TargetMouseInterface, String>(path.clone())
                .await;
            if let Err(e) = result {
                log::error!("Failed to stop dbus interface {path}: {e:?}");
            } else {
                log::debug!("Stopped dbus interface for {path}");
            };
        });
    }
}

impl TargetOutputDevice for MouseDevice {
    /// Move the mouse based on the given input event translation
    fn poll(&mut self, _: &Option<CompositeDeviceClient>) -> Result<Vec<OutputEvent>, OutputError> {
        // Calculate the delta between the last poll
        let delta = self.last_poll.elapsed();
        self.last_poll = Instant::now();

        // Calculate how much the mouse should move based on the current mouse velocity
        let mut pixels_to_move = (0.0, 0.0);
        pixels_to_move.0 = delta.as_secs_f64() * self.state.mouse_velocity.0;
        pixels_to_move.1 = delta.as_secs_f64() * self.state.mouse_velocity.1;

        // Get the fractional value of the position so we can accumulate them
        // in between invocations
        let mut x = pixels_to_move.0 as i32; // E.g. 3.14 -> 3
        let mut y = pixels_to_move.1 as i32;
        self.state.mouse_remainder.0 += pixels_to_move.0 - x as f64;
        self.state.mouse_remainder.1 += pixels_to_move.1 - y as f64;

        // Keep track of relative mouse movements to keep around fractional values
        if self.state.mouse_remainder.0 >= 1.0 {
            x += 1;
            self.state.mouse_remainder.0 -= 1.0;
        }
        if self.state.mouse_remainder.0 <= -1.0 {
            x -= 1;
            self.state.mouse_remainder.0 += 1.0;
        }
        if self.state.mouse_remainder.1 >= 1.0 {
            y += 1;
            self.state.mouse_remainder.1 -= 1.0;
        }
        if self.state.mouse_remainder.1 <= -1.0 {
            y -= 1;
            self.state.mouse_remainder.1 += 1.0;
        }

        // Send events to the device if the mouse state has changed
        if x != 0 {
            let value = InputValue::Vector2 {
                x: Some(x as f64),
                y: None,
            };
            let event = NativeEvent::new(Capability::Mouse(Mouse::Motion), value);
            if let Err(e) = self.write_event(event) {
                return Err(e.to_string().into());
            }
        }
        if y != 0 {
            let value = InputValue::Vector2 {
                x: None,
                y: Some(y as f64),
            };
            let event = NativeEvent::new(Capability::Mouse(Mouse::Motion), value);
            if let Err(e) = self.write_event(event) {
                return Err(e.to_string().into());
            }
        }

        Ok(vec![])
    }
}
