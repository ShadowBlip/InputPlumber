use std::{collections::HashSet, str::FromStr};

use tokio::sync::mpsc;
use zbus::{
    fdo,
    zvariant::{self, Value},
};
use zbus_macros::interface;

use crate::input::{
    capability::{Capability, Gamepad, Mouse},
    composite_device::{Command, InterceptMode},
    event::{native::NativeEvent, value::InputValue},
};

/// The [CompositeDeviceInterface] provides a DBus interface that can be exposed for managing
/// a [CompositeDevice]. It works by sending command messages to a channel that the
/// [CompositeDevice] is listening on.
pub struct CompositeDeviceInterface {
    tx: mpsc::Sender<Command>,
}

impl CompositeDeviceInterface {
    pub fn new(tx: mpsc::Sender<Command>) -> CompositeDeviceInterface {
        CompositeDeviceInterface { tx }
    }
}

#[interface(name = "org.shadowblip.Input.CompositeDevice")]
impl CompositeDeviceInterface {
    /// Name of the composite device
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        let (sender, mut receiver) = mpsc::channel::<String>(1);
        self.tx
            .send(Command::GetName(sender))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(name) = receiver.recv().await else {
            return Ok("".to_string());
        };

        Ok(name)
    }

    /// Name of the currently loaded profile
    #[zbus(property)]
    async fn profile_name(&self) -> fdo::Result<String> {
        let (sender, mut receiver) = mpsc::channel::<String>(1);
        self.tx
            .send(Command::GetProfileName(sender))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(profile_name) = receiver.recv().await else {
            return Ok("".to_string());
        };

        Ok(profile_name)
    }

    /// Stop the composite device and all target devices
    async fn stop(&self) -> fdo::Result<()> {
        self.tx
            .send(Command::Stop)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    /// Load the device profile from the given path
    async fn load_profile_path(&self, path: String) -> fdo::Result<()> {
        let (sender, mut receiver) = mpsc::channel::<Result<(), String>>(1);
        self.tx
            .send(Command::LoadProfilePath(path, sender))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        let Some(result) = receiver.recv().await else {
            return Err(fdo::Error::Failed(
                "No response from CompositeDevice".to_string(),
            ));
        };

        if let Err(e) = result {
            return Err(fdo::Error::Failed(format!(
                "Failed to load profile: {:?}",
                e
            )));
        }

        Ok(())
    }

    /// Set the target input device types the composite device should emulate,
    /// such as ["gamepad", "mouse", "keyboard"]. This method will stop all
    /// current virtual devices for the composite device and create and attach
    /// new target devices.
    async fn set_target_devices(&self, target_device_types: Vec<String>) -> fdo::Result<()> {
        self.tx
            .send(Command::SetTargetDevices(target_device_types))
            .await
            .map_err(|err| fdo::Error::Failed(err.to_string()))?;
        Ok(())
    }

    /// Directly write to the composite device's target devices with the given event
    fn send_event(&self, event: String, value: zvariant::Value) -> fdo::Result<()> {
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

        self.tx
            .blocking_send(Command::WriteSendEvent(event))
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    /// Directly write to the composite device's target devices with the given button event list
    async fn send_button_chord(&self, mut events: Vec<String>) -> fdo::Result<()> {
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
        events = events.into_iter().rev().collect();
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

        self.tx
            .send(Command::WriteChordEvent(chord))
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

        self.tx
            .send(Command::SetInterceptActivation(activation_caps, target_cap))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;

        Ok(())
    }

    /// List of capabilities that all source devices implement
    #[zbus(property)]
    async fn capabilities(&self) -> fdo::Result<Vec<String>> {
        let (sender, mut receiver) = mpsc::channel::<HashSet<Capability>>(1);
        self.tx
            .send(Command::GetCapabilities(sender))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(capabilities) = receiver.recv().await else {
            return Ok(Vec::new());
        };

        let mut capability_strings = Vec::new();
        for cap in capabilities {
            let str = match cap {
                Capability::Gamepad(gamepad) => match gamepad {
                    Gamepad::Button(button) => format!("Gamepad:Button:{}", button),
                    Gamepad::Axis(axis) => format!("Gamepad:Axis:{}", axis),
                    Gamepad::Trigger(trigger) => format!("Gamepad:Trigger:{}", trigger),
                    Gamepad::Accelerometer => "Gamepad:Accelerometer".to_string(),
                    Gamepad::Gyro => "Gamepad:Gyro".to_string(),
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

    /// List of capabilities that all target devices implement
    #[zbus(property)]
    async fn target_capabilities(&self) -> fdo::Result<Vec<String>> {
        let (sender, mut receiver) = mpsc::channel::<HashSet<Capability>>(1);
        self.tx
            .send(Command::GetTargetCapabilities(sender))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(capabilities) = receiver.recv().await else {
            return Ok(Vec::new());
        };

        let mut capability_strings = Vec::new();
        for cap in capabilities {
            let str = match cap {
                Capability::Gamepad(gamepad) => match gamepad {
                    Gamepad::Button(button) => format!("Gamepad:Button:{}", button),
                    Gamepad::Axis(axis) => format!("Gamepad:Axis:{}", axis),
                    Gamepad::Trigger(trigger) => format!("Gamepad:Trigger:{}", trigger),
                    Gamepad::Accelerometer => "Gamepad:Accelerometer".to_string(),
                    Gamepad::Gyro => "Gamepad:Gyro".to_string(),
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
        let (sender, mut receiver) = mpsc::channel::<Vec<String>>(1);
        self.tx
            .send(Command::GetSourceDevicePaths(sender))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(paths) = receiver.recv().await else {
            return Ok(Vec::new());
        };

        Ok(paths)
    }

    /// The intercept mode of the composite device.
    #[zbus(property)]
    async fn intercept_mode(&self) -> fdo::Result<u32> {
        let (sender, mut receiver) = mpsc::channel::<InterceptMode>(1);
        self.tx
            .send(Command::GetInterceptMode(sender))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(mode) = receiver.recv().await else {
            return Ok(0);
        };

        match mode {
            InterceptMode::None => Ok(0),
            InterceptMode::Pass => Ok(1),
            InterceptMode::Always => Ok(2),
        }
    }

    #[zbus(property)]
    async fn set_intercept_mode(&self, mode: u32) -> zbus::Result<()> {
        let mode = match mode {
            0 => InterceptMode::None,
            1 => InterceptMode::Pass,
            2 => InterceptMode::Always,
            _ => InterceptMode::None,
        };
        self.tx
            .send(Command::SetInterceptMode(mode))
            .await
            .map_err(|err| zbus::Error::Failure(err.to_string()))?;
        Ok(())
    }

    /// Target devices that this [CompositeDevice] is managing
    #[zbus(property)]
    async fn target_devices(&self) -> fdo::Result<Vec<String>> {
        let (sender, mut receiver) = mpsc::channel::<Vec<String>>(1);
        self.tx
            .send(Command::GetTargetDevicePaths(sender))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(paths) = receiver.recv().await else {
            return Ok(Vec::new());
        };

        Ok(paths)
    }

    /// Target dbus devices that this [CompositeDevice] is managing
    #[zbus(property)]
    async fn dbus_devices(&self) -> fdo::Result<Vec<String>> {
        let (sender, mut receiver) = mpsc::channel::<Vec<String>>(1);
        self.tx
            .send(Command::GetDBusDevicePaths(sender))
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        let Some(paths) = receiver.recv().await else {
            return Ok(Vec::new());
        };

        Ok(paths)
    }
}
