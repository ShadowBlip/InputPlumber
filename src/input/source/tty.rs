use std::{error::Error, time::Duration};

pub mod oxflyserial;
pub mod oxp_x1_serial;

use super::{InputError, OutputError, SourceDeviceCompatible, SourceDriver};
use crate::{
    config,
    constants::BUS_SOURCES_PREFIX,
    drivers::oxp_tty::{self, OxpDriverType},
    input::{
        capability::Capability,
        composite_device::client::CompositeDeviceClient,
        info::DeviceInfoRef,
        output_capability::OutputCapability,
        source::{
            tty::{oxflyserial::OneXFlySerial, oxp_x1_serial::OxpX1Serial},
            SourceDriverOptions,
        },
    },
    udev::device::UdevDevice,
};

/// List of available drivers
enum DriverType {
    Unknown,
    OneXFlySerial,
    OxpX1Serial,
}

/// [TtyDevice] represents an input device using the tty subsystem.
#[derive(Debug)]
pub enum TtyDevice {
    OneXFlySerial(SourceDriver<OneXFlySerial>),
    OxpX1Serial(SourceDriver<OxpX1Serial>),
}

impl SourceDeviceCompatible for TtyDevice {
    fn get_device_ref(&self) -> DeviceInfoRef<'_> {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.info_ref(),
            TtyDevice::OxpX1Serial(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.get_id(),
            TtyDevice::OxpX1Serial(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> super::client::SourceDeviceClient {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.client(),
            TtyDevice::OxpX1Serial(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.run().await,
            TtyDevice::OxpX1Serial(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.get_capabilities(),
            TtyDevice::OxpX1Serial(source_driver) => source_driver.get_capabilities(),
        }
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![])
    }

    fn get_device_path(&self) -> String {
        match self {
            TtyDevice::OneXFlySerial(source_driver) => source_driver.get_device_path(),
            TtyDevice::OxpX1Serial(source_driver) => source_driver.get_device_path(),
        }
    }
}

impl TtyDevice {
    /// Create a new [TtyDevice] associated with the given device and
    /// composite device. The appropriate driver will be selected based on
    /// the provided device.
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        conf: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver_type = TtyDevice::get_driver_type(&device_info);
        match driver_type {
            DriverType::Unknown => Err("No driver for tty interface found".into()),
            DriverType::OneXFlySerial => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(oxp_tty::TTY_TIMEOUT),
                    buffer_size: 4096,
                };
                let device = OneXFlySerial::new(device_info.clone(), OxpDriverType::OneXFly)?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::OneXFlySerial(source_device))
            }
            DriverType::OxpX1Serial => {
                let options = SourceDriverOptions {
                    poll_rate: Duration::from_millis(oxp_tty::TTY_TIMEOUT),
                    buffer_size: 4096,
                };
                let device = OxpX1Serial::new(device_info.clone(), OxpDriverType::OxpX1)?;
                let source_device = SourceDriver::new_with_options(
                    composite_device,
                    device,
                    device_info.into(),
                    options,
                    conf,
                );
                Ok(Self::OxpX1Serial(source_device))
            }
        }
    }

    /// Return the driver type for the given device info
    fn get_driver_type(device: &UdevDevice) -> DriverType {
        let device_name = device.sysname();
        let name = device_name.as_str();
        let port_str = device.get_attribute_from_tree("port");
        let port = port_str
            .and_then(|ps| {
                let ps = ps.strip_prefix("0x").unwrap_or(ps.as_str());
                u16::from_str_radix(ps, 16).ok()
            })
            .unwrap_or_default();

        let vid_str = device.get_attribute_from_tree("idVendor");
        let vid = vid_str
            .and_then(|vs| {
                let vs = vs.strip_prefix("0x").unwrap_or(vs.as_str());
                u16::from_str_radix(vs, 16).ok()
            })
            .unwrap_or_default();

        let pid_str = device.get_attribute_from_tree("idProduct");
        let pid = pid_str
            .and_then(|ps| {
                let ps = ps.strip_prefix("0x").unwrap_or(ps.as_str());
                u16::from_str_radix(ps, 16).ok()
            })
            .unwrap_or_default();

        let if_str = device.get_attribute_from_tree("bInterfaceNumber");
        let interface = if_str
            .and_then(|is| {
                let is = is.strip_prefix("0x").unwrap_or(is.as_str());
                u16::from_str_radix(is, 16).ok()
            })
            .unwrap_or_default();

        log::debug!(
            "Finding driver for tty interface: {name}, port: {:04x?}",
            port
        );

        match oxp_tty::get_driver_type(port, vid, pid, interface) {
            OxpDriverType::Unknown => DriverType::Unknown,
            OxpDriverType::OneXFly => DriverType::OneXFlySerial,
            OxpDriverType::OxpX1 => DriverType::OxpX1Serial,
        }
    }
}

/// Returns the DBus path for an [TtyDevice] from a device id (E.g. tty://ttyS0)
pub fn get_dbus_path(id: String) -> String {
    let name = id.replace(':', "_").replace("-", "_");
    format!("{}/{}", BUS_SOURCES_PREFIX, name)
}
