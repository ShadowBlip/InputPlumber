/// Events that can be emitted by the BMI IMU
#[derive(Clone, Debug)]
pub enum Event {
    /// Accelerometer events measure the acceleration in a particular direction
    /// in units of meters per second. It is generally used to determine which
    /// direction is "down" due to the accelerating force of gravity.
    Accelerometer(AxisData),
    /// Gyro events measure the angular velocity in rads per second.
    Gyro(AxisData),
}

/// AxisData represents the raw (offset-corrected, pre-scale) accelerometer or
/// gyro (x, y, z) counts, plus the kernel-reported scale to convert them to
/// physical units. The scale is assumed uniform across x/y/z within one
/// accel or gyro group, matching how the mount-matrix rotation already mixes
/// these axes together.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AxisData {
    pub roll: f64,
    pub pitch: f64,
    pub yaw: f64,
    pub scale: f64,
}
