extern crate industrial_io as iio;

#[derive(Debug, Clone)]
pub struct Device {
    pub id: Option<String>,
    pub name: Option<String>,
}
