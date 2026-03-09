use crate::{
    input::{
        capability::Capability,
        output_capability::{OutputCapability, LED},
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};
use std::{
    error::Error,
    fmt::Debug,
    fs::{self, read_to_string},
    path::PathBuf,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedSingleColorError {
    #[error("Error reading value: {0}")]
    Read(std::io::Error),

    #[error("Error writing value: {0}")]
    Write(std::io::Error),

    #[error("Path not found: {0}")]
    Path(PathBuf),
}

/// Single-color brightness-only LED source device. Used for LEDs that
/// only support on/off or variable brightness with a fixed hardware
/// color (e.g. green player indicator LEDs on Nintendo Switch Pro
/// Controller).
pub struct LedSingleColor {
    brightness_path: PathBuf,
    #[allow(dead_code)]
    device_info: UdevDevice,
    max_brightness: u8,
}

impl Debug for LedSingleColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedSingleColor")
            .field("brightness_path", &self.brightness_path)
            .field("max_brightness", &self.max_brightness)
            .finish()
    }
}

impl LedSingleColor {
    /// Create a new single-color LED source device with the given udev
    /// device information.
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let brightness_path = PathBuf::from(device_info.syspath().as_str()).join("brightness");
        if !brightness_path.exists() {
            return Err(Box::new(LedSingleColorError::Path(brightness_path)));
        }

        let max_brightness_path =
            PathBuf::from(device_info.syspath().as_str()).join("max_brightness");
        if !max_brightness_path.exists() {
            return Err(Box::new(LedSingleColorError::Path(max_brightness_path)));
        }

        let max_brightness = read_brightness(&max_brightness_path)?;

        log::debug!(
            "LED Simple Device setup complete. max_brightness: {}",
            max_brightness,
        );

        Ok(Self {
            brightness_path,
            device_info,
            max_brightness,
        })
    }

    /// Write the given brightness value, scaled to max_brightness.
    fn write_brightness(&self, brightness: u8) -> Result<(), Box<dyn Error + Send + Sync>> {
        let scaled: u8 = (brightness as u32 * self.max_brightness as u32 / 255) as u8;
        fs::write(&self.brightness_path, scaled.to_string())
            .map_err(|err| Box::new(LedSingleColorError::Write(err)))?;
        Ok(())
    }
}

impl SourceInputDevice for LedSingleColor {
    fn poll(&mut self) -> Result<Vec<crate::input::event::native::NativeEvent>, InputError> {
        Ok(Vec::new())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(Vec::new())
    }
}

impl SourceOutputDevice for LedSingleColor {
    fn write_event(&mut self, event: OutputEvent) -> Result<(), OutputError> {
        log::trace!("LedSingleColor received output event: {event:?}");
        match event {
            OutputEvent::LedSingleColor { brightness } => {
                self.write_brightness(brightness)?;
            }
            OutputEvent::LedRgb { brightness, .. } => {
                self.write_brightness(brightness)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![OutputCapability::LED(LED::Brightness)])
    }
}

/// Read a brightness value from a sysfs path.
fn read_brightness(path: &PathBuf) -> Result<u8, Box<dyn Error + Send + Sync>> {
    let s = read_to_string(path).map_err(LedSingleColorError::Read)?;
    Ok(s.trim().parse()?)
}
