pub mod blocked;
pub mod gamepad;

use std::{collections::HashMap, error::Error, path::Path, time::Duration};

use evdev::{Device, EventType};

use crate::{
    constants::BUS_SOURCES_PREFIX, input::composite_device::client::CompositeDeviceClient,
    udev::device::UdevDevice,
};

use self::{blocked::BlockedEventDevice, gamepad::GamepadEventDevice};

use super::{SourceDriver, SourceDriverOptions};

/// List of available drivers
enum DriverType {
    Blocked,
    Gamepad,
}

/// [EventDevice] represents an input device using the input event subsystem.
#[derive(Debug)]
pub enum EventDevice {
    Blocked(SourceDriver<BlockedEventDevice>),
    Gamepad(SourceDriver<GamepadEventDevice>),
}

impl EventDevice {
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        is_blocked: bool,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver_type = EventDevice::get_driver_type(&device_info, is_blocked);

        match driver_type {
            DriverType::Blocked => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(200),
                    buffer_size: 4096,
                };
                let device = BlockedEventDevice::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new_with_options(composite_device, device, device_info, options);
                Ok(Self::Blocked(source_device))
            }
            DriverType::Gamepad => {
                let device = GamepadEventDevice::new(device_info.clone())?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::Gamepad(source_device))
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
        DriverType::Gamepad
    }
}

/// Returns the DBus object path for evdev devices
pub fn get_dbus_path(handler: String) -> String {
    format!("{}/{}", BUS_SOURCES_PREFIX, handler.clone())
}

/// Returns the evdev capabilities of the input device at the given path (e.g. /dev/input/event0)
pub fn get_capabilities(handler: &str) -> Result<HashMap<EventType, Vec<u16>>, Box<dyn Error>> {
    if (!handler.contains("input/event")) || (!Path::new(handler).exists()) {
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
