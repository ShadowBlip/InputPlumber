pub mod blocked;
pub mod gamepad;
pub mod keyboard;
pub mod touchscreen;

use std::{collections::HashMap, error::Error, time::Duration};

use evdev::{Device, EventType};
use keyboard::KeyboardEventDevice;
use touchscreen::TouchscreenEventDevice;

use crate::{
    config::{
        self,
        capability_map::{load_capability_mappings, CapabilityMapConfig},
    },
    constants::BUS_SOURCES_PREFIX,
    input::{
        capability::Capability, composite_device::client::CompositeDeviceClient,
        info::DeviceInfoRef, output_capability::OutputCapability,
    },
    udev::device::UdevDevice,
};

use self::{blocked::BlockedEventDevice, gamepad::GamepadEventDevice};

use super::{OutputError, SourceDeviceCompatible, SourceDriver, SourceDriverOptions};

/// List of available drivers
enum DriverType {
    Blocked,
    Gamepad,
    Touchscreen,
    Keyboard,
}

/// [EventDevice] represents an input device using the input event subsystem.
#[derive(Debug)]
pub enum EventDevice {
    Blocked(SourceDriver<BlockedEventDevice>),
    Gamepad(SourceDriver<GamepadEventDevice>),
    Touchscreen(SourceDriver<TouchscreenEventDevice>),
    Keyboard(SourceDriver<KeyboardEventDevice>),
}

impl SourceDeviceCompatible for EventDevice {
    fn get_device_ref(&self) -> DeviceInfoRef {
        match self {
            EventDevice::Blocked(source_driver) => source_driver.info_ref(),
            EventDevice::Gamepad(source_driver) => source_driver.info_ref(),
            EventDevice::Touchscreen(source_driver) => source_driver.info_ref(),
            EventDevice::Keyboard(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            EventDevice::Blocked(source_driver) => source_driver.get_id(),
            EventDevice::Gamepad(source_driver) => source_driver.get_id(),
            EventDevice::Touchscreen(source_driver) => source_driver.get_id(),
            EventDevice::Keyboard(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> super::client::SourceDeviceClient {
        match self {
            EventDevice::Blocked(source_driver) => source_driver.client(),
            EventDevice::Gamepad(source_driver) => source_driver.client(),
            EventDevice::Touchscreen(source_driver) => source_driver.client(),
            EventDevice::Keyboard(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            EventDevice::Blocked(source_driver) => source_driver.run().await,
            EventDevice::Gamepad(source_driver) => source_driver.run().await,
            EventDevice::Touchscreen(source_driver) => source_driver.run().await,
            EventDevice::Keyboard(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, super::InputError> {
        match self {
            EventDevice::Blocked(source_driver) => source_driver.get_capabilities(),
            EventDevice::Gamepad(source_driver) => source_driver.get_capabilities(),
            EventDevice::Touchscreen(source_driver) => source_driver.get_capabilities(),
            EventDevice::Keyboard(source_driver) => source_driver.get_capabilities(),
        }
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        match self {
            EventDevice::Blocked(source_driver) => source_driver.get_output_capabilities(),
            EventDevice::Gamepad(source_driver) => source_driver.get_output_capabilities(),
            EventDevice::Touchscreen(source_driver) => source_driver.get_output_capabilities(),
            EventDevice::Keyboard(source_driver) => source_driver.get_output_capabilities(),
        }
    }

    fn get_device_path(&self) -> String {
        match self {
            EventDevice::Blocked(source_driver) => source_driver.get_device_path(),
            EventDevice::Gamepad(source_driver) => source_driver.get_device_path(),
            EventDevice::Touchscreen(source_driver) => source_driver.get_device_path(),
            EventDevice::Keyboard(source_driver) => source_driver.get_device_path(),
        }
    }
}

impl EventDevice {
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        conf: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let is_blocked = conf.as_ref().and_then(|c| c.blocked).unwrap_or(false);
        let driver_type = EventDevice::get_driver_type(&device_info, is_blocked);

        // Check to see if a capability map was set for this device
        let mut capability_map = None;
        let capability_map_id = conf.clone().and_then(|device| device.capability_map_id);
        if let Some(capability_map_id) = capability_map_id {
            let mapping_configs = load_capability_mappings();
            if let Some(CapabilityMapConfig::V2(config)) = mapping_configs.get(&capability_map_id) {
                capability_map = Some(config.clone());
            }
        }

        match driver_type {
            DriverType::Blocked => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(200),
                    buffer_size: 4096,
                };
                let device = BlockedEventDevice::new(device_info.clone())?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::Blocked(source_device))
            }
            DriverType::Gamepad => {
                let device = GamepadEventDevice::new(device_info.clone(), capability_map)?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::Gamepad(source_device))
            }
            DriverType::Touchscreen => {
                let touch_config = conf
                    .as_ref()
                    .and_then(|c| c.config.clone())
                    .and_then(|c| c.touchscreen);
                let device =
                    TouchscreenEventDevice::new(device_info.clone(), touch_config, capability_map)?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::Touchscreen(source_device))
            }
            DriverType::Keyboard => {
                let device = KeyboardEventDevice::new(device_info.clone(), &conf, capability_map)?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::Keyboard(source_device))
            }
        }
    }

    /// Return the driver type for the given vendor and product
    fn get_driver_type(device: &UdevDevice, is_blocked: bool) -> DriverType {
        // TODO: add implmentations for other classes of evdev devices (e.g.
        // driving wheels, touch, etc.)
        log::debug!("Finding driver for interface: {:?}", device);
        if is_blocked {
            return DriverType::Blocked;
        }

        let properties = device.get_properties();
        if properties.contains_key("ID_INPUT_TOUCHSCREEN") {
            return DriverType::Touchscreen;
        }
        if properties.contains_key("ID_INPUT_JOYSTICK") {
            return DriverType::Gamepad;
        }
        if properties.contains_key("ID_INPUT_KEYBOARD") {
            return DriverType::Keyboard;
        }

        let devnode = device.devnode();
        log::debug!("Unknown input device '{devnode}', falling back to gamepad implementation. Device had udev properties: {properties:?}");

        DriverType::Gamepad
    }
}

/// Returns the DBus object path for evdev devices
pub fn get_dbus_path(handler: String) -> String {
    format!("{}/{}", BUS_SOURCES_PREFIX, handler.clone())
}

/// Returns the evdev capabilities of the input device at the given path (e.g. /dev/input/event0)
pub fn get_capabilities(handler: &str) -> Result<HashMap<EventType, Vec<u16>>, Box<dyn Error>> {
    if !handler.contains("input/event") {
        return Ok(HashMap::new());
    }
    let path = handler;
    log::debug!("Opening device at: {}", path);
    let device = Device::open(path)?;
    let mut capabilities: HashMap<EventType, Vec<u16>> = HashMap::new();

    // Loop through all support events
    let events = device.supported_events();
    for event in events.iter() {
        match event {
            EventType::KEY => {
                let Some(keys) = device.supported_keys() else {
                    continue;
                };
                for key in keys.iter() {
                    capabilities
                        .entry(EventType::KEY)
                        .and_modify(|caps| caps.push(key.0))
                        .or_insert(vec![key.0]);
                }
            }
            EventType::RELATIVE => {
                let Some(rel) = device.supported_relative_axes() else {
                    continue;
                };
                for axis in rel.iter() {
                    capabilities
                        .entry(EventType::RELATIVE)
                        .and_modify(|caps| caps.push(axis.0))
                        .or_insert(vec![axis.0]);
                }
            }
            EventType::ABSOLUTE => {
                let Some(abs) = device.supported_absolute_axes() else {
                    continue;
                };
                for axis in abs.iter() {
                    capabilities
                        .entry(EventType::ABSOLUTE)
                        .and_modify(|caps| caps.push(axis.0))
                        .or_insert(vec![axis.0]);
                }
            }
            EventType::SWITCH => {
                let Some(supported) = device.supported_switches() else {
                    continue;
                };
                for cap in supported.iter() {
                    capabilities
                        .entry(EventType::SWITCH)
                        .and_modify(|caps| caps.push(cap.0))
                        .or_insert(vec![cap.0]);
                }
            }
            EventType::LED => {
                let Some(supported) = device.supported_leds() else {
                    continue;
                };
                for cap in supported.iter() {
                    capabilities
                        .entry(EventType::LED)
                        .and_modify(|caps| caps.push(cap.0))
                        .or_insert(vec![cap.0]);
                }
            }
            EventType::SOUND => {
                let Some(supported) = device.supported_sounds() else {
                    continue;
                };
                for cap in supported.iter() {
                    capabilities
                        .entry(EventType::SOUND)
                        .and_modify(|caps| caps.push(cap.0))
                        .or_insert(vec![cap.0]);
                }
            }
            EventType::FORCEFEEDBACK => {
                let Some(supported) = device.supported_ff() else {
                    continue;
                };
                for cap in supported.iter() {
                    capabilities
                        .entry(EventType::FORCEFEEDBACK)
                        .and_modify(|caps| caps.push(cap.0))
                        .or_insert(vec![cap.0]);
                }
            }
            _ => (),
        }
    }

    Ok(capabilities)
}
