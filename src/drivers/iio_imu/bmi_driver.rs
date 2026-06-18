use core::{option::Option::None, time::Duration};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs::{self},
    thread,
};

use industrial_io::{Buffer, Channel, ChannelType, Device, Direction};

use crate::input::capability::{Capability, Source};

use super::{
    event::{AxisData, Event},
    info::{Axis, AxisInfo, ImuGroup, MountMatrix},
};

const DEFAULT_SAMPLE_RATE: f64 = 200.0;

/// Driver for reading IIO IMU data
pub struct Driver {
    _device: Device, // must outlive Channel raw pointers
    buffer: Buffer,
    mount_matrix: MountMatrix,
    accel: Option<ImuGroup>,
    gyro: Option<ImuGroup>,
    /// List of events that should not be generated
    filtered_events: HashSet<Capability>,
}

impl Driver {
    pub fn new(
        name: String,
        matrix: Option<MountMatrix>,
        sample_rate: Option<f64>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // Create an IIO local context used to query for devices
        let ctx = industrial_io::context::Context::new()?;
        ctx.set_timeout(Duration::from_secs(5))?;
        log::debug!("IIO context version: {}", ctx.version());

        // Find the IMU device
        let Some(device) = ctx.find_device(name.as_str()) else {
            return Err("Failed to find device".into());
        };

        match Self::warm_reset_iio_buffer(&device, name.clone()) {
            Ok(_) => (),
            Err(_) => {
                log::warn!("Failed to reset device, it may not permit grab.");
            }
        };

        log::debug!("Creating IIO IMU driver instance for {:?}", device.name());

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

        // Find all accelerometer and gyro channels
        let (accel_ch, accel_info) = get_channels_with_type(&device, ChannelType::Accel);

        for attr in &accel_info {
            log::debug!("Found accel_info: {:?}", attr);
        }

        let (gyro_ch, gyro_info) = get_channels_with_type(&device, ChannelType::AnglVel);

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

        // Bind a trigger for this device
        if let Some(trigger) = ctx.devices().find(|d| {
            if let (Some(current), Some(iter)) = (device.name().as_deref(), d.name().as_deref()) {
                current != iter && iter.contains(current) && d.is_trigger()
            } else {
                false
            }
        }) {
            match device.set_trigger(&trigger) {
                Ok(_) => log::debug!("Set trigger for IMU to {:?}", trigger.name()),
                Err(e) => log::warn!(
                    "Failed to set trigger for IMU: {:?}. Assuming trigger already set.",
                    e
                ),
            };
        } else {
            log::debug!("Unable to find trigger for {name:?}");
        };

        // Enable scan elements for channels and request a higher sampling rate
        for channel in device.channels() {
            let id = channel.id().unwrap_or_default();

            if channel.has_attr("sampling_frequency") {
                let current_rate = match channel.attr_read_float("sampling_frequency") {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!(
                            "Unable to read sample rate for channel {}: {:?}",
                            id.clone(),
                            e
                        );
                        4.0
                    }
                };

                let channel_type = channel.channel_type();
                if let Err(err) = set_sample_rate_or_default(
                    &device,
                    id.clone(),
                    current_rate,
                    &channel,
                    channel_type,
                    sample_rate,
                ) {
                    log::warn!("Failed to set sample rate: {err}, falling back to max available");
                    set_sample_rate_max(&device, id.clone(), current_rate, &channel, channel_type);
                }
            }

            if channel.is_scan_element() {
                log::debug!("Channel {:?} in not enabled, enabling.", channel.id());
                channel.enable();
            }
            log::debug!(
                "Channel {:?} enabled state is {:?}",
                channel.id(),
                channel.is_enabled()
            );
        }

        // Create buffer
        let buffer = match device.create_buffer(64, false) {
            Ok(b) => b,
            Err(e) => {
                log::error!("create_buffer failed: {:?}", e);
                return Err(e.into());
            }
        };

        // Ensure minimum latency
        if buffer.has_attr("watermark") {
            buffer.attr_write_int("watermark", 1)?;
            println!("Successfully set buffer watermark to 1 sample.");
        }
        buffer.set_blocking_mode(false)?;

        // Build channel data
        let accel = Self::build_group(&accel_ch, &accel_info, "accel");
        let gyro = Self::build_group(&gyro_ch, &gyro_info, "anglvel");

        log::debug!("accel present: {:?}", accel.is_some());
        log::debug!("gyro present: {:?}", gyro.is_some());

        Ok(Self {
            _device: device,
            buffer,
            mount_matrix,
            accel,
            gyro,
            filtered_events: Default::default(),
        })
    }

    fn warm_reset_iio_buffer(device: &Device, name: String) -> std::io::Result<()> {
        let buffer_path = format!("/sys/bus/iio/devices/{}/buffer/enable", name);

        log::debug!("Executing dynamic buffer cycle reset...");

        // Force the kernel trigger assignment to clear out any stale, locked states
        if let Err(e) = device.remove_trigger() {
            log::debug!("Trigger already clear or handled: {:?}", e);
        }

        // Explicitly write 0 to sysfs to tear down any stuck hardware state machines
        if let Err(e) = fs::write(&buffer_path, "0") {
            log::debug!("Sysfs buffer disable skipped or already 0: {:?}", e);
        }

        thread::sleep(Duration::from_millis(100));

        // Cycle a mock single-sample buffer execution to flush out the kernel ring boundaries
        let mut buffer_init = device.create_buffer(1, false);

        if matches!(&buffer_init, Err(industrial_io::Error::Nix(errno)) if *errno as i32 == nix::errno::Errno::EBUSY as i32)
        {
            log::debug!("Unable to clear buffer with IOCTL, attempting forced drop");
            fs::write(&buffer_path, "0")?;
            thread::sleep(Duration::from_millis(100));
            buffer_init = device.create_buffer(1, false);
        }

        // Drop the initialization handles completely before building the true runtime configuration
        let tmp_buffer = buffer_init;
        std::mem::drop(tmp_buffer);

        log::debug!("Dynamic reset complete. udev node preserved.");
        Ok(())
    }

    fn build_axis(
        channels: &HashMap<String, Channel>,
        info: &HashMap<String, AxisInfo>,
        key: &str,
    ) -> Option<Axis> {
        let channel = match channels.get(key) {
            Some(c) => c.clone(),
            None => return None,
        };

        let info = match info.get(key) {
            Some(i) => i.clone(),
            None => return None,
        };

        Some(Axis { channel, info })
    }

    fn build_group(
        channels: &HashMap<String, Channel>,
        info: &HashMap<String, AxisInfo>,
        prefix: &str,
    ) -> Option<ImuGroup> {
        let x = Self::build_axis(channels, info, &format!("{prefix}_x"))?;
        let y = Self::build_axis(channels, info, &format!("{prefix}_y"))?;
        let z = Self::build_axis(channels, info, &format!("{prefix}_z"))?;

        Some(ImuGroup { x, y, z })
    }

    //TODO: Using InputPlumber Capability enum prevents this driver from having the ability to be
    //a standalone crate. When this driver is eventually separated, refactor the Event type to
    //follow the pattern DeviceEvent(Event, Value) and create a match table for
    //Capability->Event/Event->Capability in the SourceDriver implementation.
    pub fn update_filtered_events(&mut self, events: HashSet<Capability>) {
        self.filtered_events = events;
    }

    /// Poll the device for data
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut events = Vec::new();
        let refill_res = self.buffer.refill();

        // EAGAIN is IIO telling us refill would block but we're set to non-blocking. This happens
        // because no data is available in the buffer. We can trigger a single read of one channel
        // to fill the buffer and get it on the next loop.
        if matches!(&refill_res, Err(industrial_io::Error::Nix(errno)) if *errno as i32 == nix::errno::Errno::EAGAIN as i32)
            || matches!(&refill_res, Err(industrial_io::Error::Nix(errno)) if *errno as i32 == nix::errno::Errno::EBUSY as i32)
        {
            log::debug!("Buffer has no data. Forcing read to fill buffer");
            if let Some(accel) = &self.accel {
                match accel.x.channel.attr_read_int("raw") {
                    Ok(_) => return Ok(events),
                    Err(e) => return Err(format!("Unable to probe accel channel: {:?}", e).into()),
                };
            };
            if let Some(gyro) = &self.gyro {
                match gyro.x.channel.attr_read_int("raw") {
                    Ok(_) => return Ok(events),
                    Err(e) => return Err(format!("Unable to probe gyro channel: {:?}", e).into()),
                };
            };
            return Ok(events);
        } else if let Err(e) = refill_res {
            log::error!("True buffer error: {:?}", e);
            return Ok(events);
        }

        if self.accel.is_some()
            && !self
                .filtered_events
                .contains(&Capability::Accelerometer(Source::Center))
        {
            events.extend(self.poll_accel()?);
        }

        if self.gyro.is_some()
            && !self
                .filtered_events
                .contains(&Capability::Gyroscope(Source::Center))
        {
            events.extend(self.poll_gyro()?);
        }
        log::debug!("Got IIO IMU events: {:?}", events);

        Ok(events)
    }

    /// Polls all the channels from the accelerometer and drains the buffer
    fn poll_accel(&self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let accel = match &self.accel {
            Some(a) => a,
            None => return Ok(Vec::new()),
        };

        let mut events = Vec::new();
        let mut x_iter = self.buffer.channel_iter::<i32>(&accel.x.channel);
        let mut y_iter = self.buffer.channel_iter::<i32>(&accel.y.channel);
        let mut z_iter = self.buffer.channel_iter::<i32>(&accel.z.channel);

        // Continuous loop to exhaustively empty the kernel's filled buffer window
        while let (Some(x), Some(y), Some(z)) = (x_iter.next(), y_iter.next(), z_iter.next()) {
            log::debug!(
                "Raw accel X data: {x}, offset: {:?}, scale: {:?}",
                accel.x.info.offset,
                accel.x.info.scale
            );
            log::debug!(
                "Raw accel Y data: {y}, offset: {:?}, scale: {:?}",
                accel.y.info.offset,
                accel.y.info.scale
            );
            log::debug!(
                "Raw accel Z data: {z}, offset: {:?}, scale: {:?}",
                accel.z.info.offset,
                accel.z.info.scale
            );
            let mut out = AxisData {
                roll: (*x as f64 + accel.x.info.offset as f64) * accel.x.info.scale,
                pitch: (*y as f64 + accel.y.info.offset as f64) * accel.y.info.scale,
                yaw: (*z as f64 + accel.z.info.offset as f64) * accel.z.info.scale,
            };
            self.rotate_value(&mut out);
            events.push(Event::Accelerometer(out));
        }

        Ok(events)
    }

    /// Polls all the channels from the gyro and drains the buffer
    fn poll_gyro(&self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let gyro = match &self.gyro {
            Some(g) => g,
            None => return Ok(Vec::new()),
        };

        let mut events = Vec::new();
        let mut x_iter = self.buffer.channel_iter::<i32>(&gyro.x.channel);
        let mut y_iter = self.buffer.channel_iter::<i32>(&gyro.y.channel);
        let mut z_iter = self.buffer.channel_iter::<i32>(&gyro.z.channel);

        // Continuous loop to exhaustively empty the kernel's filled buffer window
        while let (Some(x), Some(y), Some(z)) = (x_iter.next(), y_iter.next(), z_iter.next()) {
            log::debug!(
                "Raw gyro X data: {x}, offset: {:?}, scale: {:?}",
                gyro.x.info.offset,
                gyro.x.info.scale
            );
            log::debug!(
                "Raw gyro Y data: {y}, offset: {:?}, scale: {:?}",
                gyro.y.info.offset,
                gyro.y.info.scale
            );
            log::debug!(
                "Raw gyro Z data: {z}, offset: {:?}, scale: {:?}",
                gyro.z.info.offset,
                gyro.z.info.scale
            );
            let mut out = AxisData {
                roll: (*x as f64 + gyro.x.info.offset as f64) * gyro.x.info.scale,
                pitch: (*y as f64 + gyro.y.info.offset as f64) * gyro.y.info.scale,
                yaw: (*z as f64 + gyro.z.info.offset as f64) * gyro.z.info.scale,
            };
            self.rotate_value(&mut out);
            events.push(Event::Gyro(out));
        }

        Ok(events)
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
            let id = channel.id().unwrap_or_default();
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
            let sample_rate = channel
                .attr_read_float("sampling_frequency")
                .unwrap_or_else(|_| {
                    let fallback_attr = match channel_type {
                        ChannelType::Accel => "in_accel_sampling_frequency",
                        ChannelType::AnglVel => "in_anglvel_sampling_frequency",
                        // Add other types here if your framework scales them later
                        _ => "",
                    };

                    if !fallback_attr.is_empty() {
                        device.attr_read_float(fallback_attr).unwrap_or(1.0)
                    } else {
                        4.0
                    }
                });

            let sample_rates_avail = read_sample_rates_available(id.clone(), sample_rate, &channel);
            // Get the scale of the axis to normalize values to meters per second^2 or rads per second
            let scale = channel.attr_read_float("scale").unwrap_or_else(|_| {
                let fallback_attr = match channel_type {
                    ChannelType::Accel => "in_accel_scale",
                    ChannelType::AnglVel => "in_anglvel_scale",
                    // Add other types here if your framework scales them later
                    _ => "",
                };

                if !fallback_attr.is_empty() {
                    device.attr_read_float(fallback_attr).unwrap_or(1.0)
                } else {
                    1.0
                }
            });

            let scales_avail = read_scales_available(id.clone(), scale, &channel);

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

/// Try to set a specific or default sampling rate. Returns Err if the
/// requested rate is not in the hardware's available list.
fn set_sample_rate_or_default(
    device: &Device,
    id: String,
    current_rate: f64,
    channel: &Channel,
    channel_type: ChannelType,
    target_rate: Option<f64>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let rate = target_rate.unwrap_or(DEFAULT_SAMPLE_RATE);

    let avail: Vec<f64> = read_sample_rates_available(id.clone(), current_rate, channel);

    if !avail.is_empty() && !avail.contains(&rate) {
        return Err(format!("Requested {rate} Hz not in available rates: {avail:?}").into());
    }

    write_sample_rate(device, channel, channel_type, rate)
}

/// Set sampling rate to the maximum reported by the hardware.
/// Falls back to DEFAULT_SAMPLE_RATE if no available rates are reported.
fn set_sample_rate_max(
    device: &Device,
    id: String,
    current_rate: f64,
    channel: &Channel,
    channel_type: ChannelType,
) {
    let avail = read_sample_rates_available(id.clone(), current_rate, channel);
    if avail.len() == 1 {
        log::info!("Channel {id} only has 1 sample rate available: {current_rate}");
        return;
    }

    let rate = if avail.is_empty() {
        log::warn!("No available sample rates reported, using default {DEFAULT_SAMPLE_RATE} Hz");
        DEFAULT_SAMPLE_RATE
    } else {
        let max = avail.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        log::info!("Using max available sample rate: {max} Hz");
        max
    };

    if rate == current_rate {
        log::info!("Channel {} is currently at {}", id.clone(), rate.clone());
        return;
    }

    if let Err(err) = write_sample_rate(device, channel, channel_type, rate) {
        log::warn!("Failed to set max sample rate: {err}");
    }
}

/// Write a sampling rate to the device. Tries per-channel for BMI-style,
/// and device-level for HID Sensor Hub.
fn write_sample_rate(
    device: &Device,
    channel: &Channel,
    channel_type: ChannelType,
    rate: f64,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let Some(name) = device.name() else {
        return Err("Unable to find name for IIO IMU device. Can't set sample rate".into());
    };

    if ["gyro_3d", "accel_3d"].contains(&name.as_str()) {
        write_sample_rate_to_device(device, channel_type, rate)
    } else {
        write_sample_rate_per_channel(channel, rate)
    }
}

fn write_sample_rate_to_device(
    device: &Device,
    channel_type: ChannelType,
    rate: f64,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("Setting rate to {rate} on IMU IIO Device");

    let attr = match channel_type {
        ChannelType::Accel => "in_accel_sampling_frequency",
        ChannelType::AnglVel => "in_anglvel_sampling_frequency",
        _ => return Err("Unknown channel type".into()),
    };

    device.attr_write_float(attr, rate)?;
    match device.attr_read_float(attr) {
        Ok(actual) => log::info!("Set device-level {attr} to {actual} Hz"),
        Err(err) => log::warn!("Set {attr} but read-back failed: {err}, assuming {rate} Hz"),
    }
    Ok(())
}
fn write_sample_rate_per_channel(
    channel: &Channel,
    rate: f64,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log::info!("Setting rate to {rate} on all channels of IIO IMU Device");

    let id = channel.id().unwrap_or_else(|| "Unknown".to_string());
    match channel.attr_write_float("sampling_frequency", rate) {
        Ok(_) => {
            match channel.attr_read_float("sampling_frequency") {
                    Ok(actual) => {
                        log::info!("Set sampling_frequency to {actual} Hz via channel {id}")
                    }
                    Err(err) => log::warn!(
                        "Set sampling_frequency for {id} but read-back failed: {err}, assuming {rate} Hz"
                    ),
                }
            return Ok(());
        }
        Err(err) => {
            log::warn!("Per-channel sampling_frequency write failed for {id}: {err}");
        }
    }
    Ok(())
}

/// Read the list of supported sampling rates from the hardware.
fn read_sample_rates_available(id: String, sample_rate: f64, channel: &Channel) -> Vec<f64> {
    match channel.has_attr("sampling_frequency_available") {
        true => {
            match channel.attr_read_str("sampling_frequency_available") {
                Ok(v) => {
                    let mut all_rates = Vec::new();
                    for val in v.split_whitespace() {
                        // convert the string into f64
                        all_rates.push(val.parse::<f64>().unwrap());
                    }
                    all_rates
                }
                Err(e) => {
                    log::warn!(
                        "Unable to read available sample rates for channel {id}: {:?}",
                        e
                    );
                    vec![sample_rate]
                }
            }
        }
        false => {
            log::warn!(
                "Unable to read available sample rates for channel {id}: attribute not available"
            );
            vec![sample_rate]
        }
    }
}

/// Read the list of supported scales from the hardware.
fn read_scales_available(id: String, scale: f64, channel: &Channel) -> Vec<f64> {
    match channel.has_attr("scale_available") {
        true => {
            match channel.attr_read_str("scale_available") {
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
                    vec![scale]
                }
            }
        }
        false => {
            log::warn!(
                "Unable to read available sample rates for channel {id}: attribute not available"
            );
            vec![scale]
        }
    }
}
