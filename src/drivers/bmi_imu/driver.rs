use std::{collections::HashMap, error::Error};

use industrial_io::{Channel, ChannelType, Device};

use crate::drivers::bmi_imu::info::MountMatrix;

use super::{
    event::{AxisData, Event},
    info::AxisInfo,
};

/// Driver for reading BMI IMU data
pub struct Driver {
    mount_matrix: MountMatrix,
    accel: HashMap<String, Channel>,
    accel_info: HashMap<String, AxisInfo>,
    gyro: HashMap<String, Channel>,
    gyro_info: HashMap<String, AxisInfo>,
}

impl Driver {
    pub fn new(
        id: String,
        name: String,
        matrix: Option<MountMatrix>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        log::debug!("Creating BMI IMU driver instance for {name}");

        // Create an IIO local context used to query for devices
        let ctx = industrial_io::context::Context::new()?;
        log::debug!("IIO context version: {}", ctx.version());

        // Find the IMU device
        let Some(device) = ctx.find_device(id.as_str()) else {
            return Err("Failed to find device".into());
        };

        // Try finding the mount matrix to determine how sensors were mounted inside
        // the device.
        // https://github.com/torvalds/linux/blob/master/Documentation/devicetree/bindings/iio/mount-matrix.txt
        let mount_matrix = if let Some(matrix) = matrix {
            // Use the provided mount matrix if it is defined
            matrix
        } else if let Some(mount) = device.find_channel("mount", false) {
            // Read from the matrix
            let matrix_str = mount.attr_read_str("matrix")?;
            log::debug!("Found mount matrix: {matrix_str}");
            let matrix = MountMatrix::new(matrix_str)?;
            log::debug!("Decoded mount matrix: {matrix}");
            matrix
        } else {
            MountMatrix::default()
        };

        // Find all accelerometer and gyro channels and insert them into a hashmap
        let (accel, accel_info) = get_channels_with_type(&device, ChannelType::Accel);
        let (gyro, gyro_info) = get_channels_with_type(&device, ChannelType::AnglVel);

        // Log device attributes
        for attr in device.attributes() {
            log::debug!("Found device attribute: {:?}", attr)
        }

        // Log all found channels
        for channel in device.channels() {
            log::debug!("Found channel: {:?} {:?}", channel.id(), channel.name());
            log::debug!("  Is output: {}", channel.is_output());
            log::debug!("  Is scan element: {}", channel.is_scan_element());
            for attr in channel.attrs() {
                log::debug!("  Found attribute: {:?}", attr);
            }
        }

        Ok(Self {
            mount_matrix,
            accel,
            accel_info,
            gyro,
            gyro_info,
        })
    }

    /// Poll the device for data
    pub fn poll(&self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut events = vec![];

        // Read from the accelerometer
        if let Some(event) = self.poll_accel()? {
            events.push(event);
        }

        // Read from the gyro
        if let Some(event) = self.poll_gyro()? {
            events.push(event);
        }

        Ok(events)
    }

    /// Polls all the channels from the accelerometer
    fn poll_accel(&self) -> Result<Option<Event>, Box<dyn Error + Send + Sync>> {
        // Read from each accel channel
        let mut accel_input = AxisData::default();
        for (id, channel) in self.accel.iter() {
            // Get the info for the axis and read the data
            let Some(info) = self.accel_info.get(id) else {
                continue;
            };
            let data = channel.attr_read::<i64>("raw")?;

            // processed_value = (raw + offset) * scale
            let value = (data + info.offset) as f64 * info.scale;
            if id.ends_with('x') {
                accel_input.x = value;
            }
            if id.ends_with('y') {
                accel_input.y = value;
            }
            if id.ends_with('z') {
                accel_input.z = value;
            }
        }
        self.rotate_value(&mut accel_input);

        Ok(Some(Event::Accelerometer(accel_input)))
    }

    /// Polls all the channels from the gyro
    fn poll_gyro(&self) -> Result<Option<Event>, Box<dyn Error + Send + Sync>> {
        // Read from each accel channel
        let mut gyro_input = AxisData::default();
        for (id, channel) in self.gyro.iter() {
            // Get the info for the axis and read the data
            let Some(info) = self.gyro_info.get(id) else {
                continue;
            };
            let data = channel.attr_read::<i64>("raw")?;

            // processed_value = (raw + offset) * scale
            let value = (data + info.offset) as f64 * info.scale;

            if id.ends_with('x') {
                gyro_input.x = value;
            }
            if id.ends_with('y') {
                gyro_input.y = value;
            }
            if id.ends_with('z') {
                gyro_input.z = value;
            }
        }
        self.rotate_value(&mut gyro_input);

        Ok(Some(Event::Gyro(gyro_input)))
    }

    /// Rotate the given axis data according to the mount matrix. This is used
    /// to calculate the final value according to the sensor oritentation.
    // Values are intended to be multiplied as:
    //   x' = mxx * x + myx * y + mzx * z
    //   y' = mxy * x + myy * y + mzy * z
    //   z' = mxz * x + myz * y + mzz * z
    fn rotate_value(&self, value: &mut AxisData) {
        let x = value.x;
        let y = value.y;
        let z = value.z;
        let mxx = self.mount_matrix.x.0;
        let myx = self.mount_matrix.x.1;
        let mzx = self.mount_matrix.x.2;
        let mxy = self.mount_matrix.y.0;
        let myy = self.mount_matrix.y.1;
        let mzy = self.mount_matrix.y.2;
        let mxz = self.mount_matrix.z.0;
        let myz = self.mount_matrix.z.1;
        let mzz = self.mount_matrix.z.2;
        value.x = mxx * x + myx * y + mzx * z;
        value.y = mxy * x + myy * y + mzy * z;
        value.z = mxz * x + myz * y + mzz * z;
    }
}

/// Returns all channels and channel information from the given device matching
/// the given channel type.
fn get_channels_with_type(
    device: &Device,
    channel_type: ChannelType,
) -> (HashMap<String, Channel>, HashMap<String, AxisInfo>) {
    let mut channels = HashMap::new();
    let mut channel_info = HashMap::new();
    device
        .channels()
        .filter(|channel| channel.channel_type() == channel_type)
        .for_each(|channel| {
            let Some(id) = channel.id() else {
                log::warn!("Unable to get channel id for channel: {:?}", channel);
                return;
            };
            log::debug!("Found channel: {id}");

            // Get the scale of the axis to normalize values to meters per second or rads per
            // second
            let scale = match channel.attr_read::<f64>("scale") {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("Unable to read scale for channel {id}: {:?}", e);
                    1.0
                }
            };

            // Get the offset of the axis
            let offset = match channel.attr_read::<i64>("offset") {
                Ok(v) => v,
                Err(e) => {
                    log::debug!("Unable to read offset for channel {id}: {:?}", e);
                    0
                }
            };

            let info = AxisInfo { scale, offset };
            channel_info.insert(id.clone(), info);
            channels.insert(id, channel);
        });

    (channels, channel_info)
}
