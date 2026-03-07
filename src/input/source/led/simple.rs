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
pub enum LedSimpleError {
    #[error("Error reading value: {0}")]
    Read(std::io::Error),

    #[error("Error writing value: {0}")]
    Write(std::io::Error),

    #[error("Path not found: {0}")]
    Path(PathBuf),
}

/// Simple brightness-only LED source device. Used for LEDs that only
/// support on/off or variable brightness (e.g. player indicator LEDs
/// on Nintendo Switch Pro Controller).
pub struct LedSimple {
    brightness_path: PathBuf,
    #[allow(dead_code)]
    device_info: UdevDevice,
    max_brightness: u8,
}

impl Debug for LedSimple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedSimple")
            .field("brightness_path", &self.brightness_path)
            .field("max_brightness", &self.max_brightness)
            .finish()
    }
}

impl LedSimple {
    /// Create a new simple LED source device with the given udev device
    /// information.
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let brightness_path = PathBuf::from(device_info.syspath().as_str()).join("brightness");
        if !brightness_path.exists() {
            return Err(Box::new(LedSimpleError::Path(brightness_path)));
        }

        let max_brightness_path =
            PathBuf::from(device_info.syspath().as_str()).join("max_brightness");
        if !max_brightness_path.exists() {
            return Err(Box::new(LedSimpleError::Path(max_brightness_path)));
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
            .map_err(|err| Box::new(LedSimpleError::Write(err)))?;
        Ok(())
    }
}

impl SourceInputDevice for LedSimple {
    fn poll(&mut self) -> Result<Vec<crate::input::event::native::NativeEvent>, InputError> {
        Ok(Vec::new())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(Vec::new())
    }
}

impl SourceOutputDevice for LedSimple {
    fn write_event(&mut self, event: OutputEvent) -> Result<(), OutputError> {
        log::trace!("LedSimple received output event: {event:?}");
        if let OutputEvent::Led { brightness, .. } = event {
            self.write_brightness(brightness)?;
        }
        Ok(())
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![OutputCapability::LED(LED::Brightness)])
    }
}

/// Read a brightness value from a sysfs path.
fn read_brightness(path: &PathBuf) -> Result<u8, Box<dyn Error + Send + Sync>> {
    let s = read_to_string(path).map_err(LedSimpleError::Read)?;
    Ok(s.trim().parse()?)
}
