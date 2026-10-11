use crate::{
    config,
    drivers::iio_imu::{self, event, info::MountMatrix},
    input::{
        capability::{Capability, Source},
        event::{native::NativeEvent, value::InputValue},
        source::EventCapability,
    },
};

/// Build a mount matrix override from the per-device config, if one is set.
/// The `config.imu.mount_matrix` option takes precedence over the
/// deprecated `iio.mount_matrix` option.
pub(super) fn mount_matrix_from_config(
    config: Option<&config::SourceDevice>,
) -> Option<MountMatrix> {
    let matrix = config
        .and_then(|c| c.config.as_ref())
        .and_then(|c| c.imu.as_ref())
        .and_then(|c| c.mount_matrix.as_ref())
        .or_else(|| deprecated_mount_matrix(config));
    matrix.map(to_mount_matrix)
}

#[allow(deprecated)]
fn deprecated_mount_matrix(config: Option<&config::SourceDevice>) -> Option<&config::MountMatrix> {
    let matrix = config
        .and_then(|c| c.iio.as_ref())
        .and_then(|i| i.mount_matrix.as_ref())?;
    log::warn!("config option iio.mount_matrix is deprecated, use <SourceDevice>.config.imu.mount_matrix instead");
    Some(matrix)
}

/// The sample rate requested in the per-device config, if one is set.
/// The `config.imu.sample_rate` option takes precedence over the
/// `iio.sample_rate` option.
pub(super) fn sample_rate_from_config(config: Option<&config::SourceDevice>) -> Option<f64> {
    if let Some(rate) = config
        .and_then(|c| c.config.as_ref())
        .and_then(|c| c.imu.as_ref())
        .and_then(|c| c.sample_rate)
    {
        return Some(rate);
    }
    #[allow(deprecated)]
    let rate = config
        .and_then(|c| c.iio.as_ref())
        .and_then(|i| i.sample_rate)?;
    log::warn!("config option iio.sample_rate is deprecated, use <SourceDevice>.config.imu.sample_rate instead");
    Some(rate)
}

/// The scale correction factors requested in the per-device config.
/// Each defaults to 1.0 when the option is not set.
pub(super) fn corrections_from_config(config: Option<&config::SourceDevice>) -> (f64, f64) {
    let imu = config
        .and_then(|c| c.config.as_ref())
        .and_then(|c| c.imu.as_ref());
    (
        imu.and_then(|c| c.accel_correction).unwrap_or(1.0),
        imu.and_then(|c| c.gyro_correction).unwrap_or(1.0),
    )
}

fn to_mount_matrix(matrix: &config::MountMatrix) -> MountMatrix {
    MountMatrix {
        x: (matrix.x[0], matrix.x[1], matrix.x[2]),
        y: (matrix.y[0], matrix.y[1], matrix.y[2]),
        z: (matrix.z[0], matrix.z[1], matrix.z[2]),
    }
}

/// Translate the given driver events into native events
pub(super) fn translate_events(events: Vec<iio_imu::event::Event>) -> Vec<NativeEvent> {
    events.into_iter().filter_map(translate_event).collect()
}

fn translate_event(event: iio_imu::event::Event) -> Option<NativeEvent> {
    let value = event.to_value()?;
    Some(NativeEvent::new(event.to_capability(), value))
}

impl EventCapability for event::Event {
    fn to_capability(&self) -> Capability {
        match self {
            event::Event::Accelerometer(_) => Capability::Accelerometer(Source::Center),
            event::Event::Gyroscope(_) => Capability::Gyroscope(Source::Center),
        }
    }

    fn to_value(&self) -> Option<InputValue> {
        let data = match self {
            event::Event::Accelerometer(data) | event::Event::Gyroscope(data) => data,
        };
        Some(InputValue::Vector3 {
            x: Some(data.roll * data.scale),
            y: Some(data.pitch * data.scale),
            z: Some(data.yaw * data.scale),
        })
    }

    fn from_capability(capability: &Capability) -> Option<Self> {
        match capability {
            Capability::Accelerometer(Source::Center) => {
                Some(event::Event::Accelerometer(event::AxisData::default()))
            }
            Capability::Gyroscope(Source::Center) => {
                Some(event::Event::Gyroscope(event::AxisData::default()))
            }
            _ => None,
        }
    }
}

/// List of all capabilities that the iio IMU drivers implement
pub(super) const CAPABILITIES: &[Capability] = &[
    Capability::Accelerometer(Source::Center),
    Capability::Gyroscope(Source::Center),
];
