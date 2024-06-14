use std::{collections::HashMap, error::Error, time::Duration};

use industrial_io::{Channel, ChannelType, Device};

use crate::drivers::iio_imu::info::MountMatrix;

use super::{
    event::{AxisData, Event},
    info::AxisInfo,
};

/// Driver for reading IIO IMU data
pub struct Driver {
    mount_matrix: MountMatrix,
    accel: HashMap<String, Channel>,
    accel_info: HashMap<String, AxisInfo>,
    gyro: HashMap<String, Channel>,
    gyro_info: HashMap<String, AxisInfo>,
    pub sample_delay: Duration,
}

impl Driver {
    pub fn new(
        id: String,
        name: String,
        matrix: Option<MountMatrix>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        log::debug!("Creating IIO IMU driver instance for {name}");

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
        for attr in &accel_info {
            log::debug!("Found accel_info: {:?}", attr);
        }
        let (gyro, gyro_info) = get_channels_with_type(&device, ChannelType::AnglVel);
        for attr in &gyro_info {
            log::debug!("Found gyro_info: {:?}", attr);
        }

        // Log device attributes
        for attr in device.attributes() {
            log::trace!("Found device attribute: {:?}", attr)
        }

        // Log all found channels
        for channel in device.channels() {
            log::trace!("Found channel: {:?} {:?}", channel.id(), channel.name());
            log::trace!("  Is output: {}", channel.is_output());
            log::trace!("  Is scan element: {}", channel.is_scan_element());
            for attr in channel.attrs() {
                log::trace!("  Found attribute: {:?}", attr);
            }
        }

        // Calculate the initial sample delay

        Ok(Self {
            mount_matrix,
            accel,
            accel_info,
            gyro,
            gyro_info,
            sample_delay: Duration::from_micros(2500), //400Hz
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
            let data = channel.attr_read_int("raw")?;

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
            let data = channel.attr_read_int("raw")?;

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

    /// Calculates the duration in seconds that this device should sleep for before polling for
    /// events again. Uses the fastest frequency set between the currently set accelerometer and
    /// gyroscope sample rates. Called automatically when the sample_rate is changed.
    pub fn calculate_sample_delay(&self) -> Result<Duration, Box<dyn Error>> {
        let accel_rate = self.get_sample_rate("accel").unwrap_or(1.0);
        let gyro_rate = self.get_sample_rate("gyro").unwrap_or(1.0);
        let mut sample_delay = 1.0 / accel_rate.max(gyro_rate);
        if sample_delay <= 0.0 {
            sample_delay = 0.0025;
        }
        log::debug!("Updated sample delay is: {sample_delay} seconds.");

        Ok(Duration::from_secs_f64(sample_delay))
    }

    /// Returns the currently set sample rate from the X axis of the given input source, either
    /// "accel" or "gyro".
    pub fn get_sample_rate(&self, imu_type: &str) -> Result<f64, Box<dyn Error>> {
        match imu_type {
            "accel" => {
                let Some(info) = self.accel_info.get("accel_x") else {
                    return Err(format!("Unable to get sample rate for IMU type {imu_type}").into());
                };

                Ok(info.sample_rate)
            }
            "gyro" => {
                let Some(info) = self.gyro_info.get("anglvel_x") else {
                    return Err(format!("Unable to get sample rate for IMU type {imu_type}").into());
                };
                //log::debug!("sample rate found: {:?}", info.sample_rate);

                Ok(info.sample_rate)
            }
            _ => Err(format!("{imu_type} is not a valid imu type.").into()),
        }
    }

    /// Returns the available sample rates for the given input source, either "accel" or "gyro".
    pub fn get_sample_rates_avail(&self, imu_type: &str) -> Result<Vec<f64>, Box<dyn Error>> {
        match imu_type {
            "accel" => {
                let Some(info) = self.accel_info.get("accel_x") else {
                    return Err(format!("Unable to get sample rate for IMU type {imu_type}").into());
                };

                Ok(info.sample_rates_avail.clone())
            }
            "gyro" => {
                let Some(info) = self.gyro_info.get("anglvel_x") else {
                    return Err(format!("Unable to get sample rate for IMU type {imu_type}").into());
                };
                //log::debug!("sample rates avail found: {:?}", info.sample_rates_avail);

                Ok(info.sample_rates_avail.clone())
            }
            _ => Err(format!("{imu_type} is not a valid imu type.").into()),
        }
    }

    /// Sets the given input source, either "accel" or "gyro", to the given rate. Returns an error
    /// if the given sample rate is not in the list of valid sample rates.
    pub fn set_sample_rate(&mut self, imu_type: &str, rate: f64) -> Result<(), Box<dyn Error>> {
        match imu_type {
            "accel" => {
                for (id, channel) in &self.accel {
                    let Some(info) = self.accel_info.get_mut(id) else {
                        return Err(format!(
                            "Unable to get channel info for {imu_type} channel {id}"
                        )
                        .into());
                    };

                    if !info.sample_rates_avail.contains(&rate) {
                        return Err(format!(
                            "Unable to set sample rate to {rate}, frequency not supported."
                        )
                        .into());
                    }

                    if info.sample_rate == rate {
                        log::debug!("IMU {imu_type} Channel {channel:?} already set to {rate:?}. Nothing to do.");
                        continue;
                    };

                    let result = channel.attr_write_float("sampling_frequency", rate);
                    match result {
                        Ok(_) => {
                            log::debug!("IMU {imu_type} Channel {channel:?} set to {rate:?}.");
                            info.sample_rate = rate;
                        }

                        Err(e) => {
                            return Err(format!(
                                "Unable to set sample rate for channel {id}, got error {e}"
                            )
                            .into())
                        }
                    }
                }
            }
            "gyro" => {
                for (id, channel) in &self.gyro {
                    let Some(info) = self.gyro_info.get_mut(id) else {
                        return Err(format!(
                            "Unable to get channel info for {imu_type} channel {id}"
                        )
                        .into());
                    };

                    if !info.sample_rates_avail.contains(&rate) {
                        return Err(format!(
                            "Unable to set sample rate to {rate}, frequency not supported."
                        )
                        .into());
                    }

                    if info.sample_rate == rate {
                        log::debug!("IMU {imu_type} Channel {channel:?} already set to {rate:?}. Nothing to do.");
                        continue;
                    };

                    let result = channel.attr_write_float("sampling_frequency", rate);
                    match result {
                        Ok(_) => {
                            log::debug!("IMU {imu_type} Channel {channel:?} set to {rate:?}.");
                            info.sample_rate = rate;
                        }

                        Err(e) => {
                            return Err(format!(
                                "Unable to set sample rate for channel {id}, got error {e}"
                            )
                            .into())
                        }
                    }
                }
            }
            _ => {
                return Err(format!("{imu_type} is not a valid imu type.").into());
            }
        }
        self.sample_delay = self.calculate_sample_delay()?;
        Ok(())
    }

    /// Returns the currently set cale from the X axis of the given input source, either "accel" or "gyro".
    pub fn get_scale(&self, imu_type: &str) -> Result<f64, Box<dyn Error>> {
        match imu_type {
            "accel" => {
                let Some(info) = self.accel_info.get("accel_x") else {
                    return Err(format!("Unable to get scale for IMU type {imu_type}.").into());
                };

                Ok(info.scale)
            }
            "gyro" => {
                let Some(info) = self.gyro_info.get("anglvel_x") else {
                    return Err(
                        format!("Unable to get scale available for IMU type {imu_type}.").into(),
                    );
                };

                Ok(info.scale)
            }
            _ => Err(format!("{imu_type} is not a valid imu type.").into()),
        }
    }

    /// Returns the available scales for the given input source, either "accel" or "gyro".
    pub fn get_scales_avail(&self, imu_type: &str) -> Result<Vec<f64>, Box<dyn Error>> {
        match imu_type {
            "accel" => {
                let Some(info) = self.accel_info.get("accel_x") else {
                    return Err(format!("Unable to scale for IMU type {imu_type}.").into());
                };

                Ok(info.scales_avail.clone())
            }
            "gyro" => {
                let Some(info) = self.gyro_info.get("anglvel_x") else {
                    return Err(
                        format!("Unable to get scales available for IMU type {imu_type}.").into(),
                    );
                };

                Ok(info.scales_avail.clone())
            }
            _ => Err(format!("{imu_type} is not a valid imu type.").into()),
        }
    }

    /// Sets the given input source, either "accel" or "gyro", to the given scale. Returns an error
    /// if the given scale is not in the list of valid sample rates.
    pub fn set_scale(&mut self, imu_type: &str, scale: f64) -> Result<(), Box<dyn Error>> {
        match imu_type {
            "accel" => {
                for (id, channel) in &self.accel {
                    let Some(info) = self.accel_info.get_mut(id) else {
                        return Err(format!(
                            "Unable to get channel info for {imu_type} channel {id}."
                        )
                        .into());
                    };

                    if !info.scales_avail.contains(&scale) {
                        return Err(format!(
                            "Unable to set scale to {scale}, scale not supported."
                        )
                        .into());
                    }

                    if info.scale == scale {
                        log::debug!("IMU {imu_type} Channel {channel:?} already set to {scale:?}. Nothing to do.");
                        continue;
                    };

                    let result = channel.attr_write_float("scale", scale);
                    match result {
                        Ok(_) => {
                            log::debug!("IMU {imu_type} Channel {channel:?} set to {scale:?}.");
                            info.scale = scale;
                        }

                        Err(e) => {
                            return Err(format!(
                                "Unable to set scale rate for channel {id}, got error {e}"
                            )
                            .into())
                        }
                    }
                }
            }
            "gyro" => {
                for (id, channel) in &self.gyro {
                    let Some(info) = self.gyro_info.get_mut(id) else {
                        return Err(format!(
                            "Unable to get channel info for {imu_type} channel {id}."
                        )
                        .into());
                    };

                    if !info.scales_avail.contains(&scale) {
                        return Err(format!(
                            "Unable to set scale to {scale}, scale not supported."
                        )
                        .into());
                    };

                    if info.scale == scale {
                        log::debug!("IMU {imu_type} Channel {channel:?} already set to {scale:?}. Nothing to do.");
                        continue;
                    };

                    let result = channel.attr_write_float("scale", scale);
                    match result {
                        Ok(_) => {
                            log::debug!("IMU {imu_type} Channel {channel:?} set to {scale:?}.");
                            info.scale = scale;
                        }

                        Err(e) => {
                            return Err(format!(
                                "Unable to set scale for channel {id}, got error {e}."
                            )
                            .into())
                        }
                    }
                }
            }
            _ => {
                return Err(format!("{imu_type} is not a valid imu type.").into());
            }
        }
        Ok(())
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

            // Get the offset of the axis
            let offset = match channel.attr_read_int("offset") {
                Ok(v) => v,
                Err(e) => {
                    log::debug!("Unable to read offset for channel {id}: {:?}", e);
                    0
                }
            };

            // Get the sample rate of the axis
            let sample_rate = match channel.attr_read_float("sampling_frequency") {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("Unable to read sample rate for channel {id}: {:?}", e);
                    4.0
                }
            };

            let sample_rates_avail = match channel.attr_read_str("sampling_frequency_available") {
                Ok(v) => {
                    let mut all_scales = Vec::new();
                    for val in v.split_whitespace() {
                        // convert the string into f64
                        all_scales.push(val.parse::<f64>().unwrap());
                    }
                    all_scales
                }
                Err(e) => {
                    log::warn!(
                        "Unable to read available sample rates for channel {id}: {:?}",
                        e
                    );
                    vec![4.0]
                }
            };

            // Get the scale of the axis to normalize values to meters per second or rads per
            // second
            let scale = match channel.attr_read_float("scale") {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("Unable to read scale for channel {id}: {:?}", e);
                    1.0
                }
            };

            let scales_avail = match channel.attr_read_str("scale_available") {
                Ok(v) => {
                    let mut all_scales = Vec::new();
                    for val in v.split_whitespace() {
                        // convert the string into f64
                        all_scales.push(val.parse::<f64>().unwrap());
                    }
                    all_scales
                }
                Err(e) => {
                    log::warn!("Unable to read available scales for channel {id}: {:?}", e);
                    vec![1.0]
                }
            };

            let info = AxisInfo {
                offset,
                sample_rate,
                sample_rates_avail,
                scale,
                scales_avail,
            };
            channel_info.insert(id.clone(), info);
            channels.insert(id, channel);
        });

    (channels, channel_info)
}
