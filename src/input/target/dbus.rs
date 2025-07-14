use std::{
    collections::{HashMap, HashSet},
    error::Error,
};

use zbus::object_server::Interface;

use crate::{
    dbus::interface::{
        force_feedback::ForceFeedbackInterface, target::dbus::TargetDBusInterface,
        DBusInterfaceManager,
    },
    input::{
        capability::{Capability, Gamepad, GamepadButton},
        composite_device::client::CompositeDeviceClient,
        event::{
            dbus::{Action, DBusEvent},
            native::NativeEvent,
            value::InputValue,
        },
        output_capability::OutputCapability,
    },
};

use super::{
    client::TargetDeviceClient, InputError, OutputError, TargetDeviceTypeId, TargetInputDevice,
    TargetOutputDevice,
};

/// The threshold for axis inputs to be considered "pressed"
const AXIS_THRESHOLD: f64 = 0.60;
/// The threshold for trigger inputs to be considered "pressed"
const TRIGGER_THRESHOLD: f64 = 0.75;

/// The internal emulated device state for tracking analog input
#[derive(Debug, Clone, Default)]
struct State {
    pressed_left: bool,
    pressed_right: bool,
    pressed_up: bool,
    pressed_down: bool,
    pressed_l2: bool,
    l2_value: Option<f64>,
    pressed_r2: bool,
    r2_value: Option<f64>,
    buttons: HashMap<Capability, bool>,
}

/// The [DBusDevice] is a virtual input device that can emit input events. It
/// is primarily used when a [CompositeDevice] is using input interception to
/// divert inputs to an overlay over DBus.
#[derive(Debug)]
pub struct DBusDevice {
    state: State,
    dbus: Option<DBusInterfaceManager>,
    device: Option<CompositeDeviceClient>,
}

impl Default for DBusDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl DBusDevice {
    // Create a new [DBusDevice] instance.
    pub fn new() -> Self {
        Self {
            state: State::default(),
            dbus: None,
            device: None,
        }
    }

    /// Translate the given native event into one or more dbus events
    fn translate_event(&mut self, event: NativeEvent) -> Vec<DBusEvent> {
        // Check to see if this is an axis event, which requires special
        // handling.
        let source_cap = event.as_capability();

        let mut translated = vec![];
        let events = DBusEvent::from_native_event(event);
        for mut event in events {
            // Axis input is a special case, where we need to keep track of the
            // current state of the axis, and only emit events whenever the axis
            // passes or falls below the defined threshold.
            let include_event = if matches!(&source_cap, Capability::Gamepad(Gamepad::Axis(_))) {
                match event.action {
                    Action::Left => {
                        // If the opposite axis is pressed, emit a "release" event
                        // for it.
                        if self.state.pressed_right {
                            let other_event = DBusEvent::new(Action::Right, InputValue::Float(0.0));
                            translated.push(other_event);
                            self.state.pressed_right = false;
                        }
                        if self.state.pressed_left && event.as_f64() < AXIS_THRESHOLD {
                            event.value = InputValue::Float(0.0);
                            self.state.pressed_left = false;
                            true
                        } else if !self.state.pressed_left && event.as_f64() > AXIS_THRESHOLD {
                            event.value = InputValue::Float(1.0);
                            self.state.pressed_left = true;
                            true
                        } else {
                            false
                        }
                    }
                    Action::Right => {
                        // If the opposite axis is pressed, emit a "release" event
                        // for it.
                        if self.state.pressed_left {
                            let other_event = DBusEvent::new(Action::Left, InputValue::Float(0.0));
                            translated.push(other_event);
                            self.state.pressed_left = false;
                        }
                        if self.state.pressed_right && event.as_f64() < AXIS_THRESHOLD {
                            event.value = InputValue::Float(0.0);
                            self.state.pressed_right = false;
                            true
                        } else if !self.state.pressed_right && event.as_f64() > AXIS_THRESHOLD {
                            event.value = InputValue::Float(1.0);
                            self.state.pressed_right = true;
                            true
                        } else {
                            false
                        }
                    }
                    Action::Up => {
                        // If the opposite axis is pressed, emit a "release" event
                        // for it.
                        if self.state.pressed_down {
                            let other_event = DBusEvent::new(Action::Down, InputValue::Float(0.0));
                            translated.push(other_event);
                            self.state.pressed_down = false;
                        }
                        if self.state.pressed_up && event.as_f64() < AXIS_THRESHOLD {
                            event.value = InputValue::Float(0.0);
                            self.state.pressed_up = false;
                            true
                        } else if !self.state.pressed_up && event.as_f64() > AXIS_THRESHOLD {
                            event.value = InputValue::Float(1.0);
                            self.state.pressed_up = true;
                            true
                        } else {
                            false
                        }
                    }
                    Action::Down => {
                        // If the opposite axis is pressed, emit a "release" event
                        // for it.
                        if self.state.pressed_up {
                            let other_event = DBusEvent::new(Action::Up, InputValue::Float(0.0));
                            translated.push(other_event);
                            self.state.pressed_up = false;
                        }
                        if self.state.pressed_down && event.as_f64() < AXIS_THRESHOLD {
                            event.value = InputValue::Float(0.0);
                            self.state.pressed_down = false;
                            true
                        } else if !self.state.pressed_down && event.as_f64() > AXIS_THRESHOLD {
                            event.value = InputValue::Float(1.0);
                            self.state.pressed_down = true;
                            true
                        } else {
                            false
                        }
                    }
                    _ => true,
                }
            }
            // Trigger input is also a special case, where we need to keep track of the
            // current state of the trigger, and only emit events whenever the trigger
            // passes or falls below the defined threshold.
            else if matches!(&source_cap, Capability::Gamepad(Gamepad::Trigger(_))) {
                match event.action {
                    Action::L2 => {
                        let value = event.as_f64();
                        self.state.l2_value = Some(value);
                        if self.state.pressed_l2 && value < TRIGGER_THRESHOLD {
                            event.value = InputValue::Float(0.0);
                            self.state.pressed_l2 = false;
                            true
                        } else if !self.state.pressed_l2 && value > TRIGGER_THRESHOLD {
                            event.value = InputValue::Float(1.0);
                            self.state.pressed_l2 = true;
                            true
                        } else {
                            false
                        }
                    }
                    Action::R2 => {
                        let value = event.as_f64();
                        self.state.r2_value = Some(value);
                        if self.state.pressed_r2 && value < TRIGGER_THRESHOLD {
                            event.value = InputValue::Float(0.0);
                            self.state.pressed_r2 = false;
                            true
                        } else if !self.state.pressed_r2 && value > TRIGGER_THRESHOLD {
                            event.value = InputValue::Float(1.0);
                            self.state.pressed_r2 = true;
                            true
                        } else {
                            false
                        }
                    }
                    _ => true,
                }
            }
            // Trigger buttons should be ignored if analog trigger input is
            // detected.
            else if matches!(
                &source_cap,
                Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger))
            ) {
                self.state.l2_value.is_none()
            } else if matches!(
                &source_cap,
                Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger))
            ) {
                self.state.r2_value.is_none()
            }
            // All other translated events should be emitted
            else {
                true
            };

            if include_event {
                translated.push(event);
            }
        }

        translated
    }

    /// Writes the given event to DBus
    fn write_dbus_event(&self, event: DBusEvent) -> Result<(), Box<dyn Error>> {
        // Only send valid events
        let valid = !matches!(event.action, Action::None);
        if !valid {
            return Ok(());
        }

        // DBus events can only be written if there is a DBus path reference.
        let Some(dbus) = self.dbus.as_ref() else {
            return Err("No dbus interface manager exists to send events to".into());
        };

        // Send the input event signal based on the type of value
        let path = dbus.path().to_string();
        let conn = dbus.connection().clone();
        tokio::task::spawn(async move {
            // Get the object instance at the given path so we can send DBus signal
            // updates
            let iface_ref = match conn
                .object_server()
                .interface::<_, TargetDBusInterface>(path.as_str())
                .await
            {
                Ok(refr) => refr,
                Err(e) => {
                    log::error!("Failed to get interface: {e:?}");
                    return;
                }
            };
            let result = match event.value {
                InputValue::Bool(value) => {
                    let value = match value {
                        true => 1.0,
                        false => 0.0,
                    };
                    TargetDBusInterface::input_event(
                        iface_ref.signal_emitter(),
                        event.action.as_string(),
                        value,
                    )
                    .await
                }
                InputValue::Float(value) => {
                    TargetDBusInterface::input_event(
                        iface_ref.signal_emitter(),
                        event.action.as_string(),
                        value,
                    )
                    .await
                }
                InputValue::Touch {
                    index,
                    is_touching,
                    pressure,
                    x,
                    y,
                } => {
                    // Send the input event signal
                    TargetDBusInterface::touch_event(
                        iface_ref.signal_emitter(),
                        event.action.as_string(),
                        index as u32,
                        is_touching,
                        pressure.unwrap_or(1.0),
                        x.unwrap_or(0.0),
                        y.unwrap_or(0.0),
                    )
                    .await
                }
                _ => Ok(()),
            };
            if let Err(e) = result {
                log::error!("Failed to send event: {e:?}");
            }
        });

        Ok(())
    }

    /// Checks if the given button event has changed from the previous state.
    fn is_duplicate_event(&self, event: &NativeEvent) -> bool {
        let InputValue::Bool(value) = event.get_value() else {
            return false;
        };

        let cap = event.as_capability();
        let Some(current) = self.state.buttons.get(&cap) else {
            return false;
        };
        value == *current
    }

    fn update_button_state(&mut self, event: &NativeEvent) {
        let InputValue::Bool(value) = event.get_value() else {
            return;
        };

        let cap = event.as_capability();
        self.state
            .buttons
            .entry(cap)
            .and_modify(|v| *v = value)
            .or_insert(value);
    }
}

impl TargetInputDevice for DBusDevice {
    fn start_dbus_interface(
        &mut self,
        dbus: &mut DBusInterfaceManager,
        _client: TargetDeviceClient,
        _type_id: TargetDeviceTypeId,
    ) {
        self.dbus =
            DBusInterfaceManager::new(dbus.connection().clone(), dbus.path().to_string()).ok();
        let iface = TargetDBusInterface::new();
        dbus.register(iface);
    }

    fn on_composite_device_attached(
        &mut self,
        device: CompositeDeviceClient,
    ) -> Result<(), InputError> {
        self.device = Some(device);
        Ok(())
    }

    fn write_event(
        &mut self,
        event: crate::input::event::native::NativeEvent,
    ) -> Result<(), InputError> {
        log::trace!("Got event to emit: {:?}", event);
        if self.is_duplicate_event(&event) {
            return Ok(());
        }
        self.update_button_state(&event);
        let dbus_events = self.translate_event(event);
        for dbus_event in dbus_events {
            log::trace!("Writing DBus event: {dbus_event:?}");
            self.write_dbus_event(dbus_event)?;
        }

        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<crate::input::capability::Capability>, InputError> {
        let capabilities = vec![
            Capability::DBus(Action::Guide),
            Capability::DBus(Action::Quick),
            Capability::DBus(Action::Quick2),
            Capability::DBus(Action::Context),
            Capability::DBus(Action::Option),
            Capability::DBus(Action::Select),
            Capability::DBus(Action::Accept),
            Capability::DBus(Action::Back),
            Capability::DBus(Action::ActOn),
            Capability::DBus(Action::Left),
            Capability::DBus(Action::Right),
            Capability::DBus(Action::Up),
            Capability::DBus(Action::Down),
            Capability::DBus(Action::L1),
            Capability::DBus(Action::L2),
            Capability::DBus(Action::L3),
            Capability::DBus(Action::R1),
            Capability::DBus(Action::R2),
            Capability::DBus(Action::R3),
            Capability::DBus(Action::VolumeUp),
            Capability::DBus(Action::VolumeDown),
            Capability::DBus(Action::VolumeMute),
            Capability::DBus(Action::Keyboard),
            Capability::DBus(Action::Screenshot),
            Capability::DBus(Action::Touch),
        ];

        Ok(capabilities)
    }
}

impl TargetOutputDevice for DBusDevice {
    fn on_output_capabilities_changed(
        &mut self,
        capabilities: HashSet<OutputCapability>,
    ) -> Result<(), OutputError> {
        log::info!("Output capabilities changed: {capabilities:?}");
        let Some(dbus) = self.dbus.as_mut() else {
            log::warn!("No dbus interface manager set to update interfaces!");
            return Ok(());
        };

        // Look for dbus interfaces to start/stop based on output capability
        let supports_ff = capabilities.contains(&OutputCapability::ForceFeedback);
        let ff_iface_name = ForceFeedbackInterface::<CompositeDeviceClient>::name();

        // Remove output interfaces if they are no longer supported
        if !supports_ff {
            if dbus.has_interface(&ff_iface_name) {
                dbus.unregister(&ff_iface_name);
            }
            return Ok(());
        }
        let Some(device) = self.device.clone() else {
            log::warn!("No composite device was set to start ForceFeedback interface!");
            return Ok(());
        };

        // Start the force feedback interface
        let iface = ForceFeedbackInterface::new(device);
        dbus.register(iface);

        Ok(())
    }
}
