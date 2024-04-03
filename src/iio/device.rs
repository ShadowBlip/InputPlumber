use std::error::Error;

extern crate industrial_io as iio;

#[derive(Debug, Clone)]
pub struct Device {
    pub id: Option<String>,
    pub name: Option<String>,
}

/// Returns all iio devices on the system
pub fn list_devices() -> Result<Vec<Device>, Box<dyn Error>> {
    let ctx = iio::Context::new()?;
    let devices: Vec<Device> = ctx
        .devices()
        .map(|dev| Device {
            id: dev.id(),
            name: dev.name(),
        })
        .collect();
    Ok(devices)
}
