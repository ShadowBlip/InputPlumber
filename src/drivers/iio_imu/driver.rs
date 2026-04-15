use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs::File,
    io::{self, BufRead, BufReader},
};

use industrial_io::{Channel, ChannelType, Device, Direction};

use crate::{
    drivers::iio_imu::info::MountMatrix,
    input::capability::{Capability, Source},
};

use super::{
    event::{AxisData, Event},
    info::AxisInfo,
};

const DEFAULT_SAMPLE_RATE: f64 = 200.0;

/// Driver for reading IIO IMU data
pub struct Driver {
    _device: Device, // must outlive Channel raw pointers
    mount_matrix: MountMatrix,
    accel: HashMap<String, Channel>,
    accel_info: HashMap<String, AxisInfo>,
    gyro: HashMap<String, Channel>,
    gyro_info: HashMap<String, AxisInfo>,
    /// List of events that should not be generated
    filtered_events: HashSet<Capability>,
}

impl Driver {
    pub fn new(
        id: String,
        name: String,
        matrix: Option<MountMatrix>,
        sample_rate: Option<f64>,
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
        } else if let Some(mount) = device.find_channel("mount", Direction::Input) {
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

        if !accel.is_empty() {
            set_sample_rate(&device, &accel, ChannelType::Accel, sample_rate);
        }
        if !gyro.is_empty() {
            set_sample_rate(&device, &gyro, ChannelType::AnglVel, sample_rate);
        }

        Ok(Self {
            _device: device,
            mount_matrix,
            accel,
            accel_info,
            gyro,
            gyro_info,
            filtered_events: Default::default(),
        })
    }

    //TODO: Using InputPlumber Capability enum prevents this driver from having the ability to be
    //a standalone crate. When this driver is eventually separated, refactor the Event type to
    //follow the pattern DeviceEvent(Event, Value) and create a match table for
    //Capability->Event/Event->Capability in the SourceDriver implementation.
    pub fn update_filtered_events(&mut self, events: HashSet<Capability>) {
        self.filtered_events = events;
    }

    pub fn get_default_event_filter(
        &self,
    ) -> Result<HashSet<Capability>, Box<dyn Error + Send + Sync>> {
        let filtered_events = match is_driver_loaded("hid_lenovo_go") {
            Ok(true) => {
                log::debug!("Found hid-lenovo-go driver. Disabling internal gyroscope.");
                HashSet::from([
                    Capability::Accelerometer(Source::Center),
                    Capability::Gyroscope(Source::Center),
                ])
            }
            Ok(false) => {
                log::debug!("Did not find hid-lenovo-go driver. Enabling internal gyroscope.");
                HashSet::new()
            }
            Err(e) => {
                return Err(format!("Failed to read '/proc/modules': {e:?}").into());
            }
        };
        Ok(filtered_events)
    }

    /// Poll the device for data
    pub fn poll(&self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut events = vec![];

        // Read from the accelerometer
        if !self
            .filtered_events
            .contains(&Capability::Accelerometer(Source::Center))
        {
            if let Some(event) = self.poll_accel()? {
                events.push(event);
            }
        }

        // Read from the gyro
        if !self
            .filtered_events
            .contains(&Capability::Gyroscope(Source::Center))
        {
            if let Some(event) = self.poll_gyro()? {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Polls all the channels from the accelerometer
    fn poll_accel(&self) -> Result<Option<Event>, Box<dyn Error + Send + Sync>> {
        if self.accel.is_empty() {
            return Ok(None);
        }

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
                accel_input.roll = value;
            }
            if id.ends_with('y') {
                accel_input.pitch = value;
            }
            if id.ends_with('z') {
                accel_input.yaw = value;
            }
        }
        self.rotate_value(&mut accel_input);

        Ok(Some(Event::Accelerometer(accel_input)))
    }

    /// Polls all the channels from the gyro
    fn poll_gyro(&self) -> Result<Option<Event>, Box<dyn Error + Send + Sync>> {
        if self.gyro.is_empty() {
            return Ok(None);
        }

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
                gyro_input.roll = value;
            }
            if id.ends_with('y') {
                gyro_input.pitch = value;
            }
            if id.ends_with('z') {
                gyro_input.yaw = value;
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
        let x = value.roll;
        let y = value.pitch;
        let z = value.yaw;
        let mxx = self.mount_matrix.x.0;
        let myx = self.mount_matrix.x.1;
        let mzx = self.mount_matrix.x.2;
        let mxy = self.mount_matrix.y.0;
        let myy = self.mount_matrix.y.1;
        let mzy = self.mount_matrix.y.2;
        let mxz = self.mount_matrix.z.0;
        let myz = self.mount_matrix.z.1;
        let mzz = self.mount_matrix.z.2;
        value.roll = mxx * x + myx * y + mzx * z;
        value.pitch = mxy * x + myy * y + mzy * z;
        value.yaw = mxz * x + myz * y + mzz * z;
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

fn is_driver_loaded(driver_name: &str) -> io::Result<bool> {
    let file = File::open("/proc/modules")?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        if line.starts_with(driver_name) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn set_sample_rate(
    device: &Device,
    channels: &HashMap<String, Channel>,
    channel_type: ChannelType,
    target_rate: Option<f64>,
) {
    let rate = if let Some(r) = target_rate {
        log::debug!("Using configured sample rate: {r} Hz");
        r
    } else {
        let avail = read_sample_rates_available(device, channels, &channel_type);
        if !avail.is_empty() {
            let max = avail.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            log::debug!("Using max available sample rate: {max} Hz");
            max
        } else {
            log::debug!("No available rates found, using default: {DEFAULT_SAMPLE_RATE} Hz");
            DEFAULT_SAMPLE_RATE
        }
    };

    // per-channel attribute
    for (id, channel) in channels.iter() {
        match channel.attr_write_float("sampling_frequency", rate) {
            Ok(_) => {
                let actual = channel
                    .attr_read_float("sampling_frequency")
                    .unwrap_or(rate);
                log::debug!("Set sampling_frequency to {actual} Hz via channel {id}");
                return;
            }
            Err(e) => {
                log::debug!(
                    "Per-channel sampling_frequency write failed for {id}: {e:?}, trying device-level"
                );
            }
        }
    }

    // device-level fallback
    let attr = match channel_type {
        ChannelType::Accel => "in_accel_sampling_frequency",
        ChannelType::AnglVel => "in_anglvel_sampling_frequency",
        _ => return,
    };

    match device.attr_write_float(attr, rate) {
        Ok(_) => {
            let actual = device.attr_read_float(attr).unwrap_or(rate);
            log::info!("Set device-level {attr} to {actual} Hz");
        }
        Err(e) => {
            log::warn!("Failed to set {attr}: {e:?}");
        }
    }
}

fn read_sample_rates_available(
    device: &Device,
    channels: &HashMap<String, Channel>,
    channel_type: &ChannelType,
) -> Vec<f64> {
    for (_, channel) in channels.iter() {
        if let Ok(v) = channel.attr_read_str("sampling_frequency_available") {
            let rates: Vec<f64> = v
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if !rates.is_empty() {
                return rates;
            }
        }
    }

    let attr = match channel_type {
        ChannelType::Accel => "in_accel_sampling_frequency_available",
        ChannelType::AnglVel => "in_anglvel_sampling_frequency_available",
        _ => return vec![],
    };

    if let Ok(v) = device.attr_read_str(attr) {
        let rates: Vec<f64> = v
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if !rates.is_empty() {
            return rates;
        }
    }

    vec![]
}
