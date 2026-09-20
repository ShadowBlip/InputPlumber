use crate::{
    config,
    drivers::iio_imu::{self, info::MountMatrix},
    input::{
        capability::{Capability, Source},
        event::{native::NativeEvent, value::InputValue},
    },
};

/// List of all capabilities that the iio IMU drivers implement
pub(super) const CAPABILITIES: &[Capability] = &[
    Capability::Accelerometer(Source::Center),
    Capability::Gyroscope(Source::Center),
];

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
pub(super) fn sample_rate_from_config(
    config: Option<&config::SourceDevice>,
) -> Option<f64> {
    if let Some(rate) = config
        .and_then(|c| c.config.as_ref())
        .and_then(|c| c.imu.as_ref())
        .and_then(|c| c.sample_rate)
    {
        return Some(rate);
    }
    #[allow(deprecated)]
    let rate = config.and_then(|c| c.iio.as_ref()).and_then(|i| i.sample_rate)?;
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

/// Translate the given driver events into native events, applying the given
/// per-group scale corrections to the kernel-reported scale.
pub(super) fn translate_events(
    events: Vec<iio_imu::event::Event>,
    accel_correction: f64,
    gyro_correction: f64,
) -> Vec<NativeEvent> {
    events
        .into_iter()
        .map(|event| translate_event(event, accel_correction, gyro_correction))
        .collect()
}

fn translate_event(
    event: iio_imu::event::Event,
    accel_correction: f64,
    gyro_correction: f64,
) -> NativeEvent {
    let (cap, data, correction) = match event {
        iio_imu::event::Event::Accelerometer(data) => (
            Capability::Accelerometer(Source::Center),
            data,
            accel_correction,
        ),
        iio_imu::event::Event::Gyroscope(data) => (
            Capability::Gyroscope(Source::Center),
            data,
            gyro_correction,
        ),
    };
    NativeEvent::new(
        cap,
        InputValue::Vector3 {
            x: Some(data.roll * data.scale * correction),
            y: Some(data.pitch * data.scale * correction),
            z: Some(data.yaw * data.scale * correction),
        },
    )
}
