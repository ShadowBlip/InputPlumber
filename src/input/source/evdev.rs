pub mod blocked;
pub mod gamepad;

use std::{error::Error, time::Duration};

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
