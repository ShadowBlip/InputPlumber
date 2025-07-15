use std::{error::Error, fmt};

/// The [MountMatrix] is used to define how sensors are oriented inside a device
/// https://github.com/torvalds/linux/blob/master/Documentation/devicetree/bindings/iio/mount-matrix.txt
#[derive(Clone, Debug)]
pub struct MountMatrix {
    pub x: (f64, f64, f64),
    pub y: (f64, f64, f64),
    pub z: (f64, f64, f64),
}

impl MountMatrix {
    /// Create a new [MountMatrix] from the given mount matrix string
    /// Example:
    /// "1, 0, 0; 0, 1, 0; 0, 0, 1"
    pub fn new(matrix_str: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let parts: Vec<&str> = matrix_str.split(';').map(|part| part.trim()).collect();
        if parts.len() != 3 {
            return Err("Not enough parts found in the matrix".into());
        }

        let mut matrix = MountMatrix::default();
        for (i, part) in parts.into_iter().enumerate() {
            let sub_parts: Vec<&str> = part.split(',').map(|part| part.trim()).collect();
            if sub_parts.len() != 3 {
                return Err("Not enough subparts".into());
            }
            let x = sub_parts.first().unwrap().parse::<f64>()?;
            let y = sub_parts.get(1).unwrap().parse::<f64>()?;
            let z = sub_parts.get(2).unwrap().parse::<f64>()?;

            match i {
                0 => matrix.x = (x, y, z),
                1 => matrix.y = (x, y, z),
                2 => matrix.z = (x, y, z),
                _ => (),
            }
        }

        Ok(matrix)
    }
}

impl Default for MountMatrix {
    fn default() -> Self {
        MountMatrix {
            x: (1.0, 0.0, 0.0),
            y: (0.0, 1.0, 0.0),
            z: (0.0, 0.0, 1.0),
        }
    }
}

impl fmt::Display for MountMatrix {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "--- Mount Matrix ---\nx: {}, {}, {}\ny: {}, {}, {}\nz: {}, {}, {}",
            self.x.0,
            self.x.1,
            self.x.2,
            self.y.0,
            self.y.1,
            self.y.2,
            self.z.0,
            self.z.1,
            self.z.2,
        )
    }
}

/// The scale and offset information for a particular axis. These are used to
/// normalize data into real units.
///   processed_value = (raw + offset) * scale
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct AxisInfo {
    pub offset: i64,
    pub sample_rate: f64,
    pub sample_rates_avail: Vec<f64>,
    pub scale: f64,
    pub scales_avail: Vec<f64>,
}
