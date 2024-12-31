use crate::{
    config::LedFixedColor,
    input::source::{SourceInputDevice, SourceOutputDevice},
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
    RGB,
}

/// MultiColorChassis source device implementation
pub struct MultiColorChassis {
    multi_intensity_path: PathBuf,
    multi_index_map: Vec<IndexType>,
    fixed_color: Option<LedFixedColor>,
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
            .into_iter()
            .map(|str| String::from(str))
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
            fixed_color,
            multi_intensity_path,
            multi_index_map,
        };

        // Immediatly set the user-defined color if one is provided
        if let Some(color) = result.fixed_color.as_ref() {
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
            .zip(contents.split_whitespace().into_iter())
            .map(|(index_type, index_value)| match index_type {
                IndexType::RGB => {
                    (((r as u32) << 16u32) | ((g as u32) << 8u32) | ((b as u32) << 0u32))
                        .to_string()
                }
                _ => String::from(index_value),
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
    fn poll(
        &mut self,
    ) -> Result<Vec<crate::input::event::native::NativeEvent>, crate::input::source::InputError>
    {
        Ok(Vec::new())
    }

    fn get_capabilities(
        &self,
    ) -> Result<Vec<crate::input::capability::Capability>, crate::input::source::InputError> {
        Ok(Vec::new())
    }
}

impl SourceOutputDevice for MultiColorChassis {
    fn write_event(
        &mut self,
        event: crate::input::output_event::OutputEvent,
    ) -> Result<(), crate::input::source::OutputError> {
        //log::trace!("Received output event: {event:?}");
        let _ = event;
        Ok(())
    }

    fn upload_effect(
        &mut self,
        effect: evdev::FFEffectData,
    ) -> Result<i16, crate::input::source::OutputError> {
        //log::trace!("Received upload effect: {effect:?}");
        let _ = effect;
        Ok(-1)
    }

    fn update_effect(
        &mut self,
        effect_id: i16,
        effect: evdev::FFEffectData,
    ) -> Result<(), crate::input::source::OutputError> {
        //log::trace!("Received update effect: {effect_id:?} {effect:?}");
        let _ = effect;
        let _ = effect_id;
        Ok(())
    }

    fn erase_effect(&mut self, effect_id: i16) -> Result<(), crate::input::source::OutputError> {
        //log::trace!("Received erase effect: {effect_id:?}");
        let _ = effect_id;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), crate::input::source::OutputError> {
        Ok(())
    }
}
