use crate::{
    config::LedFixedColor,
    input::{
        capability::Capability,
        output_event::OutputEvent,
        source::{InputError, OutputError, SourceInputDevice, SourceOutputDevice},
    },
    udev::device::UdevDevice,
};
use std::{error::Error, fmt::Debug, path::PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MultiColorChassisError {
    #[error("Error reading multi_index: {0}")]
    MultiIndexError(std::io::Error),

    #[error("File multi_intensity not found: {0}")]
    MultiIntensityNotFound(PathBuf),

    #[error("Error updating multi_intensity: {0}")]
    MultiIntensityUpdateError(std::io::Error),

    #[error("Unsupported index type: {0}")]
    UnsupportedIndexType(String),
}

enum IndexType {
    #[allow(clippy::upper_case_acronyms)]
    RGB,
}

/// MultiColorChassis source device implementation
pub struct MultiColorChassis {
    multi_intensity_path: PathBuf,
    multi_index_map: Vec<IndexType>,
    current_color: Option<LedFixedColor>,
}

impl MultiColorChassis {
    /// Create a new MultiColorChassis source device with the given udev
    /// device information
    pub fn new(
        device_info: UdevDevice,
        fixed_color: Option<LedFixedColor>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let multi_intensity_path =
            PathBuf::from(device_info.syspath().as_str()).join("multi_intensity");
        if !multi_intensity_path.exists() {
            return Err(Box::new(MultiColorChassisError::MultiIntensityNotFound(
                multi_intensity_path,
            )));
        }

        let multi_index = PathBuf::from(device_info.syspath().as_str()).join("multi_index");
        let contents = std::fs::read_to_string(multi_index)
            .map_err(|err| Box::new(MultiColorChassisError::MultiIndexError(err)))?;
        let multi_index_strings = contents
            .split_whitespace()
            .map(String::from)
            .collect::<Vec<String>>();
        let mut multi_index_map = Vec::<IndexType>::with_capacity(multi_index_strings.len());
        for idx in multi_index_strings.iter() {
            match idx.as_str() {
                "rgb" => multi_index_map.push(IndexType::RGB),
                _ => {
                    return Err(Box::new(MultiColorChassisError::UnsupportedIndexType(
                        idx.clone(),
                    )))
                }
            }
        }

        let result = Self {
            current_color: fixed_color,
            multi_intensity_path,
            multi_index_map,
        };

        // Immediatly set the user-defined color if one is provided
        if let Some(color) = result.current_color.as_ref() {
            result.write_color(color.r, color.g, color.b)?
        };

        Ok(result)
    }

    fn write_color(&self, r: u8, g: u8, b: u8) -> Result<(), Box<dyn Error + Send + Sync>> {
        let contents = std::fs::read_to_string(self.multi_intensity_path.as_path())
            .map_err(|err| Box::new(MultiColorChassisError::MultiIndexError(err)))?;
        let contents = (self
            .multi_index_map
            .iter()
            .zip(contents.split_whitespace())
            .map(|(index_type, _index_value)| match index_type {
                IndexType::RGB => {
                    (((r as u32) << 16u32) | ((g as u32) << 8u32) | (b as u32)).to_string()
                }
            })
            .collect::<Vec<String>>())
        .join(" ");
        Ok(
            std::fs::write(self.multi_intensity_path.as_path(), contents)
                .map_err(|err| Box::new(MultiColorChassisError::MultiIntensityUpdateError(err)))?,
        )
    }
}

impl Debug for MultiColorChassis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiColorChassis").finish()
    }
}

impl SourceInputDevice for MultiColorChassis {
    fn poll(&mut self) -> Result<Vec<crate::input::event::native::NativeEvent>, InputError> {
        Ok(Vec::new())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(Vec::new())
    }
}

impl SourceOutputDevice for MultiColorChassis {
    fn write_event(&mut self, event: OutputEvent) -> Result<(), OutputError> {
        log::trace!("Received output event: {event:?}");
        match event {
            OutputEvent::DualSense(report) => {
                if !report.allow_led_color {
                    return Ok(());
                }
                self.write_color(report.led_red, report.led_green, report.led_blue)?;
            }
            _ => {
                return Ok(());
            }
        }
        Ok(())
    }
}
