use crate::{
    input::{
        capability::Capability,
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};
use std::{
    collections::HashMap,
    error::Error,
    fmt::Debug,
    fs::{self, read_to_string},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedMcError {
    #[error("Error writing value: {0}")]
    Read(std::io::Error),

    #[error("Error writing value: {0}")]
    Write(std::io::Error),

    #[error("Path not found: {0}")]
    Path(PathBuf),

    #[error("Unsupported index type: {0}")]
    UnsupportedColor(String),
}

#[derive(Eq, Hash, PartialEq, Clone, Copy, Debug)]
enum ColorType {
    Amber,
    Blue,
    Cyan,
    Green,
    Infrared,
    Lime,
    Multi,
    Orange,
    Pink,
    Purple,
    Rgb,
    Red,
    Violet,
    White,
    Yellow,
}

impl TryFrom<String> for ColorType {
    type Error = LedMcError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "amber" => Ok(ColorType::Amber),
            "blue" => Ok(ColorType::Blue),
            "cyan" => Ok(ColorType::Cyan),
            "green" => Ok(ColorType::Green),
            "infrared" => Ok(ColorType::Infrared),
            "lime" => Ok(ColorType::Lime),
            "multi" => Ok(ColorType::Multi),
            "orange" => Ok(ColorType::Orange),
            "pink" => Ok(ColorType::Pink),
            "purple" => Ok(ColorType::Purple),
            "red" => Ok(ColorType::Red),
            "rgb" => Ok(ColorType::Rgb),
            "violet" => Ok(ColorType::Violet),
            "white" => Ok(ColorType::White),
            "yellow" => Ok(ColorType::Yellow),
            _ => Err(LedMcError::UnsupportedColor(value)),
        }
    }
}

/// MultiColorChassis source device implementation
pub struct LedMultiColor {
    brightness_path: PathBuf,
    #[allow(dead_code)]
    current_brightness: u8,
    #[allow(dead_code)]
    current_color: Vec<HashMap<ColorType, u8>>,
    #[allow(dead_code)]
    device_info: UdevDevice,
    max_brightness: u8,
    multi_index_map: Vec<ColorType>,
    multi_intensity_path: PathBuf,
}

impl LedMultiColor {
    /// Create a new LedMultiColor source device with the given udev
    /// device information
    pub fn new(device_info: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let brightness_path = PathBuf::from(device_info.syspath().as_str()).join("brightness");
        if !brightness_path.exists() {
            return Err(Box::new(LedMcError::Path(brightness_path)));
        }

        let current_brightness: u8 = read_brightness(brightness_path.as_path())?;

        let max_brightness_path =
            PathBuf::from(device_info.syspath().as_str()).join("max_brightness");
        if !max_brightness_path.exists() {
            return Err(Box::new(LedMcError::Path(brightness_path)));
        }

        let max_brightness: u8 = read_brightness(max_brightness_path.as_path())?;

        let multi_intensity_path =
            PathBuf::from(device_info.syspath().as_str()).join("multi_intensity");
        if !multi_intensity_path.exists() {
            return Err(Box::new(LedMcError::Path(multi_intensity_path)));
        }

        let multi_index_path = PathBuf::from(device_info.syspath().as_str()).join("multi_index");
        let multi_index_map = read_color_index(&multi_index_path)?;

        let current_color = read_color(multi_intensity_path.as_path(), &multi_index_map)?;

        log::debug!(
            "LED MultiColor Device setup complete. brightness: {}, max_brightness {}, color {:?},",
            current_brightness,
            max_brightness,
            current_color
        );

        let result = Self {
            brightness_path,
            current_brightness,
            current_color,
            device_info,
            max_brightness,
            multi_index_map,
            multi_intensity_path,
        };

        Ok(result)
    }

    /// Writes the given red, green, and blue (only) values to the device. Any other color is set
    /// to 0. Used for RGB only implementations we can't adjust (i.e. DualSense)
    fn write_color(&self, r: u8, g: u8, b: u8) -> Result<(), Box<dyn Error + Send + Sync>> {
        let contents = read_to_string(self.multi_intensity_path.as_path())
            .map_err(|err| Box::new(LedMcError::Read(err)))?;
        let contents = (self
            .multi_index_map
            .iter()
            .zip(contents.split_whitespace())
            .map(|(index_type, _)| match index_type {
                ColorType::Blue => b.to_string(),
                ColorType::Green => g.to_string(),
                ColorType::Multi | ColorType::Rgb => {
                    (((r as u32) << 16u32) | ((g as u32) << 8u32) | (b as u32)).to_string()
                }
                ColorType::Red => r.to_string(),
                _ => 0.to_string(),
            })
            .collect::<Vec<String>>())
        .join(" ");
        Ok(fs::write(self.multi_intensity_path.as_path(), contents)
            .map_err(|err| Box::new(LedMcError::Write(err)))?)
    }

    /// Writes the given brightness valeu to the brightness path, but scales it to the max
    /// brightness value. This assumes userspace will be sending a full u8
    fn write_brightness(&self, brightness: u8) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Scale brightness to max_brightness, cast avoids overflow
        let brightness: u8 = (brightness as u32 * self.max_brightness as u32 / 255) as u8;

        Ok(fs::write(
            self.brightness_path.as_path(),
            brightness.to_string().as_str(),
        )
        .map_err(|err| Box::new(LedMcError::Write(err)))?)
    }

    // Retain below functions for dbus
    /// Reads the multi_intensity path and returns a Vec<HashMap<ColorType, u8>> with the current color setting
    /// "rgb" and "multi" ColorType's will be converted into three u8's for Red, Green, and Blue.
    #[allow(dead_code)]
    fn read_color(&self) -> Result<Vec<HashMap<ColorType, u8>>, LedMcError> {
        read_color(
            self.multi_intensity_path.as_path(),
            self.multi_index_map.as_ref(),
        )
    }

    /// Reads the brightness path and returns a u8 with the current brightness setting
    #[allow(dead_code)]
    fn read_brightness(&self) -> Result<u8, Box<dyn Error + Send + Sync>> {
        read_brightness(self.brightness_path.as_path())
    }

    /// Writes the given Vec<u32> color mapping to the device. The color mapping should be
    /// created following the multi_index_map of the device. Ensure that Multi and Rgb ColorType
    /// indexes are accounted for if they were translated to Red Green Blue ColorType before
    /// sending to this function.
    #[allow(dead_code)]
    fn write_colors(&self, colors: Vec<u32>) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Check for the correct quantity of colors.
        if colors.len() != self.multi_index_map.len() {
            return Err(format!(
                "Attempted to write {} colors to path that takes {} colors.",
                colors.len(),
                self.multi_index_map.len()
            )
            .into());
        }
        // Loop through all the colors, Then add the values of each to a string.
        let mut raw_values = Vec::with_capacity(colors.len());

        for color in colors.iter() {
            raw_values.push(color.to_string());
        }

        let contents = raw_values.join(" ");

        Ok(fs::write(self.multi_intensity_path.as_path(), contents)
            .map_err(|err| Box::new(LedMcError::Write(err)))?)
    }

    /// Converts a generic dbus provided Vec<u8> to a matching Vec<u32> based on the device's
    /// multi_index_map. This accounts for converting three u8 values into a singe Rgb or Multi
    /// value.
    #[allow(dead_code)]
    fn dbus_write_colors(&self, colors: Vec<u8>) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut values = vec![];
        let mut visited_index = vec![];
        for (pos, color) in self.multi_index_map.iter().enumerate() {
            if visited_index.contains(&pos) {
                continue;
            }
            match color {
                ColorType::Multi | ColorType::Rgb => {
                    let value: u32 = ((colors[pos] as u32) << 16u32)
                        | ((colors[pos + 1] as u32) << 8u32)
                        | (colors[pos + 2] as u32);
                    values.push(value);
                    visited_index.push(pos);
                    visited_index.push(pos + 1);
                    visited_index.push(pos + 2);
                }
                _ => {
                    values.push(colors[pos] as u32);
                    visited_index.push(pos);
                }
            }
        }

        self.write_colors(values)
    }
}

impl Debug for LedMultiColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiColorChassis").finish()
    }
}

impl SourceInputDevice for LedMultiColor {
    fn poll(&mut self) -> Result<Vec<crate::input::event::native::NativeEvent>, InputError> {
        Ok(Vec::new())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(Vec::new())
    }
}

impl SourceOutputDevice for LedMultiColor {
    fn write_event(&mut self, event: OutputEvent) -> Result<(), OutputError> {
        log::trace!("Received output event: {event:?}");
        match event {
            OutputEvent::DualSense(report) => {
                if !report.allow_led_color {
                    return Ok(());
                }
                self.write_color(report.led_red, report.led_green, report.led_blue)?;
                self.write_brightness(255)?; // Always set max brightness as DS uses only RGB.
            }
            _ => {
                return Ok(());
            }
        }
        Ok(())
    }
}

/// Reads the multi_intensity path and returns a Vec<HashMap<ColorType, u8>> with the current color setting.
/// "rgb" and "multi" ColorType's will be converted into three u8's for Red, Green, and Blue.
fn read_color(path: &Path, index: &[ColorType]) -> Result<Vec<HashMap<ColorType, u8>>, LedMcError> {
    let mut current_color: Vec<HashMap<ColorType, u8>> = Vec::new();

    let color_contents = read_to_string(path).map_err(LedMcError::Read)?;
    let color_vals = color_contents
        .split_whitespace()
        .map(|val| val.to_string().parse().unwrap_or_default())
        .collect::<Vec<u32>>();

    for (pos, e) in index.iter().enumerate() {
        let mut color_map: HashMap<ColorType, u8> = HashMap::new();

        match e {
            ColorType::Multi | ColorType::Rgb => {
                let rgb: [u8; 4] = color_vals[pos].to_be_bytes();
                log::debug!("Byte array: {rgb:?}");
                // 0th index is null
                color_map.insert(ColorType::Red, rgb[1]);
                color_map.insert(ColorType::Green, rgb[2]);
                color_map.insert(ColorType::Blue, rgb[3]);
            }
            _ => {
                color_map.insert(*e, color_vals[pos] as u8);
            }
        }
        current_color.insert(pos, color_map);
    }

    Ok(current_color)
}

/// Reads the brightness path and returns a u8 with the current brightness setting
fn read_brightness(path: &Path) -> Result<u8, Box<dyn Error + Send + Sync>> {
    let mut brightstring = read_to_string(path)?;
    brightstring = brightstring.trim().to_string();
    log::debug!("Got brightness value: {brightstring}");
    Ok(brightstring.parse()?)
}

/// Reads the multi_index path and returns a Vec<ColorType> of the configured device.
fn read_color_index(path: &Path) -> Result<Vec<ColorType>, LedMcError> {
    let mi_contents = read_to_string(path).map_err(LedMcError::Read)?;
    let multi_index_strings = mi_contents
        .split_whitespace()
        .map(String::from)
        .collect::<Vec<String>>();
    let mut multi_index_map = Vec::<ColorType>::with_capacity(multi_index_strings.len());
    for idx in multi_index_strings.iter() {
        multi_index_map.push(idx.to_string().try_into()?);
    }

    Ok(multi_index_map)
}
