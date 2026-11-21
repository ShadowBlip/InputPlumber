//! Guided validation of a device's inertial sensors. A fixed sequence of poses and
//! rotations is measured on every sensor at once, and the mount matrix those readings
//! imply is shown together with the configuration change to make. Nothing is written.

use std::{
    cmp::Ordering,
    collections::HashMap,
    error::Error,
    time::{Duration, Instant},
};

use packed_struct::PackedStruct;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Direction, Layout},
    prelude::*,
    symbols::border,
    widgets::{Block, Paragraph, Widget},
};
use serde::Serialize;
use tokio::sync::mpsc;
use zbus::Connection;

use crate::{
    cli::ui::InterfaceCommand,
    config::{
        path::{get_devices_paths, get_multidir_sorted_files},
        CompositeDeviceConfig, MountMatrix,
    },
    dbus::interface::composite_device::CompositeDeviceInterfaceProxy,
    drivers::unified_gamepad::{
        capability::InputCapability,
        reports::{
            input_capability_report::InputCapabilityReport, input_data_report::InputDataReport,
        },
        value::Value,
        UNIFIED_GAMEPAD_ACCEL_RAW_TO_MPS2, UNIFIED_GAMEPAD_GYRO_RAW_TO_RAD_S,
    },
};

use super::debug_target::{self, SavedState};
use super::MenuWidget;

/// Acceleration due to gravity in meters per second squared.
pub const GRAVITY_MPS2: f64 = 9.80665;

/// Formats a matrix entry without a pointless trailing ".0".
fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        return format!("{}", value as i32);
    }
    format!("{value}")
}

/// How long a resting pose is measured, in wall-clock time so any report rate feels the same.
const REST_DURATION: Duration = Duration::from_millis(2000);

/// How long a rotation is measured, longer than a pose since the motion takes a second or two.
const SPIN_DURATION: Duration = Duration::from_millis(3000);

/// Smallest dominant-axis acceleration, in g, that a resting pose may show.
pub const REST_DOMINANT_G: f64 = 0.60;

/// Largest acceleration, in g, that an orthogonal axis may show.
pub const REST_OFF_AXIS_G: f64 = 0.35;

/// Smallest angular velocity, in radians per second, counted as motion.
pub const SPIN_MIN_RAD_S: f64 = 0.50;

/// Rotation samples weaker than this are ignored so holding still does not dilute the measurement.
pub const SPIN_SAMPLE_FLOOR_RAD_S: f64 = 1.00;

/// Largest share of the spin axis that an orthogonal axis may reach.
pub const SPIN_ORTHOGONAL_RATIO: f64 = 0.40;

/// The rotation each step asks for, which the integrated angle is compared against.
pub const ROTATION_TARGET_DEG: f64 = 90.0;

/// How far the integrated angle may sit from 90 degrees; further out is a gyro scale problem.
pub const ROTATION_TOLERANCE_DEG: f64 = 22.5;

/// How far the integrated vector may sit from the instructed axis, catching a wrong axis.
pub const ROTATION_AXIS_TOL_DEG: f64 = 30.0;

/// The longest gap that still counts as motion. A delta filtered source goes quiet when the
/// device stops moving, so a longer gap adds nothing and caps the slowest stream that integrates.
pub const ROTATION_MAX_GAP_SECS: f64 = 0.5;

/// Three numbers with vector math on them, used for readings and expected directions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    /// A vector from three components.
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Component-wise sum of two vectors.
    pub fn add(&self, other: &Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    /// Every component multiplied by one factor.
    pub fn scale(&self, factor: f64) -> Vec3 {
        Vec3::new(self.x * factor, self.y * factor, self.z * factor)
    }

    /// Dot product of two vectors.
    pub fn dot(&self, other: &Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Length of the vector.
    pub fn norm(&self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Unit vector in this direction, or None when the vector is degenerate.
    pub fn normalized(&self) -> Option<Vec3> {
        let norm = self.norm();
        if norm < 1.0e-6 {
            return None;
        }
        Some(Vec3::new(self.x / norm, self.y / norm, self.z / norm))
    }

    /// Values expressed in units of gravity.
    pub fn to_g(self) -> Vec3 {
        Vec3::new(
            self.x / GRAVITY_MPS2,
            self.y / GRAVITY_MPS2,
            self.z / GRAVITY_MPS2,
        )
    }

    /// The component along the given axis.
    pub fn axis_value(&self, axis: Axis) -> f64 {
        match axis {
            Axis::X => self.x,
            Axis::Y => self.y,
            Axis::Z => self.z,
        }
    }

    /// The axis with the largest magnitude, and that magnitude.
    pub fn dominant_axis(&self) -> (Axis, f64) {
        let mut best = (Axis::X, self.x.abs());
        for axis in [Axis::Y, Axis::Z] {
            let value = self.axis_value(axis).abs();
            if value > best.1 {
                best = (axis, value);
            }
        }
        (best.0, self.axis_value(best.0))
    }

    /// Largest magnitude among the axes orthogonal to the given axis.
    pub fn max_orthogonal(&self, axis: Axis) -> f64 {
        let others = match axis {
            Axis::X => [Axis::Y, Axis::Z],
            Axis::Y => [Axis::Z, Axis::X],
            Axis::Z => [Axis::X, Axis::Y],
        };
        others
            .iter()
            .map(|other| self.axis_value(*other).abs())
            .fold(0.0_f64, f64::max)
    }
}

/// One axis of the canonical frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    /// Unit vector along this axis pointing in the given direction.
    pub fn unit(&self, positive: bool) -> Vec3 {
        let value = if positive { 1.0 } else { -1.0 };
        match self {
            Axis::X => Vec3::new(value, 0.0, 0.0),
            Axis::Y => Vec3::new(0.0, value, 0.0),
            Axis::Z => Vec3::new(0.0, 0.0, value),
        }
    }

    /// The axis as a single letter.
    pub fn as_str(&self) -> &'static str {
        match self {
            Axis::X => "X",
            Axis::Y => "Y",
            Axis::Z => "Z",
        }
    }
}

/// A 3x3 matrix whose rows name the axis each output component is built from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3 {
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub z: [f64; 3],
}

impl Default for Mat3 {
    /// The default matrix is the identity.
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat3 {
    pub const IDENTITY: Mat3 = Mat3 {
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        z: [0.0, 0.0, 1.0],
    };

    /// A matrix from its three rows.
    pub fn from_rows(x: [f64; 3], y: [f64; 3], z: [f64; 3]) -> Self {
        Self { x, y, z }
    }

    /// A matrix with the rows of a configured mount matrix.
    pub fn from_mount_matrix(matrix: &MountMatrix) -> Self {
        Self {
            x: matrix.x,
            y: matrix.y,
            z: matrix.z,
        }
    }

    /// The three rows of the matrix.
    pub fn rows(&self) -> [[f64; 3]; 3] {
        [self.x, self.y, self.z]
    }

    /// Apply the matrix to a vector.
    pub fn apply(&self, value: Vec3) -> Vec3 {
        let dot = |row: &[f64; 3]| row[0] * value.x + row[1] * value.y + row[2] * value.z;
        Vec3::new(dot(&self.x), dot(&self.y), dot(&self.z))
    }

    /// Compose two matrices so that `self.mul(other).apply(v)` applies other first.
    pub fn mul(&self, other: &Mat3) -> Mat3 {
        let left = self.rows();
        let right = other.rows();
        let mut rows = [[0.0; 3]; 3];
        for (a, row) in left.iter().enumerate() {
            for column in 0..3 {
                rows[a][column] = row[0] * right[0][column]
                    + row[1] * right[1][column]
                    + row[2] * right[2][column];
            }
        }
        Mat3::from_rows(rows[0], rows[1], rows[2])
    }

    /// Determinant of the matrix, which is -1 when it mirrors.
    pub fn det(&self) -> f64 {
        let a = &self.x;
        let b = &self.y;
        let c = &self.z;
        a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0])
    }

    /// True when the matrix is an orthonormal rotation rather than a reflection.
    pub fn is_proper_rotation(&self) -> bool {
        let rows = self.rows();
        for a in 0..3 {
            for b in 0..3 {
                let dot: f64 = (0..3).map(|k| rows[a][k] * rows[b][k]).sum();
                let expected = if a == b { 1.0 } else { 0.0 };
                if (dot - expected).abs() > 0.05 {
                    return false;
                }
            }
        }
        (self.det() - 1.0).abs() < 0.05
    }

    /// The matrix as a paste-ready configuration block at the given indent.
    pub fn to_config_block_at(self, indent: usize) -> Vec<String> {
        let pad = " ".repeat(indent);
        let row = |name: &str, values: &[f64; 3]| {
            let cells = values
                .iter()
                .map(|v| format_number(*v))
                .collect::<Vec<String>>()
                .join(", ");
            format!("{pad}  {name}: [{cells}]")
        };

        let mut lines = vec![format!("{pad}mount_matrix:")];
        lines.extend([row("x", &self.x), row("y", &self.y), row("z", &self.z)]);
        lines
    }
}

/// What a step expects to measure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StepKind {
    /// A resting pose where gravity points along `expected`.
    Rest { expected: Vec3 },
    /// A rotation about an axis that must be positive or negative.
    Spin { axis: Axis, positive: bool },
}

/// One pose or rotation to perform, with the wording the user sees.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub id: &'static str,
    pub title: &'static str,
    pub instruction: &'static str,
    pub hint: &'static str,
    pub kind: StepKind,
    /// True when the step reads the gyroscope instead of the accelerometer.
    pub uses_gyro: bool,
}

impl Step {
    /// The direction this step expects, as a unit canonical axis.
    pub fn expected(&self) -> Vec3 {
        match self.kind {
            StepKind::Rest { expected } => expected,
            StepKind::Spin { axis, positive } => axis.unit(positive),
        }
    }

    /// How long this step is measured for.
    pub fn duration(&self) -> Duration {
        if self.uses_gyro {
            return SPIN_DURATION;
        }
        REST_DURATION
    }
}

/// A resting step where gravity reads along `expected`.
fn rest(
    id: &'static str,
    title: &'static str,
    instruction: &'static str,
    hint: &'static str,
    expected: Vec3,
) -> Step {
    Step {
        id,
        title,
        instruction,
        hint,
        kind: StepKind::Rest { expected },
        uses_gyro: false,
    }
}

/// A rotation step about one axis in one direction.
fn spin(
    id: &'static str,
    title: &'static str,
    instruction: &'static str,
    hint: &'static str,
    axis: Axis,
) -> Step {
    Step {
        id,
        title,
        instruction,
        hint,
        kind: StepKind::Spin {
            axis,
            positive: true,
        },
        uses_gyro: true,
    }
}

/// Gain and offset for one axis from a pair of opposite resting poses, which no
/// single pose can separate.
pub fn paired_scale(first: &Attempt, second: &Attempt) -> Option<ScalePair> {
    if first.count == 0 || second.count == 0 {
        return None;
    }
    if first.step.expected().dot(&second.step.expected()) > -0.5 {
        return None;
    }

    let forward = first.mean.norm();
    let back = second.mean.norm();

    Some(ScalePair {
        gain_g: (forward + back) / 2.0 / GRAVITY_MPS2,
        offset_mps2: (forward - back) / 2.0,
        forward_mps2: forward,
        back_mps2: back,
    })
}

/// Gain and offset from opposite resting poses, carried as magnitudes so they hold in any frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalePair {
    /// Mean of the two magnitudes, in units of one gravity
    pub gain_g: f64,
    /// Half the difference of the two magnitudes, in m/s^2
    pub offset_mps2: f64,
    /// Magnitude of the first pose, in m/s^2
    pub forward_mps2: f64,
    /// Magnitude of the second pose, in m/s^2
    pub back_mps2: f64,
}

/// How far a paired gain may sit from one gravity; hand-held poses make this a gross-error check.
pub const SCALE_TOLERANCE_G: f64 = 0.03;

/// Pairs of resting steps pointing opposite ways, the only ones separating gain from offset.
pub fn opposite_pairs(steps: &[Step]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for first in 0..steps.len() {
        for second in first + 1..steps.len() {
            if steps[first].uses_gyro || steps[second].uses_gyro {
                continue;
            }
            if steps[first].expected().dot(&steps[second].expected()) < -0.5 {
                pairs.push((first, second));
            }
        }
    }
    pairs
}

/// The validation sequence in the order it is presented. Directions are written as the holder sees
/// them and name controls rather than a display, so they read the same on a handheld and a gamepad.
pub fn steps() -> Vec<Step> {
    vec![
        rest(
            "r1",
            "Flat, controls to the ceiling",
            "Lie the device flat on a table with the face buttons and joysticks facing the ceiling.",
            "The joysticks point up and gravity reads along +Z.",
            Vec3::new(0.0, 0.0, 1.0),
        ),
        rest(
            "r2",
            "Upright, controls facing you",
            "Hold the device in playing orientation with the face buttons toward you and the top edge up.",
            "The top edge points at the ceiling and gravity reads along +Y.",
            Vec3::new(0.0, 1.0, 0.0),
        ),
        rest(
            "r3",
            "Upright, top edge to your left",
            "Hold the device in playing orientation with the face buttons toward you and the top edge pointing to your LEFT.",
            "The shoulder buttons point to your left and gravity reads along +X.",
            Vec3::new(1.0, 0.0, 0.0),
        ),
        rest(
            "r4",
            "Flat, controls to the floor",
            "Lie the device flat on a table with the face buttons and joysticks facing the floor.",
            "The joysticks point down at the table and gravity reads along -Z.",
            Vec3::new(0.0, 0.0, -1.0),
        ),
        spin(
            "g1",
            "Pitch, nod the control half toward you",
            "Hold the device in playing orientation with the face buttons toward you. Nod the control half so its top edge comes toward your face by about 90 degrees, then hold it still. Do not rock it back and forth.",
            "The control half nods toward you about the axis running between the joysticks. Expected: gyro X positive.",
            Axis::X,
        ),
        spin(
            "g2",
            "Yaw, swing the control half to your right",
            "Hold the device in playing orientation at arm's length with the face buttons toward you. Keep it upright and swing the control half at the wrist so the face buttons point at a spot on the wall to your RIGHT by about 90 degrees, then hold it still. Do not rock it back and forth.",
            "The control face swings about the axis running up through the top edge. Expected: gyro Y positive.",
            Axis::Y,
        ),
        spin(
            "g3",
            "Roll, twist the control face counter-clockwise",
            "Hold the device in playing orientation with the face buttons toward you. Twist the control half counter-clockwise about the axis pointing out of it, as though turning a doorknob, by about 90 degrees, then hold it still. Do not rock it back and forth.",
            "The control face rolls about its own normal, top edge travelling to your left. Expected: gyro Z positive.",
            Axis::Z,
        ),
    ]
}

/// How far the device actually turned, integrated across the window. A mean rate cannot show a
/// scale error because scaling the sensor scales the mean, but an integral matches the turn.
#[derive(Clone, Copy, Debug, Default)]
pub struct RotationMeter {
    last_secs: Option<f64>,
    last: Vec3,
    angle: Vec3,
}

impl RotationMeter {
    /// Add a sample arriving at `secs`, holding its value until the next one. A gap longer than
    /// [ROTATION_MAX_GAP_SECS] contributes nothing, since going quiet means the device stopped.
    pub fn push(&mut self, secs: f64, value: &Vec3) {
        let delta = match self.last_secs {
            Some(last_secs) if secs - last_secs <= ROTATION_MAX_GAP_SECS => secs - last_secs,
            _ => 0.0,
        };
        self.last_secs = Some(secs);

        if delta <= 0.0 {
            self.last = *value;
            return;
        }

        // Trapezoid per axis so a turn the wrong way integrates negative on that axis.
        self.angle = self.angle.add(&Vec3::new(
            (self.last.x + value.x) * 0.5 * delta,
            (self.last.y + value.y) * 0.5 * delta,
            (self.last.z + value.z) * 0.5 * delta,
        ));
        self.last = *value;
    }

    /// The integrated rotation vector in rad: direction is the axis, length is the angle.
    pub fn angle(&self) -> Vec3 {
        self.angle
    }
}

/// Degrees turned, given an integrated rotation vector in rad.
pub fn turned_deg(angle: &Vec3) -> f64 {
    angle.norm().to_degrees()
}

/// Degrees between an integrated rotation vector and the axis asked for; reversed reads 180.
pub fn off_axis_deg(angle: &Vec3, axis: Axis, positive: bool) -> f64 {
    let Some(unit) = angle.normalized() else {
        return 180.0;
    };
    let sign = if positive { 1.0 } else { -1.0 };
    let wanted = match axis {
        Axis::X => Vec3::new(sign, 0.0, 0.0),
        Axis::Y => Vec3::new(0.0, sign, 0.0),
        Axis::Z => Vec3::new(0.0, 0.0, sign),
    };
    let dot = (unit.x * wanted.x + unit.y * wanted.y + unit.z * wanted.z).clamp(-1.0, 1.0);
    dot.acos().to_degrees()
}

/// A completed measurement for one step.
#[derive(Clone, Copy, Debug)]
pub struct Attempt {
    pub step: Step,
    /// Mean of the raw samples, before any candidate is applied.
    pub mean: Vec3,
    /// Integrated rotation over the window in rad, zero when nothing was integrated.
    pub angle: Vec3,
    pub count: u32,
}

/// Whether a step passed, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub pass: bool,
    pub message: String,
}

/// A verdict with its explanation.
fn outcome(pass: bool, message: &str) -> Outcome {
    Outcome {
        pass,
        message: message.to_string(),
    }
}

/// Judge a measured mean against what the step expects.
pub fn evaluate(attempt: &Attempt, transform: &Mat3) -> Outcome {
    if attempt.count == 0 {
        return outcome(false, "no samples, this source may not carry this group");
    }

    match attempt.step.kind {
        StepKind::Rest { expected } => evaluate_rest(&transform.apply(attempt.mean), &expected),
        StepKind::Spin { axis, positive } => {
            // The integrated angle is judged first because it also checks gyro scale; measurements
            // without one, such as older captures, fall through to the rate check.
            let angle = transform.apply(attempt.angle);
            if angle.norm() > 1.0e-3 {
                return evaluate_rotation(&angle, axis, positive);
            }
            evaluate_rate(&transform.apply(attempt.mean), axis, positive)
        }
    }
}

/// Judge a resting pose: one gravity on the expected axis and little beside it. Every problem is
/// reported at once, since a wrong scale usually comes with a wrong axis.
fn evaluate_rest(mean: &Vec3, expected: &Vec3) -> Outcome {
    let gravity = mean.to_g();
    let magnitude = gravity.norm();
    let (axis, value) = gravity.dominant_axis();
    let want = expected_direction(expected);

    let mut problems: Vec<String> = Vec::new();

    if !(0.80..=1.20).contains(&magnitude) {
        let mut scale = format!(
            "gravity reads {magnitude:.2} g, expected 1.00 g (a scale problem: check the sensor's LSB per g constant or config.imu.accel_correction)"
        );
        // Half gravity is what averaging two sensors gives when one reports nothing.
        if (magnitude - 0.5).abs() < 0.05 {
            scale.push_str(", which is exactly half: if this capability is the average of two sensors, one of them is probably reporting zero");
        }
        problems.push(scale);
    }

    if axis != want.0 || (value > 0.0) != want.1 {
        problems.push(format!(
            "dominant axis is {}{} at {:.2} g, expected {}{}",
            if value < 0.0 { "-" } else { "+" },
            axis.as_str(),
            value.abs(),
            sign(want.1),
            want.0.as_str()
        ));
    } else if value.abs() < REST_DOMINANT_G * magnitude {
        // The pose must sit on an axis rather than halfway between two
        problems.push(format!(
            "{} carries only {:.2} g of the {magnitude:.2} g reading, so the device is tilted between axes",
            axis.as_str(),
            value.abs()
        ));
    }

    let off_axis = gravity.max_orthogonal(want.0);
    if off_axis > REST_OFF_AXIS_G {
        problems.push(format!(
            "an orthogonal axis reads {off_axis:.2} g, expected under {REST_OFF_AXIS_G:.2} g"
        ));
    }

    if !problems.is_empty() {
        return outcome(false, &problems.join("; "));
    }

    outcome(
        true,
        &format!(
            "gravity reads {value:+.2} g on {}, magnitude {magnitude:.2} g",
            axis.as_str()
        ),
    )
}

/// Judge a rotation from the mean angular rate, used only when nothing was integrated.
fn evaluate_rate(mean: &Vec3, axis: Axis, positive: bool) -> Outcome {
    let value = mean.axis_value(axis);
    if value.abs() < SPIN_MIN_RAD_S {
        return outcome(
            false,
            &format!(
                "only {value:.2} rad/s on axis {axis}, rotate further",
                axis = axis.as_str()
            ),
        );
    }
    if (value > 0.0) != positive {
        return outcome(
            false,
            &format!(
                "gyro {axis} is {kind}, expected {want}",
                axis = axis.as_str(),
                kind = if value < 0.0 { "negative" } else { "positive" },
                want = if positive { "positive" } else { "negative" }
            ),
        );
    }

    let orthogonal = mean.max_orthogonal(axis);
    let limit = value.abs() * SPIN_ORTHOGONAL_RATIO;
    if orthogonal > limit {
        return outcome(
            false,
            &format!("an orthogonal axis reads {orthogonal:.2} rad/s, over {limit:.2} rad/s"),
        );
    }

    outcome(
        true,
        "rotation is about the expected axis in the expected direction",
    )
}

/// The direction a step expects, as display text such as "+Z".
pub fn expected_label(kind: &StepKind) -> String {
    let (axis, positive) = match *kind {
        StepKind::Rest { expected } => expected_direction(&expected),
        StepKind::Spin { axis, positive } => (axis, positive),
    };
    format!("{}{}", sign(positive), axis.as_str())
}

/// The axis and sign the step's expected vector points along.
fn expected_direction(expected: &Vec3) -> (Axis, bool) {
    let (axis, value) = expected.dominant_axis();
    (axis, value > 0.0)
}

/// A mount matrix as it is written in a device configuration file.
#[derive(Clone, Debug)]
pub struct ConfiguredMatrix {
    pub file: String,
    pub group: String,
    pub iio_name: String,
    /// True when only the deprecated `iio.mount_matrix` key sets a value, so the install is stale.
    pub deprecated_key: bool,
    pub matrix: Mat3,
}

/// The mount matrices configured for the iio sources of the device with the given name.
pub fn find_configured_matrices(name: &str) -> Vec<ConfiguredMatrix> {
    let mut found = Vec::new();
    for (file, config) in matching_configs(name) {
        for source in config.source_devices {
            let Some(iio) = source.iio.as_ref() else {
                continue;
            };
            let configured = source
                .config
                .as_ref()
                .and_then(|config| config.imu.as_ref())
                .and_then(|imu| imu.mount_matrix.as_ref());
            #[allow(deprecated)]
            let legacy = iio.mount_matrix.as_ref();
            let Some(matrix) = configured.or(legacy) else {
                continue;
            };
            found.push(ConfiguredMatrix {
                file: file.clone(),
                group: source.group.clone(),
                iio_name: iio.name.clone().unwrap_or_default(),
                deprecated_key: configured.is_none(),
                matrix: Mat3::from_mount_matrix(matrix),
            });
        }
    }
    found
}

/// Every device configuration file whose name matches the device, in load order: a measurement is
/// meaningless without knowing which file was in force.
pub fn config_files_for(name: &str) -> Vec<String> {
    matching_configs(name)
        .into_iter()
        .map(|(file, _)| file)
        .collect()
}

/// True when one of this device's inertial sources can carry a mount matrix. A hidraw report has
/// nowhere to put one, so its frame belongs in the driver.
pub fn matrix_is_expressible(name: &str) -> bool {
    matching_configs(name).iter().any(|(_, config)| {
        config
            .source_devices
            .iter()
            .any(|source| source.group == "imu" && source.iio.is_some())
    })
}

/// Device configurations that match this device name.
fn matching_configs(name: &str) -> Vec<(String, CompositeDeviceConfig)> {
    let paths = get_devices_paths();
    let files = get_multidir_sorted_files(paths.as_slice(), |entry| {
        entry.path().extension().unwrap_or_default() == "yaml"
    });

    let mut found = Vec::new();
    for file in files {
        let path = file.display().to_string();
        let Ok(config) = CompositeDeviceConfig::from_yaml_file(path.clone()) else {
            continue;
        };
        if config.name == name {
            found.push((path, config));
        }
    }
    found
}

/// Where a device configuration file keeps its mount matrix, and at what indent.
#[derive(Clone, Debug)]
pub struct ConfigTarget {
    pub file: String,
    /// Line holding the key, counting from one, or zero when the file has none
    pub line: usize,
    /// Indent of the key in spaces, or eight when the file has no key to match
    pub indent: usize,
    /// True when the key sits under `iio`, which is the deprecated location
    pub deprecated: bool,
}

/// True when a line carries the mount matrix key, commented or active as asked.
fn is_matrix_key(text: &str, commented: bool) -> bool {
    let trimmed = text.trim();
    let body = trimmed.strip_prefix('#').unwrap_or(trimmed);
    let key = body.split('#').next().unwrap_or("").trim();
    let is_comment = trimmed.starts_with('#');
    key == "mount_matrix:" && is_comment == commented
}

/// Which channel group a matrix belongs to: an accelerometer and a gyroscope can need different
/// ones when a device carries them as two sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelGroup {
    Accelerometer,
    Gyroscope,
}

impl ChannelGroup {
    /// The group as a short column label
    pub fn short(self) -> &'static str {
        match self {
            Self::Accelerometer => "accel",
            Self::Gyroscope => "gyro",
        }
    }

    /// What an iio source carries. Split HID sensor hubs appear as separate `accel_3d` and
    /// `gyro_3d` sources; anything else carries both and needs one matrix for gravity and rate.
    pub fn source_groups(name: &str) -> &'static str {
        let name = name.to_ascii_lowercase();
        if name.contains("accel") {
            return "accelerometer only";
        }
        if name.contains("gyro") {
            return "gyroscope only";
        }
        "accelerometer and gyroscope"
    }

    /// True when an iio source name belongs to this group
    pub fn matches_source(self, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        match self {
            Self::Accelerometer => name.contains("accel"),
            Self::Gyroscope => name.contains("gyro"),
        }
    }
}

/// The iio source a mount matrix belongs to, from the nearest `name:` above it
fn source_name(lines: &[&str], row: usize) -> Option<String> {
    let indent = lines[row].len() - lines[row].trim_start().len();

    lines[..row]
        .iter()
        .rev()
        .map(|text| (text.trim(), text.len() - text.trim_start().len()))
        .find(|(text, width)| text.starts_with("name:") && *width < indent)
        .and_then(|(text, _)| {
            text.split_once(':')
                .map(|(_, value)| value.trim().to_string())
        })
}

/// Find a mount matrix key in a device config for the given channel group. Reports where the key
/// lives and at what indent without modelling the file, preferring the source named for the group.
pub fn find_config_target(file: &str, group: ChannelGroup) -> Option<ConfigTarget> {
    let text = std::fs::read_to_string(file).ok()?;
    let lines: Vec<&str> = text.lines().collect();

    // These files commonly carry a trailing comment on the key, so take what precedes a '#'.
    let found: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, text)| is_matrix_key(text, false))
        .map(|(row, _)| row)
        .collect();

    if found.is_empty() {
        return Some(ConfigTarget {
            file: file.to_string(),
            line: 0,
            indent: 8,
            deprecated: false,
        });
    }

    // Prefer the source that carries this group, falling back to the first key.
    let line = found
        .iter()
        .find(|row| {
            source_name(&lines, **row)
                .as_deref()
                .is_some_and(|name| group.matches_source(name))
        })
        .or_else(|| found.first())
        .copied()
        .unwrap_or(0);

    let indent = lines[line].len() - lines[line].trim_start().len();

    // The key one level above says whether this is `config.imu` or the deprecated `iio`
    let parent = lines[..line]
        .iter()
        .rev()
        .map(|text| (text.trim(), text.len() - text.trim_start().len()))
        .find(|(text, width)| !text.is_empty() && *width < indent)
        .map(|(text, _)| text.split(':').next().unwrap_or("").to_string());

    Some(ConfigTarget {
        file: file.to_string(),
        line: line + 1,
        indent,
        deprecated: parent.as_deref() == Some("iio"),
    })
}

/// Judge a rotation step from the integrated angle, which checks scale as well as axis.
fn evaluate_rotation(angle: &Vec3, axis: Axis, positive: bool) -> Outcome {
    let turned = turned_deg(angle);
    if turned < 1.0 {
        return outcome(
            false,
            "no rotation was integrated during this window, move the device while it measures",
        );
    }

    let mut problems: Vec<String> = Vec::new();
    let low = ROTATION_TARGET_DEG - ROTATION_TOLERANCE_DEG;
    let high = ROTATION_TARGET_DEG + ROTATION_TOLERANCE_DEG;
    if turned < low || turned > high {
        let ratio = turned / ROTATION_TARGET_DEG;
        let mut problem = format!(
            "gyro integrated {turned:.0} deg of rotation, expected about {ROTATION_TARGET_DEG:.0}"
        );
        // A ratio near a round factor is a scale constant, not a badly turned wrist.
        for factor in [0.5, 2.0, 0.25, 4.0] {
            if (ratio - factor).abs() < 0.1 {
                problem.push_str(&format!(
                    ", which is about {factor} times the rotation performed, so the gyro scale is wrong"
                ));
                break;
            }
        }
        problems.push(problem);
    }

    let off = off_axis_deg(angle, axis, positive);
    if off > ROTATION_AXIS_TOL_DEG {
        let along = angle.axis_value(axis).to_degrees();
        problems.push(format!(
            "rotation is {off:.0} deg away from {}{} (it carries {along:+.0} deg), so the device turned about a different axis or the wrong way",
            sign(positive),
            axis.as_str()
        ));
    }

    if !problems.is_empty() {
        return outcome(false, &problems.join("; "));
    }

    let along = angle.axis_value(axis).to_degrees();
    outcome(
        true,
        &format!(
            "integrated {turned:.0} deg, {along:+.0} deg about {}",
            axis.as_str()
        ),
    )
}

/// A candidate correction and how well it explains the measurements.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub transform: Mat3,
    pub score: f64,
    pub proper: bool,
    /// Whether each attempt would pass if the candidate were applied.
    pub predicted: Vec<(&'static str, bool)>,
}

/// All 48 signed permutations of the measured axes.
fn signed_permutations() -> Vec<Mat3> {
    let permutations: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut out = Vec::with_capacity(48);
    for permutation in permutations {
        for signs in 0..8u8 {
            let mut rows = [[0.0; 3]; 3];
            for (row, column) in permutation.iter().enumerate() {
                let sign = if signs & (1 << row) == 0 { 1.0 } else { -1.0 };
                rows[row][*column] = sign;
            }
            out.push(Mat3::from_rows(rows[0], rows[1], rows[2]));
        }
    }
    out
}

/// How well a transform explains samples: summed cosine against each expected direction.
fn score(samples: &[(Vec3, Vec3)], transform: &Mat3) -> f64 {
    samples
        .iter()
        .map(|(measured, expected)| {
            transform
                .apply(*measured)
                .normalized()
                .unwrap_or_default()
                .dot(expected)
        })
        .sum()
}

/// The highest scoring candidate, keeping the first one on a tie.
fn best(scored: &[(Mat3, f64)]) -> Option<(Mat3, f64)> {
    scored
        .iter()
        .max_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    left.0
                        .is_proper_rotation()
                        .cmp(&right.0.is_proper_rotation())
                })
        })
        .copied()
}

/// The matrix to install given what is configured now. Samples come from downstream of the source
/// driver, so a fitted transform is a correction on top of the matrix already configured.
pub fn corrected_matrix(current: Option<Mat3>, transform: &Mat3) -> Mat3 {
    match current {
        Some(current) => transform.mul(&current),
        None => *transform,
    }
}

/// Solve for the transform that maps every measured mean onto the direction its step expects.
pub fn solve(attempts: &[Attempt]) -> Option<Candidate> {
    let samples: Vec<(Vec3, Vec3)> = attempts
        .iter()
        .filter(|attempt| attempt.count > 0)
        .filter_map(|attempt| Some((attempt.mean.normalized()?, attempt.step.expected())))
        .collect();
    if samples.is_empty() {
        return None;
    }

    let scored: Vec<(Mat3, f64)> = signed_permutations()
        .into_iter()
        .map(|candidate| (candidate, score(&samples, &candidate)))
        .collect();

    // Best fit wins as it is: a reflection is legitimate when gyro channels arrive mirror-related
    // to the accelerometer's and no rotation reconciles those. Properness only breaks ties.
    let (transform, score) = best(&scored)?;
    let predicted = attempts
        .iter()
        .map(|attempt| (attempt.step.id, evaluate(attempt, &transform).pass))
        .collect();

    Some(Candidate {
        transform,
        score,
        proper: transform.is_proper_rotation(),
        predicted,
    })
}

/// A drawing held as individually lit dots.
struct Dots {
    width: usize,
    height: usize,
    lit: Vec<bool>,
}

impl Dots {
    /// A blank drawing of the given size.
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            lit: vec![false; width * height],
        }
    }

    /// Light one dot, ignoring anything outside the surface
    fn dot(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        self.lit[y as usize * self.width + x as usize] = true;
    }

    /// Whether a dot of the drawing is lit.
    fn at(&self, x: usize, y: usize) -> bool {
        self.lit[y * self.width + x]
    }

    /// Light a run of dots along a row.
    fn hline(&mut self, from: i32, to: i32, y: i32) {
        for x in from..=to {
            self.dot(x, y);
        }
    }

    /// Light a run of dots along a column.
    fn vline(&mut self, x: i32, from: i32, to: i32) {
        for y in from..=to {
            self.dot(x, y);
        }
    }

    /// Plain rectangular outline, used for the display area of a handheld
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.hline(x0, x1, y0);
        self.hline(x0, x1, y1);
        self.vline(x0, y0, y1);
        self.vline(x1, y0, y1);
    }

    /// Rounded body outline: a lit dot with a neighbour outside, which joins the corners.
    fn round_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, radius: i32) {
        let inside = |x: i32, y: i32| {
            let dx = (x0 + radius - x).max(0) + (x - (x1 - radius)).max(0);
            let dy = (y0 + radius - y).max(0) + (y - (y1 - radius)).max(0);
            x >= x0
                && x <= x1
                && y >= y0
                && y <= y1
                && (dx == 0 || dy == 0 || dx * dx + dy * dy <= radius * radius)
        };

        for y in y0 - 1..=y1 + 1 {
            for x in x0 - 1..=x1 + 1 {
                let edge = inside(x, y)
                    && !(inside(x + 1, y)
                        && inside(x - 1, y)
                        && inside(x, y + 1)
                        && inside(x, y - 1));
                if edge {
                    self.dot(x, y);
                }
            }
        }
    }

    /// Filled circle of dots
    fn disc(&mut self, cx: i32, cy: i32, radius: i32) {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= radius * radius {
                    self.dot(cx + dx, cy + dy);
                }
            }
        }
    }

    /// Thumbstick: a ring of dots with a hub, so it reads as a stick and not a hole
    fn stick(&mut self, cx: i32, cy: i32, radius: i32) {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dot_on_circle(dx, dy, radius) {
                    self.dot(cx + dx, cy + dy);
                }
            }
        }
        self.disc(cx, cy, 1);
    }

    /// Four face buttons as squares at the points of a plus, as they sit on real hardware.
    fn button_block(&mut self, cx: i32, cy: i32) {
        for (dx, dy) in [(0, -5), (0, 5), (-5, 0), (5, 0)] {
            for offset_y in -1..=1 {
                for offset_x in -1..=1 {
                    self.dot(cx + dx + offset_x, cy + dy + offset_y);
                }
            }
        }
    }

    /// Thumbstick as a stem with a bar at the end, reading as a T either way up.
    fn stem(&mut self, cx: i32, from: i32, to: i32, bar: i32) {
        self.vline(cx, from, to);
        self.hline(cx - 3, cx + 3, bar);
    }

    /// D-pad as a cross
    fn cross(&mut self, cx: i32, cy: i32, arm: i32, thick: i32) {
        for dy in -thick..=thick {
            for dx in -arm..=arm {
                self.dot(cx + dx, cy + dy);
            }
        }
        for dx in -thick..=thick {
            for dy in -arm..=arm {
                self.dot(cx + dx, cy + dy);
            }
        }
    }

    /// Turn the drawing so its top edge points left, the pose where gravity reads along +X.
    fn turned_left(&self) -> Dots {
        let mut next = Dots::new(self.height, self.width);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.at(x, y) {
                    next.dot(y as i32, (self.width - 1 - x) as i32);
                }
            }
        }
        next
    }

    /// Fold each pair of rows into one, halving height so a turned view fits a short terminal.
    fn halved(&self) -> Dots {
        let mut next = Dots::new(self.width, self.height.div_ceil(2));
        for y in 0..self.height {
            for x in 0..self.width {
                if self.at(x, y) {
                    next.dot(x as i32, (y / 2) as i32);
                }
            }
        }
        next
    }

    /// Mirror top to bottom, which turns a controls-down drawing into a controls-up one
    fn flipped(&self) -> Dots {
        let mut next = Dots::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                if self.at(x, y) {
                    next.dot(x as i32, (self.height - 1 - y) as i32);
                }
            }
        }
        next
    }

    /// Render as braille rows, two dots across and four down, with trailing blanks trimmed.
    fn lines(&self) -> Vec<String> {
        let columns = self.width.div_ceil(2);
        let rows = self.height.div_ceil(4);
        let mut out: Vec<String> = Vec::new();

        for row in 0..rows {
            let mut cells: Vec<char> = Vec::new();
            for column in 0..columns {
                let mut bits = 0_u16;
                for (bit, dx, dy) in [
                    (0, 0, 0),
                    (1, 0, 1),
                    (2, 0, 2),
                    (3, 1, 0),
                    (4, 1, 1),
                    (5, 1, 2),
                    (6, 0, 3),
                    (7, 1, 3),
                ] {
                    let x = column * 2 + dx;
                    let y = row * 4 + dy;
                    if x < self.width && y < self.height && self.at(x, y) {
                        bits |= 1 << bit;
                    }
                }
                cells.push(char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' '));
            }
            while cells.last() == Some(&'\u{2800}') {
                cells.pop();
            }
            out.push(cells.into_iter().collect());
        }

        while out.last().is_some_and(|line| line.is_empty()) {
            out.pop();
        }
        out
    }
}

/// True when `(dx, dy)` sits on the circle of `radius`
fn dot_on_circle(dx: i32, dy: i32, radius: i32) -> bool {
    let squared = dx * dx + dy * dy;
    squared <= radius * radius && squared >= (radius - 1) * (radius - 1)
}

/// Two-row arrow with a tail: a shaft above a `v` head for motion toward the viewer.
fn arrow(rows: &mut [String], first_row: usize, column: usize, toward: bool) {
    if toward {
        stamp(rows, first_row, column, "|");
        stamp(rows, first_row + 1, column, "v");
    } else {
        stamp(rows, first_row, column, "^");
        stamp(rows, first_row + 1, column, "|");
    }
}

/// Write ASCII text over rendered rows; dots cannot express an arrow head legibly.
fn stamp(rows: &mut [String], row: usize, column: usize, text: &str) {
    let Some(line) = rows.get_mut(row) else {
        return;
    };
    let mut cells: Vec<char> = line.chars().collect();
    if cells.len() < column {
        cells.resize(column, '\u{2800}');
    }
    for (offset, character) in text.chars().enumerate() {
        let index = column + offset;
        if index >= cells.len() {
            cells.push(' ');
        }
        cells[index] = character;
    }
    *line = cells.into_iter().collect();
}

/// The controls side face-on: the display area in the middle with thumbsticks, d-pad and face
/// buttons in the columns beside it, and two shoulder tabs plus a centre dot on the top edge.
fn face_on(width: usize, height: usize) -> Dots {
    let mut dots = Dots::new(width, height);
    let (left, right) = (SHELL_LEFT, SHELL_RIGHT);
    let (top, bottom) = (6, height as i32 - 7);

    dots.round_rect(left, top, right, bottom, 7);

    // Shoulder tabs thicken the top edge instead of sticking out from it
    for x in [left + 16, left + 17, right - 17, right - 18] {
        dots.dot(x, top + 1);
    }
    dots.dot((left + right) / 2, top + 2);

    // Display area
    dots.rect(left + 27, top + 4, right - 27, bottom - 4);

    // Left column thumbstick above the d-pad; right column buttons above the thumbstick.
    dots.stick(LEFT_STICK, top + 11, 4);
    dots.cross(left + 14, bottom - 8, 5, 2);
    dots.button_block(right - 15, top + 13);
    dots.stick(RIGHT_STICK, bottom - 7, 4);

    dots
}

const FACE_WIDTH: usize = 100;
const FACE_HEIGHT: usize = 44;

/// Shell extents and thumbstick columns, shared so every view has the same width and alignment.
const SHELL_LEFT: i32 = 6;
const SHELL_RIGHT: i32 = FACE_WIDTH as i32 - 7;
const LEFT_STICK: i32 = SHELL_LEFT + 13;
const RIGHT_STICK: i32 = SHELL_RIGHT - 20;
/// Rows of results that must stay visible whatever the pose costs.
const MIN_RESULTS_ROWS: u16 = 12;

/// The pose drawing for a step, fitted into `max_rows`, and a note on how it is viewed. Three
/// drawings cover all seven steps, folded until they fit a short terminal.
fn pose(step: &Step, max_rows: usize) -> (Vec<String>, &'static str) {
    // Rotated, the face is as tall as it was wide, so it needs folding the most
    let turned = || fitted(face_on(FACE_WIDTH, FACE_HEIGHT).turned_left(), max_rows);

    match step.id {
        "r1" => (
            slab(true, None, max_rows),
            "seen from the side, controls facing the ceiling",
        ),
        "r2" => (
            face_view(None, max_rows),
            "seen from the front, controls facing you",
        ),
        "r3" => (turned(), "seen from the front, top edge to your left"),
        "r4" => (
            slab(false, None, max_rows),
            "seen from the side, controls facing the floor",
        ),
        "g1" => (
            slab(false, Some((Axis::X, true)), max_rows),
            "seen from the side, controls facing the floor; both ends travel toward you",
        ),
        "g2" => (
            slab(false, Some((Axis::Y, true)), max_rows),
            "seen from the side, controls facing the floor; the left end travels toward \
             you and the right end away",
        ),
        "g3" => (
            face_view(Some(true), max_rows),
            "seen from the front, top edge travels to your left",
        ),
        _ => (face_view(None, max_rows), "seen from the front"),
    }
}

/// Render a drawing, folding it in half until its rows fit the space available
fn fitted(dots: Dots, max_rows: usize) -> Vec<String> {
    let mut dots = dots;
    while dots.lines().len() > max_rows && dots.height > 12 {
        dots = dots.halved();
    }
    dots.lines()
}

/// The device as a slab: rounded shell, controls face heavier than the back, thumbsticks outside.
fn slab(controls_up: bool, arrows: Option<(Axis, bool)>, max_rows: usize) -> Vec<String> {
    let mut dots = Dots::new(FACE_WIDTH, 15);
    dots.round_rect(SHELL_LEFT, 1, SHELL_RIGHT, 9, 4);
    dots.hline(SHELL_LEFT + 2, SHELL_RIGHT - 2, 8);
    for cx in [LEFT_STICK, RIGHT_STICK] {
        dots.stem(cx, 9, 12, 13);
    }

    if controls_up {
        dots = dots.flipped();
    }
    let mut rows = fitted(dots, max_rows);

    // Arrows go in the empty margins so they read as motion. Shell extents are dots and braille
    // packs two dots per cell, so a column is half a shell position.
    if let Some((axis, positive)) = arrows {
        let left = 0;
        let right = FACE_WIDTH / 2 - 1;
        match axis {
            // Pitch carries both ends the same way
            Axis::X => {
                arrow(&mut rows, 0, left, positive);
                arrow(&mut rows, 0, right, positive);
            }
            // Positive yaw brings one end toward the viewer and takes the other away
            Axis::Y => {
                arrow(&mut rows, 0, left, positive);
                arrow(&mut rows, 0, right, !positive);
            }
            Axis::Z => {}
        }
    }
    rows
}

/// The controls face from the front, with roll arrows beside the shell at mid height.
fn face_view(roll: Option<bool>, max_rows: usize) -> Vec<String> {
    let mut rows = fitted(face_on(FACE_WIDTH, FACE_HEIGHT), max_rows);

    if let Some(positive) = roll {
        let middle = rows.len() / 2;
        arrow(&mut rows, middle - 1, SHELL_LEFT as usize / 2 - 2, positive);
        arrow(
            &mut rows,
            middle - 1,
            SHELL_RIGHT as usize / 2 + 2,
            !positive,
        );
    }
    rows
}

/// Keys the menu responds to
const KEYS: &str = " s measure  r retry  n next  p previous  d re-derive  q quit ";

/// `+` or `-` for a direction that is or is not positive.
fn sign(positive: bool) -> &'static str {
    if positive {
        "+"
    } else {
        "-"
    }
}

/// A short verdict label
fn verdict(pass: bool) -> &'static str {
    if pass {
        "PASS"
    } else {
        "FAIL"
    }
}

/// One configuration change the measurements call for; groups wanting one matrix share an entry.
struct Suggestion {
    name: String,
    groups: String,
    target: ConfigTarget,
    matrix: Mat3,
    passing: usize,
    total: usize,
    score: f64,
    proper: bool,
    serves: &'static str,
}

/// The matrix a channel group's measurements arrive with, and where it came from
struct Baseline {
    matrix: Option<Mat3>,
    text: String,
}

/// An accelerometer and gyroscope that belong to the same sensor
#[derive(Clone, Debug, PartialEq)]
struct ImuSource {
    name: String,
    accel: Option<InputCapability>,
    gyro: Option<InputCapability>,
}

/// The wizard's state: the steps, every sensor's readings, and what they imply.
#[derive(Debug)]
pub struct ImuTestMenu {
    conn: Connection,
    device_path: String,
    device_name: String,
    saved: SavedState,
    rx_disconnect: mpsc::Receiver<()>,
    rx_reports: mpsc::Receiver<Vec<u8>>,
    rx_capabilities: mpsc::Receiver<Vec<u8>>,
    capability_report: Option<InputCapabilityReport>,
    steps: Vec<Step>,
    imus: Vec<ImuSource>,
    cursor: usize,
    /// Measured means, keyed by sensor and step
    attempts: HashMap<(usize, usize), Attempt>,
    /// Per-sensor sums and counts for the step being measured
    sums: Vec<Vec3>,
    counts: Vec<u32>,
    /// Per-sensor sums of samples that carried motion, so standing still does not dilute a rotation
    active_sums: Vec<Vec3>,
    active_counts: Vec<u32>,
    /// Per-sensor gyro integration, the only scale check a rotation step has
    meters: Vec<RotationMeter>,
    live: Vec<Option<Vec3>>,
    sampling: bool,
    started: Option<Instant>,
    elapsed: Duration,
    /// Derived matrix for each sensor and channel group that has enough measurements
    candidates: HashMap<usize, Candidate>,
    gyro_candidates: HashMap<usize, Candidate>,
    configured: Vec<ConfiguredMatrix>,
    config_files: Vec<String>,
    /// Set when the device turns out to have nothing to validate
    fatal: bool,
    status: String,
}

impl ImuTestMenu {
    /// Connect to the device and attach the debug target for raw readings.
    pub async fn new(conn: &Connection, dbus_path: &str) -> Result<Self, Box<dyn Error>> {
        let device = CompositeDeviceInterfaceProxy::builder(conn)
            .path(dbus_path.to_string())?
            .build()
            .await?;
        let device_name = device.name().await.unwrap_or_default();

        // Refuse before changing anything; an unreadable capability list is left to the report
        match device.capabilities().await {
            Ok(capabilities) => {
                let has_inertial = capabilities.iter().any(|capability| {
                    capability.starts_with("Accelerometer") || capability.starts_with("Gyroscope")
                });
                if !has_inertial {
                    return Err(format!(
                        "device '{device_name}' implements no accelerometer or gyroscope capabilities, so there is nothing to validate"
                    )
                    .into());
                }
            }
            Err(e) => {
                log::warn!("Could not list the capabilities of '{device_name}': {e}");
            }
        }

        // Attach the debug target so raw capabilities stay visible, not remapped or translated
        let (saved, stream) = debug_target::attach(conn, dbus_path).await?;
        let configured = find_configured_matrices(&device_name);
        let config_files = config_files_for(&device_name);

        Ok(Self {
            conn: conn.clone(),
            device_path: dbus_path.to_string(),
            device_name,
            saved,
            rx_disconnect: stream.rx_disconnect,
            rx_reports: stream.rx_reports,
            rx_capabilities: stream.rx_capabilities,
            capability_report: None,
            steps: steps(),
            imus: Vec::new(),
            cursor: 0,
            attempts: HashMap::new(),
            sums: Vec::new(),
            counts: Vec::new(),
            active_sums: Vec::new(),
            active_counts: Vec::new(),
            meters: Vec::new(),
            live: Vec::new(),
            sampling: false,
            started: None,
            elapsed: Duration::ZERO,
            candidates: HashMap::new(),
            gyro_candidates: HashMap::new(),
            configured,
            config_files,
            fatal: false,
            status: "Waiting for the capability report from the device...".to_string(),
        })
    }

    /// Restore the device state that attaching the debug target changed
    fn stop(&self) {
        debug_target::restore(&self.conn, &self.device_path, &self.saved);
    }

    /// True when `n` has moved past the last step. That region then lists every step's verdict, so
    /// a run is read from one screen with no separate results page.
    fn at_summary(&self) -> bool {
        self.cursor >= self.steps.len()
    }

    /// The step the cursor is on.
    fn current_step(&self) -> Option<&Step> {
        self.steps.get(self.cursor)
    }

    /// The sensor that owns a capability.
    fn imu_for_capability(&self, capability: InputCapability) -> Option<usize> {
        self.imus
            .iter()
            .position(|source| source.accel == Some(capability) || source.gyro == Some(capability))
    }

    /// The capability of the given sensor that the current step measures
    fn capability_for(&self, imu: usize, uses_gyro: bool) -> Option<InputCapability> {
        let source = self.imus.get(imu)?;
        if uses_gyro {
            return source.gyro;
        }
        source.accel
    }

    /// Rebuild the list of sensors when the capability report changes
    fn rebuild_sensors(&mut self) {
        let Some(report) = self.capability_report.as_ref() else {
            return;
        };

        let sensors = imu_sources(report);

        // A report can arrive mid-run, so measurements drop only when the sensors really changed.
        if sensors != self.imus {
            self.imus = sensors;
            self.sums = vec![Vec3::default(); self.imus.len()];
            self.counts = vec![0; self.imus.len()];
            self.active_sums = vec![Vec3::default(); self.imus.len()];
            self.active_counts = vec![0; self.imus.len()];
            self.meters = vec![RotationMeter::default(); self.imus.len()];
            self.live = vec![None; self.imus.len()];
            self.attempts.clear();
            self.candidates.clear();
            self.gyro_candidates.clear();
            self.status = "Sensors changed, measurements cleared.".to_string();
        }

        // At least one step must have a sensor that can measure it
        let measurable = self.steps.iter().any(|step| {
            (0..self.imus.len()).any(|imu| self.capability_for(imu, step.uses_gyro).is_some())
        });
        if self.imus.is_empty() || !measurable {
            self.fatal = true;
            self.status = "No inertial capabilities are measurable on this device. Inertial \
                events may be remapped by the device configuration."
                .to_string();
            return;
        }

        self.select(0);
    }

    /// Show the given step and get it ready to measure
    fn select(&mut self, cursor: usize) {
        self.cursor = cursor;
        self.sampling = false;
        self.started = None;
        self.elapsed = Duration::ZERO;

        if self.at_summary() {
            // Fitting happens on arrival so the matrix is on the summary, not behind a key press.
            self.fit_all();

            // Named rather than described, so nothing is guessed about where `p` lands.
            let last = self
                .steps
                .last()
                .map_or(String::new(), |step| format!("{} {}", step.id, step.title));
            self.status = format!("Run complete. Press p back to {last}.");
            return;
        }

        let Some(step) = self.current_step() else {
            return;
        };
        self.status = format!("{}: {}. Press s to measure.", step.title, step.instruction);
    }

    /// Move the cursor, staying inside the steps.
    fn move_cursor(&mut self, delta: isize) {
        let target = self.cursor as isize + delta;
        if !(0..=(self.steps.len() as isize)).contains(&target) {
            return;
        }
        self.select(target as usize);
    }

    /// Start measuring the current step on every sensor that can measure it
    fn start(&mut self) {
        let Some(step) = self.current_step().copied() else {
            return;
        };

        for imu in 0..self.imus.len() {
            self.attempts.remove(&(imu, self.cursor));
            self.sums[imu] = Vec3::default();
            self.counts[imu] = 0;
            self.active_sums[imu] = Vec3::default();
            self.active_counts[imu] = 0;
            self.meters[imu] = RotationMeter::default();
        }

        self.elapsed = Duration::ZERO;
        self.started = Some(Instant::now());
        self.sampling = true;
        self.status = format!(
            "Measuring {}: {}",
            step.title,
            if step.uses_gyro {
                "turn about 90 degrees, then hold still"
            } else {
                "hold the device still"
            }
        );
    }

    /// Add one round of samples for every sensor the step can measure, advancing together.
    fn feed(&mut self, samples: &[(usize, Vec3)]) {
        if !self.sampling || samples.is_empty() {
            return;
        }

        // One motion gate for the whole round keeps the sensors' sample counts comparable
        let Some(step) = self.current_step().copied() else {
            return;
        };
        let moving = !step.uses_gyro
            || samples
                .iter()
                .any(|(_, value)| value.norm() >= SPIN_SAMPLE_FLOOR_RAD_S);

        // Arrival time stands in for the sample's own time: a delta filtered source goes quiet when
        // held still, and integration must know when samples stopped rather than tile the window.
        let secs = self
            .started
            .map(|started| started.elapsed().as_secs_f64())
            .unwrap_or(0.0);

        for (imu, sample) in samples {
            if step.uses_gyro {
                self.meters[*imu].push(secs, sample);
            }
            self.sums[*imu] = self.sums[*imu].add(sample);
            self.counts[*imu] += 1;

            if moving {
                self.active_sums[*imu] = self.active_sums[*imu].add(sample);
                self.active_counts[*imu] += 1;
            }
        }

        self.tick();
    }

    /// Close the window when its time is up, even if reporting stopped partway through.
    fn tick(&mut self) {
        if !self.sampling {
            return;
        }
        let Some(step) = self.current_step().copied() else {
            return;
        };
        let Some(started) = self.started else {
            return;
        };

        self.elapsed = started.elapsed();
        if self.elapsed < step.duration() {
            return;
        }
        if self.counts.iter().all(|count| *count == 0) {
            return;
        }

        self.finish_step();
    }

    /// Turn the accumulated samples into a verdict for every sensor
    fn finish_step(&mut self) {
        self.sampling = false;
        self.started = None;

        let Some(step) = self.current_step().copied() else {
            return;
        };

        let mut passing = 0;
        let mut measured = 0;
        for imu in 0..self.imus.len() {
            let (sum, count) = if step.uses_gyro && self.active_counts[imu] > 0 {
                (self.active_sums[imu], self.active_counts[imu])
            } else {
                (self.sums[imu], self.counts[imu])
            };
            if count == 0 {
                continue;
            }
            measured += 1;

            let attempt = Attempt {
                step,
                mean: sum.scale(1.0 / count as f64),
                angle: self.meters[imu].angle(),
                count,
            };
            if evaluate(&attempt, &Mat3::IDENTITY).pass {
                passing += 1;
            }
            self.attempts.insert((imu, self.cursor), attempt);
        }

        if measured == 0 {
            self.status = format!(
                "{}: no samples arrived. Press r to retry, or n to continue.",
                step.title
            );
            return;
        }

        self.status = format!(
            "{} of {} sensors passed {}. Press r to retry, or n to continue.",
            passing, measured, step.title
        );
    }

    /// Every derived matrix, per sensor and channel group, fitted separately per group.
    fn fits(&self) -> Vec<(usize, ChannelGroup, Candidate)> {
        let mut fits: Vec<(usize, ChannelGroup, Candidate)> = self
            .candidates
            .iter()
            .map(|(imu, candidate)| (*imu, ChannelGroup::Accelerometer, candidate.clone()))
            .collect();
        for (imu, candidate) in self.gyro_candidates.iter() {
            fits.push((*imu, ChannelGroup::Gyroscope, candidate.clone()));
        }
        fits.sort_by_key(|(imu, group, _)| (self.imus[*imu].name.clone(), group.short()));
        fits
    }

    /// The measurements of one channel group, since the two groups are fitted apart
    fn attempts_for_group(&self, imu: usize, group: ChannelGroup) -> Vec<Attempt> {
        let uses_gyro = group == ChannelGroup::Gyroscope;
        let mut attempts: Vec<Attempt> = self
            .attempts
            .iter()
            .filter(|((found, _), _)| *found == imu)
            .map(|((_, _), attempt)| *attempt)
            .filter(|attempt| attempt.step.uses_gyro == uses_gyro)
            .collect();
        attempts.sort_by_key(|attempt| attempt.step.id);
        attempts
    }

    /// `d` fits on demand and reports what it found
    fn derive(&mut self) {
        let summary = self.fit_all();
        if summary.is_empty() {
            self.status = "Nothing to derive yet. Measure more steps first.".to_string();
            return;
        }
        self.status = format!("Derived a matrix for {}.", summary.join(", "));
    }

    /// Fit every sensor and group from the steps measured so far, one description per matrix.
    fn fit_all(&mut self) -> Vec<String> {
        let mut summary: Vec<String> = Vec::new();

        for imu in 0..self.imus.len() {
            for group in [ChannelGroup::Accelerometer, ChannelGroup::Gyroscope] {
                let attempts = self.attempts_for_group(imu, group);
                let Some(candidate) = solve(&attempts) else {
                    continue;
                };
                let passing = candidate.predicted.iter().filter(|(_, pass)| *pass).count();
                summary.push(format!(
                    "{} {} {passing}/{}",
                    self.imus[imu].name,
                    group.short(),
                    candidate.predicted.len()
                ));

                let table = match group {
                    ChannelGroup::Accelerometer => &mut self.candidates,
                    ChannelGroup::Gyroscope => &mut self.gyro_candidates,
                };
                table.insert(imu, candidate);
            }
        }

        summary
    }

    /// The matrix a group's measurements arrive with, from the source carrying that group.
    fn baseline_for(&self, group: ChannelGroup) -> Baseline {
        let matching = self
            .configured
            .iter()
            .find(|found| {
                group.matches_source(&found.iio_name) || group.matches_source(&found.group)
            })
            .or_else(|| {
                // One configured matrix serves both groups of a combined source
                if self.configured.len() == 1 {
                    self.configured.first()
                } else {
                    None
                }
            });

        let Some(found) = matching else {
            return Baseline {
                matrix: None,
                text: "nothing configured".to_string(),
            };
        };

        let key = if found.deprecated_key {
            "deprecated iio.mount_matrix"
        } else {
            "config.imu.mount_matrix"
        };
        Baseline {
            matrix: Some(found.matrix),
            text: format!("{} in {}", key, found.file),
        }
    }

    /// What carries this group's matrix: one group cannot be judged for handedness, both can.
    fn serves_text(&self, group: ChannelGroup) -> &'static str {
        let names: Vec<String> = self
            .configured
            .iter()
            .map(|configured| configured.iio_name.clone())
            .collect();
        let name = names
            .iter()
            .find(|name| group.matches_source(name))
            .or_else(|| names.first())
            .map_or("imu", |name| name.as_str());
        ChannelGroup::source_groups(name)
    }

    /// Where a derived matrix for this group belongs in the device configuration
    fn config_target(&self, group: ChannelGroup) -> Option<ConfigTarget> {
        let file = self.config_files.first()?;
        find_config_target(file, group)
    }

    /// Verdict of each step in `range`; `header` names the device, which the summary leaves out.
    fn results_lines(&self, range: &[usize], header: bool) -> Vec<String> {
        if self.imus.is_empty() {
            return vec!["This device reports no accelerometers or gyroscopes.".to_string()];
        }

        let mut lines = Vec::new();
        if header {
            lines.push(format!(
                "device {} sensors {}",
                self.device_name,
                self.sensor_summary()
            ));
        }

        for index in range.iter().copied() {
            let step = &self.steps[index];
            let head = format!("{:>2} {} {}", index + 1, step.id, step.title);
            let mut results: Vec<String> = Vec::new();

            for imu in 0..self.imus.len() {
                let Some(attempt) = self.attempts.get(&(imu, index)) else {
                    continue;
                };
                let outcome = evaluate(attempt, &Mat3::IDENTITY);
                if outcome.pass {
                    results.push(format!("{} {}", self.imus[imu].name, verdict(true)));
                } else {
                    results.push(format!(
                        "{} {} {}",
                        self.imus[imu].name,
                        verdict(false),
                        outcome.message
                    ));
                }
            }

            if results.is_empty() {
                lines.push(format!("{head:<34} -"));
                continue;
            }
            lines.push(format!("{head:<34} {}", results.join(", ")));
        }

        if let Some(line) = self.gain_line() {
            lines.push(line);
        }
        lines
    }

    /// Gain from opposite resting poses, the accelerometer's only scale check.
    fn gain_line(&self) -> Option<String> {
        for (first, second) in opposite_pairs(&self.steps) {
            let Some(one) = self.attempts.get(&(0, first)) else {
                continue;
            };
            let Some(other) = self.attempts.get(&(0, second)) else {
                continue;
            };
            let Some(pair) = paired_scale(one, other) else {
                continue;
            };
            let pass = (pair.gain_g - 1.0).abs() <= SCALE_TOLERANCE_G;
            return Some(format!(
                "accel gain {:.3} g {} from {} + {}",
                pair.gain_g,
                verdict(pass),
                one.step.id,
                other.step.id
            ));
        }
        None
    }

    /// What to do about what was measured
    fn recommendation_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        if !matrix_is_expressible(&self.device_name) {
            lines.push("Inertial data for this device arrives in a hidraw report, which".into());
            lines.push(
                "carries no mount matrix. A wrong frame must be corrected in the driver.".into(),
            );
            return lines;
        }

        let fits = self.fits();
        if fits.is_empty() {
            lines.push(if self.at_summary() {
                "Not enough steps measured to derive a matrix.".into()
            } else {
                "Measure all seven steps; the matrix appears on the summary.".into()
            });
            return lines;
        }

        // One entry per distinct matrix; two groups wanting the same one read as one change.
        let mut suggestions: Vec<Suggestion> = Vec::new();

        for (imu, group, candidate) in fits {
            let name = &self.imus[imu].name;
            let baseline = self.baseline_for(group);
            let new = corrected_matrix(baseline.matrix, &candidate.transform);
            let passing = candidate.predicted.iter().filter(|(_, pass)| *pass).count();
            let total = candidate.predicted.len();

            if baseline.matrix == Some(new) {
                lines.push(format!(
                    "{name} {short}: configured matrix is correct, {passing}/{total} pass ({})",
                    baseline.text,
                    short = group.short()
                ));
                continue;
            }

            let Some(target) = self.config_target(group) else {
                lines.push(format!(
                    "{name} {}: no device config matched this device",
                    group.short()
                ));
                continue;
            };

            // Merged entries keep the weaker figures so they never overstate a fit.
            let found = suggestions.iter_mut().find(|entry| {
                entry.matrix == new && entry.target.file == target.file && entry.name == *name
            });
            if let Some(entry) = found {
                entry.groups.push('+');
                entry.groups.push_str(group.short());
                entry.passing = entry.passing.min(passing);
                entry.total = entry.total.min(total);
                entry.score = entry.score.min(candidate.score);
                entry.proper &= candidate.proper;
                continue;
            }

            suggestions.push(Suggestion {
                name: name.clone(),
                groups: group.short().to_string(),
                target,
                matrix: new,
                passing,
                total,
                score: candidate.score,
                proper: candidate.proper,
                serves: self.serves_text(group),
            });
        }

        for entry in suggestions {
            lines.push(format!(
                "{} {}: replace {} around line {} of {} ({} of {} steps fit, score {:.2})",
                entry.name,
                entry.groups,
                if entry.target.deprecated {
                    "iio.mount_matrix"
                } else {
                    "config.imu.mount_matrix"
                },
                entry.target.line,
                entry.target.file,
                entry.passing,
                entry.total,
                entry.score
            ));
            // Dedented by the file's own indent so the block keeps its shape.
            for line in entry.matrix.to_config_block_at(entry.target.indent) {
                let leading = line.len() - line.trim_start().len();
                lines.push(line[leading.min(entry.target.indent)..].to_string());
            }
            if !entry.proper {
                lines.push(format!(
                    "  a mirror, and this source carries {}",
                    entry.serves
                ));
            }
        }

        lines
    }

    /// Every sensor with the kinds of measurement it offers.
    fn sensor_summary(&self) -> String {
        self.imus
            .iter()
            .map(|imu| {
                let mut kinds = Vec::new();
                if imu.accel.is_some() {
                    kinds.push("accel");
                }
                if imu.gyro.is_some() {
                    kinds.push("gyro");
                }
                format!("{}[{}]", imu.name, kinds.join("+"))
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The live reading of each sensor for the step being measured
    fn live_lines(&self) -> Vec<String> {
        let Some(step) = self.current_step() else {
            return Vec::new();
        };
        let expected = expected_label(&step.kind);

        let mut lines = vec![format!(
            "step {} of {}: expect {expected}",
            self.cursor + 1,
            self.steps.len()
        )];
        // The hint names the physical axis and sign, which the drawing cannot show.
        if step.uses_gyro {
            lines.push(step.hint.to_string());
        }

        for imu in 0..self.imus.len() {
            if self.capability_for(imu, step.uses_gyro).is_none() {
                continue;
            }
            let Some(value) = self.live[imu] else {
                lines.push(format!("{:<8} waiting for samples", self.imus[imu].name));
                continue;
            };
            let (axis, magnitude) = if step.uses_gyro {
                let angle = self.meters[imu].angle();
                (
                    angle.dominant_axis().0,
                    format!("{:.0} deg turned", turned_deg(&angle)),
                )
            } else {
                let in_g = value.to_g();
                (in_g.dominant_axis().0, format!("{:.3} g", in_g.norm()))
            };
            lines.push(format!(
                "{:<8} arrived {} {}",
                self.imus[imu].name,
                axis.as_str(),
                magnitude
            ));
        }
        lines
    }

    /// The drawing for the step being performed, folded to the rows its region has.
    fn pose_lines(&self, area: Rect) -> Vec<String> {
        let Some(step) = self.current_step() else {
            return Vec::new();
        };

        // Two rows go to the border and one to the viewing note
        let budget = (area.height as usize).saturating_sub(3).max(4);
        let (art, note) = pose(step, budget);
        let widest = art
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        let usable = area.width.saturating_sub(2) as usize;

        if widest > usable {
            // Clipped rather than wrapped, so a narrow terminal loses nothing repeated below.
            return vec![
                step.title.to_string(),
                step.instruction.to_string(),
                String::new(),
                format!("[drawing needs a terminal {widest} columns wide]"),
            ];
        }

        let mut lines = art;
        lines.push(String::new());
        lines.push(format!("[{note}]"));
        lines
    }

    /// Draw a titled panel of text lines.
    fn render_panel(
        &self,
        title: &str,
        color: Color,
        area: Rect,
        buf: &mut Buffer,
        lines: &[String],
    ) {
        let block = Block::bordered()
            .title(format!(" {title} "))
            .border_set(border::ROUNDED)
            .border_style(Style::new().fg(color));
        let inner = block.inner(area);
        block.render(area, buf);
        Paragraph::new(lines.join("\n")).render(inner, buf);
    }

    /// Restore the device and leave the interface.
    fn quit(&mut self) -> Vec<InterfaceCommand> {
        self.stop();
        vec![InterfaceCommand::Quit]
    }

    /// Decode every sensor a step can measure into canonical units as (sensor index, sample).
    fn collect_samples(&self, state: &InputDataReport, step: &Step) -> Vec<(usize, Vec3)> {
        let scale = if step.uses_gyro {
            UNIFIED_GAMEPAD_GYRO_RAW_TO_RAD_S
        } else {
            UNIFIED_GAMEPAD_ACCEL_RAW_TO_MPS2
        };
        let Some(capability_report) = self.capability_report.as_ref() else {
            return Vec::new();
        };
        let Ok(values) = capability_report.decode_data_report(state) else {
            return Vec::new();
        };

        let mut samples: Vec<(usize, Vec3)> = Vec::new();
        for (entry, value) in capability_report
            .get_capabilities()
            .iter()
            .zip(values.iter())
        {
            let Some(imu) = self.imu_for_capability(entry.capability) else {
                continue;
            };
            if self.capability_for(imu, step.uses_gyro) != Some(entry.capability) {
                continue;
            }
            let Value::Int16Vector3(value) = value else {
                continue;
            };
            samples.push((
                imu,
                Vec3::new(
                    value.x as f64 * scale,
                    value.y as f64 * scale,
                    value.z as f64 * scale,
                ),
            ));
        }
        samples
    }
}

impl Widget for &ImuTestMenu {
    /// One screen, re-rendered on every update. Nothing scrolls.
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Min(10),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(area);

        // Pose above results at every width, so the drawing always has the full terminal and a wide
        // screen gains rows instead of a different shape of layout.
        let pose_rows = layout[0].height.saturating_sub(MIN_RESULTS_ROWS).max(6);
        let regions = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(pose_rows), Constraint::Min(4)])
            .split(layout[0]);

        // The top region holds the pose while steps remain and the verdicts once `n` passes
        // the last one, so a run is read from one screen that never scrolls.
        let summary = self.at_summary();
        let top = if summary {
            let all: Vec<usize> = (0..self.steps.len()).collect();
            self.results_lines(&all, false)
        } else {
            self.pose_lines(regions[0])
        };
        let title = if summary { "summary" } else { "pose" };
        let color = if summary { Color::Green } else { Color::Cyan };
        self.render_panel(title, color, regions[0], buf, &top);

        // The recommendation leads, so a short terminal loses readings rather than the answer.
        let mut right = self.recommendation_lines();
        if !summary {
            right.push(String::new());
            right.extend(self.live_lines());
            right.extend(self.results_lines(&[self.cursor], true));
        }
        self.render_panel("results", Color::White, regions[1], buf, &right);

        let status = Paragraph::new(Line::from(Span::styled(
            self.status.clone(),
            Style::new().fg(
                if self.status.contains("FAIL") || self.status.contains("failed") {
                    Color::Red
                } else if self.status.contains("passed") {
                    Color::Green
                } else {
                    Color::White
                },
            ),
        )));
        status.render(layout[1], buf);
        Paragraph::new(Line::from(Span::styled(KEYS, Style::new().dark_gray())))
            .render(layout[2], buf);
    }
}

impl MenuWidget for ImuTestMenu {
    /// Poll the device and advance the measurement window.
    fn update(&mut self) -> Vec<InterfaceCommand> {
        if self.fatal {
            return self.quit();
        }
        if self.rx_capabilities.is_closed()
            || self.rx_reports.is_closed()
            || self.rx_disconnect.is_closed()
        {
            return self.quit();
        }
        if !self.rx_disconnect.is_empty() {
            return self.quit();
        }

        // The window runs on wall-clock time, so it is checked even when nothing is reported.
        self.tick();

        let mut capabilities_updated = false;
        while !self.rx_capabilities.is_empty() {
            let Some(data) = self.rx_capabilities.blocking_recv() else {
                return self.quit();
            };
            self.capability_report = InputCapabilityReport::unpack(data.as_slice()).ok();
            capabilities_updated = true;
        }
        if capabilities_updated {
            self.rebuild_sensors();
        }

        // Only the newest input report matters
        let mut state_bytes = None;
        while !self.rx_reports.is_empty() {
            let Some(data) = self.rx_reports.blocking_recv() else {
                return self.quit();
            };
            state_bytes = Some(data);
        }
        let Some(state_bytes) = state_bytes else {
            return vec![];
        };
        let Ok(state_slice) = state_bytes.as_slice().try_into() else {
            return vec![];
        };
        let Ok(state) = InputDataReport::unpack(state_slice) else {
            return vec![];
        };
        let Some(step) = self.current_step().copied() else {
            return vec![];
        };

        let samples = self.collect_samples(&state, &step);
        for (imu, sample) in samples.iter() {
            self.live[*imu] = Some(*sample);
        }
        self.feed(&samples);

        vec![]
    }

    /// Handle one key press.
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Vec<InterfaceCommand> {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Char('a') | KeyCode::Esc => self.quit(),
            KeyCode::Char('s') | KeyCode::Char('r') | KeyCode::Enter => {
                self.start();
                vec![]
            }
            KeyCode::Char('n') | KeyCode::Down => {
                self.move_cursor(1);
                vec![]
            }
            KeyCode::Char('p') | KeyCode::Up => {
                self.move_cursor(-1);
                vec![]
            }
            KeyCode::Char('d') => {
                self.derive();
                vec![]
            }
            _ => vec![],
        }
    }
}

/// Pair each sensor's accelerometer and gyroscope capabilities from the capability report.
fn imu_sources(report: &InputCapabilityReport) -> Vec<ImuSource> {
    let mut sources: Vec<ImuSource> = Vec::new();

    for entry in report.get_capabilities() {
        let capability = entry.capability;
        let (name, is_gyro) = match capability {
            InputCapability::AccelerometerCenter => ("Center", false),
            InputCapability::AccelerometerLeft => ("Left", false),
            InputCapability::AccelerometerRight => ("Right", false),
            InputCapability::GyroscopeCenter => ("Center", true),
            InputCapability::GyroscopeLeft => ("Left", true),
            InputCapability::GyroscopeRight => ("Right", true),
            _ => continue,
        };

        // The first capability seen for a name creates that sensor
        let index = match sources.iter().position(|source| source.name == name) {
            Some(index) => index,
            None => {
                sources.push(ImuSource {
                    name: name.to_string(),
                    accel: None,
                    gyro: None,
                });
                sources.len() - 1
            }
        };

        let source = &mut sources[index];
        if is_gyro {
            source.gyro = Some(capability);
        } else {
            source.accel = Some(capability);
        }
    }

    sources.sort_by(|a, b| a.name.cmp(&b.name));
    sources
}
