pub mod multicolor_chassis;
use std::error::Error;
//use glob_match::glob_match;
use self::multicolor_chassis::MultiColorChassis;
use super::SourceDriver;
use crate::{
    config, constants::BUS_SOURCES_PREFIX, input::composite_device::client::CompositeDeviceClient,
    udev::device::UdevDevice,
};
/// List of available drivers
enum DriverType {
    MultiColorChassis,
}
/// [LedDevice] represents an input device using the leds subsystem.
#[derive(Debug)]
pub enum LedDevice {
    MultiColorChassis(SourceDriver<MultiColorChassis>),
}

impl LedDevice {
    /// Create a new [IioDevice] associated with the given device and
    /// composite device. The appropriate driver will be selected based on
    /// the provided device.
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        config: Option<config::Led>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver_type = LedDevice::get_driver_type(&device_info);
        match driver_type {
            DriverType::MultiColorChassis => {
                let device = MultiColorChassis::new(
                    device_info.clone(),
                    match config {
                        Some(cfg) => cfg.led_fixed_color,
                        None => None,
                    },
                )?;
                let source_device = SourceDriver::new(composite_device, device, device_info);
                Ok(Self::MultiColorChassis(source_device))
            }
        }
    }
    /// Return the driver type for the given device info
    fn get_driver_type(device: &UdevDevice) -> DriverType {
        let device_name = device.name();
        let name = device_name.as_str();
        log::debug!("Finding driver for LED interface: {name}");

        // TODO: for now everthing is a MultiColorChassis
        DriverType::MultiColorChassis
    }
}
/// Returns the DBus path for an [LedDevice] from a device id (E.g. iio:device0)
pub fn get_dbus_path(id: String) -> String {
    let name = id.replace(':', "_");
    format!("{}/{}", BUS_SOURCES_PREFIX, name)
}
