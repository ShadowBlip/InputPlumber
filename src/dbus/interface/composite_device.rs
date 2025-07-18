use std::str::FromStr;

use zbus::{
    fdo,
    zvariant::{self, Value},
    Connection,
};
use zbus_macros::interface;

use crate::{
    config::DeviceProfile,
    input::{
        capability::{Capability, Gamepad, Mouse},
        composite_device::{client::CompositeDeviceClient, InterceptMode},
        event::{native::NativeEvent, value::InputValue},
    },
};

use super::Unregisterable;

/// The [CompositeDeviceInterface] provides a DBus interface that can be exposed for managing
/// a [CompositeDevice]. It works by sending command messages to a channel that the
/// [CompositeDevice] is listening on.
pub struct CompositeDeviceInterface {
    composite_device: CompositeDeviceClient,
    profile: Option<DeviceProfile>,
    profile_path: Option<String>,
}

impl CompositeDeviceInterface {
    pub fn new(
        composite_device: CompositeDeviceClient,
        profile: Option<DeviceProfile>,
        profile_path: Option<String>,
    ) -> CompositeDeviceInterface {
        CompositeDeviceInterface {
            composite_device,
            profile,
            profile_path,
        }
    }
}

#[interface(
    name = "org.shadowblip.Input.CompositeDevice",
    proxy(default_service = "org.shadowblip.InputPlumber",)
)]
impl CompositeDeviceInterface {
    /// Name of the composite device
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        self.composite_device
            .get_name()
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    /// Currently active input layer
    #[zbus(property)]
    async fn active_layer(&self) -> fdo::Result<String> {
        Ok("".to_string())
    }

    /// Name of the currently loaded profile
    #[zbus(property)]
    async fn profile_name(&self) -> fdo::Result<String> {
        let name = self
            .profile
            .as_ref()
            .map(|profile| profile.name.clone())
            .unwrap_or_default();
        Ok(name)
    }

    /// Optional path to the currently loaded profile
    #[zbus(property)]
    async fn profile_path(&self) -> fdo::Result<String> {
        Ok(self.profile_path.clone().unwrap_or_default())
    }

    /// Stop the composite device and all target devices
    async fn stop(&self) -> fdo::Result<()> {
        self.composite_device
            .stop()
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    /// Returns the currently loaded profile encoded in YAML format
    async fn get_profile_yaml(&self) -> fdo::Result<String> {
        let data =
            serde_yaml::to_string(&self.profile).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(data)
    }

    /// Load the device profile from the given path
    async fn load_profile_path(&self, path: String) -> fdo::Result<()> {
        self.composite_device
            .load_profile_path(path)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    /// Load the device profile from the given YAML/JSON string
    async fn load_profile_from_yaml(&self, profile: String) -> fdo::Result<()> {
        self.composite_device
            .load_profile_from_yaml(profile)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    /// Set the target input device types the composite device should emulate,
    /// such as ["gamepad", "mouse", "keyboard"]. This method will stop all
    /// current virtual devices for the composite device and create and attach
    /// new target devices.
    async fn set_target_devices(&self, target_device_types: Vec<String>) -> fdo::Result<()> {
        let mut target_device_type_ids = Vec::with_capacity(target_device_types.len());
        for kind in target_device_types {
            let type_id = kind.as_str().try_into().map_err(|_| {
                fdo::Error::InvalidArgs(format!("Invalid target device type: {kind}"))
            })?;
            target_device_type_ids.push(type_id);
        }
        self.composite_device
            .set_target_devices(target_device_type_ids)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))
    }

    /// Directly write to the composite device's target devices with the given event
    fn send_event(&self, event: String, value: zvariant::Value<'_>) -> fdo::Result<()> {
        let cap = Capability::from_str(event.as_str()).map_err(|_| {
            fdo::Error::Failed(format!(
                "Failed to parse event string {event} into capability."
            ))
        })?;

        let val = match value {
            zvariant::Value::Bool(v) => InputValue::Bool(v),
            zvariant::Value::F64(v) => InputValue::Float(v),
            zvariant::Value::Array(v) => match v.len() {
                2 => {
                    let x_val = v.first().unwrap();
                    let y_val: &Value = v.get(1).unwrap().unwrap();
                    let x = f64::try_from(x_val).map_err(|_| {
                        fdo::Error::Failed("Failed to parse x value into float.".to_string())
                    })?;
                    let y = f64::try_from(y_val).map_err(|_| {
                        fdo::Error::Failed("Failed to parse y value into float.".to_string())
                    })?;
                    InputValue::Vector2 {
                        x: Some(x),
                        y: Some(y),
                    }
                }
                3 => {
                    let x_val = v.first().unwrap();
                    let y_val: &Value = v.get(1).unwrap().unwrap();
                    let z_val: &Value = v.get(2).unwrap().unwrap();
                    let x = f64::try_from(x_val).map_err(|_| {
                        fdo::Error::Failed("Failed to parse x value into float.".to_string())
                    })?;
                    let y = f64::try_from(y_val).map_err(|_| {
                        fdo::Error::Failed("Failed to parse y value into float.".to_string())
                    })?;
                    let z = f64::try_from(z_val).map_err(|_| {
                        fdo::Error::Failed("Failed to parse z value into float.".to_string())
                    })?;
                    InputValue::Vector3 {
                        x: Some(x),
                        y: Some(y),
                        z: Some(z),
                    }
                }
                _ => InputValue::None,
            },
            _ => InputValue::None,
        };

        let event = NativeEvent::new(cap, val);

        self.composite_device
            .blocking_write_send_event(event)
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    /// Directly write to the composite device's target devices with the given button event list
    async fn send_button_chord(&self, events: Vec<String>) -> fdo::Result<()> {
        // Store built native events to send in a command to the CompositeDevice
        let mut chord: Vec<NativeEvent> = Vec::new();

        // Iterate in the given order for press events
        for event_str in events.clone() {
            // Validate the event is valid and create a NativeEvent
            if event_str.contains("Button") || event_str.starts_with("Keyboard") {
                let cap = Capability::from_str(event_str.as_str()).map_err(|_| {
                    fdo::Error::Failed(format!(
                        "Failed to parse event string {event_str} into capability."
                    ))
                })?;
                let val = InputValue::Bool(true);
                let event = NativeEvent::new(cap, val);
                chord.push(event);
            } else {
                return Err(fdo::Error::Failed(format!(
                    "The event '{event_str}' is not a Button capability."
                )));
            };
        }
        // Reverse the order for up events
        let events: Vec<String> = events.into_iter().rev().collect();
        for event_str in events {
            // Create a NativeEvent
            let cap = Capability::from_str(event_str.as_str()).map_err(|_| {
                fdo::Error::Failed(format!(
                    "Failed to parse event string {event_str} into capability."
                ))
            })?;
            let val = InputValue::Bool(false);
            let event = NativeEvent::new(cap, val);
            chord.push(event);
        }

        self.composite_device
            .write_chord(chord)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    async fn set_intercept_activation(
        &self,
        activation_events: Vec<String>,
        target_event: String,
    ) -> fdo::Result<()> {
        let mut activation_caps: Vec<Capability> = Vec::new();

        // Iterate in the given order for press events
        for event_str in activation_events {
            // Validate the event is valid and create a NativeEvent
            if event_str.contains("Button") || event_str.starts_with("Keyboard") {
                let cap = Capability::from_str(event_str.as_str()).map_err(|_| {
                    fdo::Error::Failed(format!(
                        "Failed to parse event string {event_str} into capability."
                    ))
                })?;
                activation_caps.push(cap);
            } else {
                return Err(fdo::Error::Failed(format!(
                    "The event '{event_str}' is not a Button capability."
                )));
            };
        }
        let mut target_cap: Capability = Capability::None;
        if target_event.contains("Button") || target_event.starts_with("Keyboard") {
            let cap = Capability::from_str(target_event.as_str()).map_err(|_| {
                fdo::Error::Failed(format!(
                    "Failed to parse event string {target_event} into capability."
                ))
            })?;
            target_cap = cap
        }

        self.composite_device
            .set_intercept_activation(activation_caps, target_cap)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    /// List of capabilities that all source devices implement
    #[zbus(property)]
    async fn capabilities(&self) -> fdo::Result<Vec<String>> {
        let capabilities = self
            .composite_device
            .get_capabilities()
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        let mut capability_strings = Vec::new();
        for cap in capabilities {
            let str = match cap {
                Capability::Gamepad(gamepad) => match gamepad {
                    Gamepad::Button(button) => format!("Gamepad:Button:{}", button),
                    Gamepad::Axis(axis) => format!("Gamepad:Axis:{}", axis),
                    Gamepad::Trigger(trigger) => format!("Gamepad:Trigger:{}", trigger),
                    Gamepad::Accelerometer => "Gamepad:Accelerometer".to_string(),
                    Gamepad::Gyro => "Gamepad:Gyro".to_string(),
                    Gamepad::Dial(dial) => format!("Gamepad:Dial:{dial}"),
                },
                Capability::Mouse(mouse) => match mouse {
                    Mouse::Motion => "Mouse:Motion".to_string(),
                    Mouse::Button(button) => format!("Mouse:Button:{}", button),
                },
                Capability::Keyboard(key) => format!("Keyboard:{}", key),
                _ => cap.to_string(),
            };
            capability_strings.push(str);
        }

        Ok(capability_strings)
    }

    #[zbus(property)]
    async fn output_capabilities(&self) -> fdo::Result<Vec<String>> {
        let capabilities = self
            .composite_device
            .get_output_capabilities()
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let capability_strings = capabilities
            .into_iter()
            .map(|cap| cap.to_string())
            .collect();

        Ok(capability_strings)
    }

    /// List of capabilities that all target devices implement
    #[zbus(property)]
    async fn target_capabilities(&self) -> fdo::Result<Vec<String>> {
        let capabilities = self
            .composite_device
            .get_target_capabilities()
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        let mut capability_strings = Vec::new();
        for cap in capabilities {
            let str = match cap {
                Capability::Gamepad(gamepad) => match gamepad {
                    Gamepad::Button(button) => format!("Gamepad:Button:{}", button),
                    Gamepad::Axis(axis) => format!("Gamepad:Axis:{}", axis),
                    Gamepad::Trigger(trigger) => format!("Gamepad:Trigger:{}", trigger),
                    Gamepad::Accelerometer => "Gamepad:Accelerometer".to_string(),
                    Gamepad::Gyro => "Gamepad:Gyro".to_string(),
                    Gamepad::Dial(dial) => format!("Gamepad:Dial:{dial}"),
                },
                Capability::Mouse(mouse) => match mouse {
                    Mouse::Motion => "Mouse:Motion".to_string(),
                    Mouse::Button(button) => format!("Mouse:Button:{}", button),
                },
                Capability::Keyboard(key) => format!("Keyboard:{}", key),
                _ => cap.to_string(),
            };
            capability_strings.push(str);
        }

        Ok(capability_strings)
    }

    /// List of source devices that this composite device is processing inputs for
    #[zbus(property)]
    async fn source_device_paths(&self) -> fdo::Result<Vec<String>> {
        let paths = self
            .composite_device
            .get_source_device_paths()
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        Ok(paths)
    }

    /// The intercept mode of the composite device.
    #[zbus(property)]
    async fn intercept_mode(&self) -> fdo::Result<u32> {
        let mode = self
            .composite_device
            .get_intercept_mode()
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        match mode {
            InterceptMode::None => Ok(0),
            InterceptMode::Pass => Ok(1),
            InterceptMode::Always => Ok(2),
            InterceptMode::GamepadOnly => Ok(3),
        }
    }

    #[zbus(property)]
    async fn set_intercept_mode(&self, mode: u32) -> zbus::Result<()> {
        let mode = match mode {
            0 => InterceptMode::None,
            1 => InterceptMode::Pass,
            2 => InterceptMode::Always,
            3 => InterceptMode::GamepadOnly,
            _ => InterceptMode::None,
        };
        self.composite_device
            .set_intercept_mode(mode)
            .await
            .map_err(|err| zbus::Error::Failure(err.to_string()))?;
        Ok(())
    }

    /// Target devices that this [CompositeDevice] is managing
    #[zbus(property)]
    async fn target_devices(&self) -> fdo::Result<Vec<String>> {
        let paths = self
            .composite_device
            .get_target_device_paths()
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        Ok(paths)
    }

    /// Target dbus devices that this [CompositeDevice] is managing
    #[zbus(property)]
    async fn dbus_devices(&self) -> fdo::Result<Vec<String>> {
        let paths = self
            .composite_device
            .get_dbus_device_paths()
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        Ok(paths)
    }
}

impl CompositeDeviceInterface {
    /// Update the profile
    pub fn update_profile(
        conn: &Connection,
        path: &str,
        profile: Option<DeviceProfile>,
        profile_path: Option<String>,
    ) {
        let conn = conn.clone();
        let path = path.to_string();
        tokio::task::spawn(async move {
            // Get the object instance at the given path so we can send DBus signal
            // updates
            let iface_ref = match conn
                .object_server()
                .interface::<_, Self>(path.clone())
                .await
            {
                Ok(iface) => iface,
                Err(e) => {
                    log::error!("Failed to get DBus interface {path}: {e:?}");
                    return;
                }
            };

            let mut iface = iface_ref.get_mut().await;
            iface.profile = profile;
            let result = iface.profile_name_changed(iface_ref.signal_emitter()).await;
            if let Err(e) = result {
                log::error!("Failed to signal property changed: {e}");
            }
            iface.profile_path = profile_path;
            let result = iface.profile_path_changed(iface_ref.signal_emitter()).await;
            if let Err(e) = result {
                log::error!("Failed to signal property changed: {e}");
            }
        });
    }
}

impl Unregisterable for CompositeDeviceInterface {}
