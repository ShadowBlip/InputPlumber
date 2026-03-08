pub mod multicolor;
pub mod rgb;
pub mod single;
use self::multicolor::LedMultiColor;
use self::rgb::LedRgb;
use self::single::LedSingleColor;
use super::{InputError, OutputError, SourceDeviceCompatible, SourceDriver};
use crate::{
    config,
    constants::BUS_SOURCES_PREFIX,
    input::{
        capability::Capability,
        composite_device::client::CompositeDeviceClient,
        info::DeviceInfoRef,
        output_capability::{OutputCapability, LED},
    },
    udev::device::UdevDevice,
};
use std::error::Error;

/// List of available LED drivers, determined by the LED's kernel color ID.
pub enum DriverType {
    /// RGB LED (sysfs color tag `rgb`). Uses `multi_intensity` interface.
    Rgb,
    /// Multicolor LED (sysfs color tag `multicolor` or `multi`). Uses
    /// `multi_intensity` interface with separate color channels.
    MultiColor,
    /// Single-color LED (any other color tag: `green`, `blue`, `red`, etc.).
    /// Brightness-only control; color is fixed in hardware.
    SingleColor,
}

impl DriverType {
    pub fn from_color_tag(tag: &str) -> Self {
        match tag {
            "rgb" => Self::Rgb,
            "multicolor" | "multi" => Self::MultiColor,
            _ => Self::SingleColor,
        }
    }
}

/// [LedDevice] represents an input device using the leds subsystem.
#[derive(Debug)]
pub enum LedDevice {
    Rgb(SourceDriver<LedRgb>),
    MultiColor(SourceDriver<LedMultiColor>),
    SingleColor(SourceDriver<LedSingleColor>),
}

impl SourceDeviceCompatible for LedDevice {
    fn get_device_ref(&self) -> DeviceInfoRef<'_> {
        match self {
            LedDevice::Rgb(source_driver) => source_driver.info_ref(),
            LedDevice::MultiColor(source_driver) => source_driver.info_ref(),
            LedDevice::SingleColor(source_driver) => source_driver.info_ref(),
        }
    }

    fn get_id(&self) -> String {
        match self {
            LedDevice::Rgb(source_driver) => source_driver.get_id(),
            LedDevice::MultiColor(source_driver) => source_driver.get_id(),
            LedDevice::SingleColor(source_driver) => source_driver.get_id(),
        }
    }

    fn client(&self) -> super::client::SourceDeviceClient {
        match self {
            LedDevice::Rgb(source_driver) => source_driver.client(),
            LedDevice::MultiColor(source_driver) => source_driver.client(),
            LedDevice::SingleColor(source_driver) => source_driver.client(),
        }
    }

    async fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            LedDevice::Rgb(source_driver) => source_driver.run().await,
            LedDevice::MultiColor(source_driver) => source_driver.run().await,
            LedDevice::SingleColor(source_driver) => source_driver.run().await,
        }
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        match self {
            LedDevice::Rgb(source_driver) => source_driver.get_capabilities(),
            LedDevice::MultiColor(source_driver) => source_driver.get_capabilities(),
            LedDevice::SingleColor(source_driver) => source_driver.get_capabilities(),
        }
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        match self {
            LedDevice::Rgb(_) | LedDevice::MultiColor(_) => Ok(vec![
                OutputCapability::LED(LED::Color),
                OutputCapability::LED(LED::Brightness),
            ]),
            LedDevice::SingleColor(_) => Ok(vec![OutputCapability::LED(LED::Brightness)]),
        }
    }

    fn get_device_path(&self) -> String {
        match self {
            LedDevice::Rgb(source_driver) => source_driver.get_device_path(),
            LedDevice::MultiColor(source_driver) => source_driver.get_device_path(),
            LedDevice::SingleColor(source_driver) => source_driver.get_device_path(),
        }
    }
}

impl LedDevice {
    /// Create a new [LedDevice] associated with the given device and
    /// composite device. The appropriate driver will be selected based on
    /// the LED's kernel color ID parsed from its sysfs name.
    pub fn new(
        device_info: UdevDevice,
        composite_device: CompositeDeviceClient,
        conf: Option<config::SourceDevice>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let driver_type = LedDevice::get_driver_type(&device_info);
        match driver_type {
            DriverType::Rgb => {
                let device = LedRgb::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::Rgb(source_device))
            }
            DriverType::MultiColor => {
                let device = LedMultiColor::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::MultiColor(source_device))
            }
            DriverType::SingleColor => {
                let device = LedSingleColor::new(device_info.clone())?;
                let source_device =
                    SourceDriver::new(composite_device, device, device_info.into(), conf);
                Ok(Self::SingleColor(source_device))
            }
        }
    }

    /// Return the driver type for the given device info by parsing the
    /// kernel LED color ID from the sysfs name.
    fn get_driver_type(device: &UdevDevice) -> DriverType {
        let sysname = device.sysname();
        log::debug!("Finding driver for LED interface: {sysname}");

        // Parse the color tag from the sysfs name (second-to-last colon segment)
        let color_tag = parse_led_color_tag(&sysname);
        log::debug!("LED color ID: '{color_tag}'");

        DriverType::from_color_tag(color_tag)
    }
}

/// Parse the LED color tag from a kernel sysfs name.
///
/// Kernel LED naming convention is `devicename:colour:function`.
/// The colour is the second-to-last colon-separated segment.
///
/// # Examples
/// - `"0003:057E:2009.0001:green:player-1"` → `"green"`
/// - `"multicolor:chassis"` → `"multicolor"`
/// - `"ayaneo:rgb:joystick_rings"` → `"rgb"`
pub fn parse_led_color_tag(sysname: &str) -> &str {
    let parts: Vec<&str> = sysname.split(':').collect();
    if parts.len() >= 2 {
        parts[parts.len() - 2]
    } else {
        ""
    }
}

/// Returns the DBus path for an [LedDevice] from a device id (E.g. leds://input7__numlock)
pub fn get_dbus_path(id: String) -> String {
    let name = id.replace([':', '-', '.'], "_");
    format!("{}/{}", BUS_SOURCES_PREFIX, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_tag_single_color() {
        assert_eq!(parse_led_color_tag("0003:057E:2009.0001:green:player-1"), "green");
        assert_eq!(parse_led_color_tag("0003:057E:2009.0001:blue:player-5"), "blue");
    }

    #[test]
    fn test_parse_color_tag_multicolor() {
        assert_eq!(parse_led_color_tag("multicolor:chassis"), "multicolor");
    }

    #[test]
    fn test_parse_color_tag_rgb() {
        assert_eq!(parse_led_color_tag("ayaneo:rgb:joystick_rings"), "rgb");
        assert_eq!(parse_led_color_tag("ayn:rgb:joystick_rings"), "rgb");
        assert_eq!(parse_led_color_tag("ally:rgb:joystick_rings"), "rgb");
        assert_eq!(parse_led_color_tag("go_s:rgb:joystick_rings"), "rgb");
        assert_eq!(parse_led_color_tag("zotac:rgb:spectra_zone_0"), "rgb");
    }

    #[test]
    fn test_parse_color_tag_single_segment() {
        assert_eq!(parse_led_color_tag("no_colons_here"), "");
    }
}
