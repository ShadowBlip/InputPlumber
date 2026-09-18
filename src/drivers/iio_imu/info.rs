use std::{
    collections::HashMap,
    error::Error,
    fmt, fs,
    io::{self, Read},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use nix::libc::{self, c_int};

use crate::drivers::iio_imu::event::AxisData;

/// Maximum delay before triggering device stall actions.
const STALL_TIMEOUT_MS: c_int = 2000;

const DEFAULT_SAMPLE_RATE: f64 = 200.0;

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

/// Blocks until exactly one record's worth of bytes is available, then
/// fills `buf` with it.
pub trait RecordSource: fmt::Debug + Send {
    fn read_one(&mut self, buf: &mut [u8]) -> io::Result<()>;
}

#[derive(Debug)]
pub struct BlockingFdSource {
    file: fs::File,
    kick_path: Option<PathBuf>,
}

impl BlockingFdSource {
    pub fn open(devnode: &Path, kick_path: Option<PathBuf>) -> io::Result<Self> {
        let file = fs::OpenOptions::new().read(true).open(devnode)?;
        Ok(Self { file, kick_path })
    }
}

impl RecordSource for BlockingFdSource {
    fn read_one(&mut self, buf: &mut [u8]) -> io::Result<()> {
        if let Some(path) = &self.kick_path {
            let _ = fs::read_to_string(path);
        }
        wait_readable(self.file.as_raw_fd())?;
        self.file.read_exact(buf)
    }
}

fn wait_readable(fd: c_int) -> io::Result<()> {
    let mut fds = [libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    }];
    loop {
        let ret = unsafe { libc::poll(fds.as_mut_ptr(), 1, STALL_TIMEOUT_MS) };
        if ret < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        if ret == 0 {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "no IMU sample within stall timeout; trigger may have stopped",
            ));
        }
        if fds[0].revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err(io::Error::other("iio buffer devnode reported error/hangup"));
        }
        return Ok(());
    }
}

/// Parsed form of a `scan_elements/in_*_type` value, e.g. "le:s16/16>>0".
#[derive(Debug, Clone, Copy)]
pub struct ScanFormat {
    little_endian: bool,
    signed: bool,
    /// Bits actually allotted in the record (padded to a byte boundary).
    storage_bits: u8,
    /// Bits of real, significant data once shifted into place.
    real_bits: u8,
    shift: u8,
}

impl ScanFormat {
    fn parse(s: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let (endian, rest) = s
            .split_once(':')
            .ok_or_else(|| format!("malformed scan type '{s}' (missing endianness)"))?;
        let little_endian = match endian {
            "le" => true,
            "be" => false,
            other => return Err(format!("unknown endianness '{other}' in scan type '{s}'").into()),
        };

        let (sign, rest) = rest.split_at(1);
        let signed = match sign {
            "s" => true,
            "u" => false,
            other => return Err(format!("unknown sign flag '{other}' in scan type '{s}'").into()),
        };

        let (bits_str, rest) = rest
            .split_once('/')
            .ok_or_else(|| format!("malformed scan type '{s}' (missing real bits)"))?;
        let real_bits: u8 = bits_str.parse()?;

        let (storage_str, shift_str) = rest
            .split_once(">>")
            .ok_or_else(|| format!("malformed scan type '{s}' (missing shift)"))?;
        let storage_bits: u8 = storage_str.parse()?;
        let shift: u8 = shift_str.parse()?;

        Ok(Self {
            little_endian,
            signed,
            storage_bits,
            real_bits,
            shift,
        })
    }

    fn storage_bytes(&self) -> usize {
        (self.storage_bits as usize).div_ceil(8)
    }

    /// Decode this channel's raw bytes into a signed integer.
    fn decode(&self, raw: &[u8]) -> i64 {
        let n = self.storage_bytes().min(8);
        let mut buf = [0u8; 8];
        if self.little_endian {
            buf[..n].copy_from_slice(&raw[..n]);
        } else {
            for i in 0..n {
                buf[n - 1 - i] = raw[i];
            }
        }
        let mut value = u64::from_le_bytes(buf);
        value >>= self.shift;
        let mask = if self.real_bits >= 64 {
            u64::MAX
        } else {
            (1u64 << self.real_bits) - 1
        };
        value &= mask;

        if self.signed && self.real_bits < 64 {
            let sign_bit = 1u64 << (self.real_bits - 1);
            if value & sign_bit != 0 {
                return (value as i64) - (1i64 << self.real_bits);
            }
        }
        value as i64
    }
}

/// A single enabled scan channel discovered from sysfs, before the
/// buffer's byte layout has been computed.
#[derive(Debug, Clone)]
pub struct DiscoveredChannel {
    /// e.g. "in_accel_x"
    pub id: String,
    pub axis: char,
    pub scan_index: i32,
    pub format: ScanFormat,
    pub offset: i64,
    pub scale: f64,
}

/// One channel's final position within a fixed-size buffer record, plus
/// its calibration data.
#[derive(Debug, Clone)]
pub struct ChannelLayout {
    byte_offset: usize,
    format: ScanFormat,
    offset: i64,
    scale: f64,
}

/// Fixed layout of one complete buffer record, computed once at startup.
#[derive(Debug, Clone, Default)]
pub struct RecordLayout {
    pub record_len: usize,
    pub accel: HashMap<char, ChannelLayout>,
    pub gyro: HashMap<char, ChannelLayout>,
}

pub fn read_trim(path: &Path) -> io::Result<String> {
    Ok(fs::read_to_string(path)?.trim().to_string())
}

/// Reads the device's mount matrix: a shared attribute first, then per-group.
pub fn read_mount_matrix(base: &Path) -> Option<String> {
    read_trim(&base.join("in_mount_matrix"))
        .ok()
        .or_else(|| read_trim(&base.join("in_accel_mount_matrix")).ok())
        .or_else(|| read_trim(&base.join("in_anglvel_mount_matrix")).ok())
}

/// Discover the x/y/z scan channels for one group from `scan_elements/in_<group>_<axis>_*`.
pub fn discover_group(
    base: &Path,
    prefix: &str,
) -> Result<Vec<DiscoveredChannel>, Box<dyn Error + Send + Sync>> {
    let scan_dir = base.join("scan_elements");
    let mut out = Vec::new();

    for axis in ['x', 'y', 'z'] {
        let id = format!("in_{prefix}_{axis}");
        let index_path = scan_dir.join(format!("{id}_index"));
        if !index_path.exists() {
            continue;
        }

        let scan_index: i32 = read_trim(&index_path)?.trim().parse()?;
        let type_str = read_trim(&scan_dir.join(format!("{id}_type")))?;
        let format = ScanFormat::parse(&type_str)?;

        let offset = read_trim(&base.join(format!("{id}_offset")))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let scale = read_trim(&base.join(format!("{id}_scale")))
            .ok()
            .and_then(|s| s.parse().ok())
            .or_else(|| {
                read_trim(&base.join(format!("in_{prefix}_scale")))
                    .ok()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(1.0);

        log::debug!(
            "Found channel {id} (scan_index={scan_index}, offset={offset}, scale={scale}, type={type_str})"
        );

        out.push(DiscoveredChannel {
            id,
            axis,
            scan_index,
            format,
            offset,
            scale,
        });
    }

    Ok(out)
}

/// Look up the `in_timestamp` scan channel, if the device exposes one.
pub fn discover_timestamp(
    base: &Path,
) -> Result<Option<(i32, ScanFormat)>, Box<dyn Error + Send + Sync>> {
    let scan_dir = base.join("scan_elements");
    let index_path = scan_dir.join("in_timestamp_index");
    if !index_path.exists() {
        return Ok(None);
    }
    let scan_index: i32 = read_trim(&index_path)?.trim().parse()?;
    let format = ScanFormat::parse(&read_trim(&scan_dir.join("in_timestamp_type"))?)?;
    Ok(Some((scan_index, format)))
}

enum ScanEntry {
    Accel(DiscoveredChannel),
    Gyro(DiscoveredChannel),
    Timestamp(ScanFormat),
}

/// Computes each channel's byte offset within one buffer record, packing
/// them in ascending scan-index order with natural alignment and padding
/// the whole record to the widest element.
pub fn compute_layout(
    accel: Vec<DiscoveredChannel>,
    gyro: Vec<DiscoveredChannel>,
    timestamp: Option<(i32, ScanFormat)>,
) -> RecordLayout {
    let mut all: Vec<(i32, ScanEntry)> = accel
        .into_iter()
        .map(|c| (c.scan_index, ScanEntry::Accel(c)))
        .chain(gyro.into_iter().map(|c| (c.scan_index, ScanEntry::Gyro(c))))
        .collect();
    if let Some((scan_index, format)) = timestamp {
        all.push((scan_index, ScanEntry::Timestamp(format)));
    }
    all.sort_by_key(|(scan_index, _)| *scan_index);

    let mut offset = 0usize;
    let mut max_align = 1usize;
    let mut accel_layout = HashMap::new();
    let mut gyro_layout = HashMap::new();

    for (_, entry) in all {
        let format = match &entry {
            ScanEntry::Accel(c) | ScanEntry::Gyro(c) => c.format,
            ScanEntry::Timestamp(f) => *f,
        };
        let size = format.storage_bytes().max(1);
        max_align = max_align.max(size);
        if !offset.is_multiple_of(size) {
            offset += size - (offset % size);
        }

        match entry {
            ScanEntry::Accel(c) => {
                accel_layout.insert(
                    c.axis,
                    ChannelLayout {
                        byte_offset: offset,
                        format: c.format,
                        offset: c.offset,
                        scale: c.scale,
                    },
                );
            }
            ScanEntry::Gyro(c) => {
                gyro_layout.insert(
                    c.axis,
                    ChannelLayout {
                        byte_offset: offset,
                        format: c.format,
                        offset: c.offset,
                        scale: c.scale,
                    },
                );
            }
            ScanEntry::Timestamp(_) => {}
        }
        offset += size;
    }

    if max_align > 0 && !offset.is_multiple_of(max_align) {
        offset += max_align - (offset % max_align);
    }

    RecordLayout {
        record_len: offset,
        accel: accel_layout,
        gyro: gyro_layout,
    }
}

pub fn decode_group(layout: &HashMap<char, ChannelLayout>, record: &[u8]) -> AxisData {
    let mut out = AxisData::default();
    for (&axis, chan) in layout.iter() {
        let end = chan.byte_offset + chan.format.storage_bytes();
        let raw = chan.format.decode(&record[chan.byte_offset..end]);
        let value = (raw + chan.offset) as f64 * chan.scale;
        match axis {
            'x' => out.roll = value,
            'y' => out.pitch = value,
            'z' => out.yaw = value,
            _ => unreachable!("discover_group only ever produces x/y/z"),
        }
    }
    out
}

fn write_bool_verified(path: &Path, value: bool) -> Result<(), Box<dyn Error + Send + Sync>> {
    let s = if value { "1" } else { "0" };
    fs::write(path, s)?;
    let actual = read_trim(path)?;
    if actual != s {
        return Err(format!(
            "write to {} did not take effect: wrote '{s}', read back '{actual}'",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn write_str_verified(path: &Path, value: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    fs::write(path, value)?;
    let actual = read_trim(path)?;
    if actual != value {
        return Err(format!(
            "write to {} did not take effect: wrote '{value}', read back '{actual}'",
            path.display()
        )
        .into());
    }
    Ok(())
}

const HRTIMER_TRIGGER_ROOT: &str = "/sys/kernel/config/iio/triggers/hrtimer";

/// How to obtain the trigger bound to a buffer during setup.
pub enum TriggerStrategy {
    /// Search for an existing IIO trigger that already references this device.
    FindExisting,
    /// Always create (or reuse) and bind a dedicated hrtimer trigger.
    Hrtimer(String),
}

/// Unbinds and removes a dedicated hrtimer trigger when the owning
/// driver is dropped, so it doesn't accumulate in configfs across
/// restarts or unplug/replug cycles.
pub struct HrtimerTriggerGuard {
    base: PathBuf,
    name: String,
}

impl Drop for HrtimerTriggerGuard {
    fn drop(&mut self) {
        let buffer_dir = if self.base.join("buffer0").is_dir() {
            self.base.join("buffer0")
        } else {
            self.base.join("buffer")
        };
        let _ = fs::write(buffer_dir.join("enable"), "0");
        let _ = fs::write(self.base.join("trigger/current_trigger"), "");
        let trigger_dir = Path::new(HRTIMER_TRIGGER_ROOT).join(&self.name);
        if let Err(e) = fs::remove_dir(&trigger_dir) {
            log::warn!(
                "Failed to remove hrtimer trigger {}: {e}",
                trigger_dir.display()
            );
        }
    }
}

fn bind_trigger(
    base: &Path,
    strategy: &TriggerStrategy,
    rate: f64,
) -> Result<Option<HrtimerTriggerGuard>, Box<dyn Error + Send + Sync>> {
    match strategy {
        TriggerStrategy::FindExisting => {
            if let Some(trigger) = find_trigger_for(base)? {
                write_str_verified(&base.join("trigger/current_trigger"), &trigger)
                    .map_err(|e| format!("failed to bind trigger '{trigger}': {e}"))?;
            } else {
                log::debug!(
                    "No matching trigger found for {}; assuming none required",
                    base.display()
                );
            }
            Ok(None)
        }
        TriggerStrategy::Hrtimer(name) => {
            if !Path::new(HRTIMER_TRIGGER_ROOT).is_dir() {
                return Err(
                    "hrtimer trigger support not available - is iio_trig_hrtimer loaded?".into(),
                );
            }
            let trigger_dir = Path::new(HRTIMER_TRIGGER_ROOT).join(name);
            if !trigger_dir.is_dir() {
                fs::create_dir(&trigger_dir).map_err(|e| {
                    format!(
                        "failed to create hrtimer trigger {}: {e}",
                        trigger_dir.display()
                    )
                })?;
            }

            // The hrtimer trigger only latches sampling_frequency at
            // bind/enable time, so this must happen before it's bound.
            match find_trigger_device(name) {
                Some(trig_dev) => {
                    let path = trig_dev.join("sampling_frequency");
                    match fs::write(&path, rate.to_string()) {
                        Ok(_) => {
                            if let Ok(actual) = read_trim(&path) {
                                log::info!("Set trigger sampling_frequency to {actual} Hz");
                            }
                        }
                        Err(e) => log::warn!("Failed to set trigger sampling_frequency: {e:?}"),
                    }
                }
                None => log::warn!(
                    "Could not locate trigger device for '{name}'; its firing rate will stay at kernel default"
                ),
            }

            write_str_verified(&base.join("trigger/current_trigger"), name)
                .map_err(|e| format!("failed to bind hrtimer trigger '{name}': {e}"))?;

            Ok(Some(HrtimerTriggerGuard {
                base: base.to_path_buf(),
                name: name.clone(),
            }))
        }
    }
}

/// Finds the `/sys/bus/iio/devices/triggerN` entry matching `name` exactly.
/// Doesn't gate on `trigger_now` since hrtimer triggers don't expose it.
fn find_trigger_device(name: &str) -> Option<PathBuf> {
    let devices_root = Path::new("/sys/bus/iio/devices");
    fs::read_dir(devices_root)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let path = entry.path();
            (read_trim(&path.join("name")).ok()? == name).then_some(path)
        })
}

/// Finds a trigger device for the given sensor
fn find_trigger_for(base: &Path) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    let own_name = read_trim(&base.join("name")).unwrap_or_default();
    if own_name.is_empty() {
        return Ok(None);
    }

    let devices_root = Path::new("/sys/bus/iio/devices");
    for entry in fs::read_dir(devices_root)? {
        let path = entry?.path();
        if path == base || !path.join("trigger_now").exists() {
            continue; // not a trigger device
        }
        if let Ok(name) = read_trim(&path.join("name")) {
            if name.contains(&own_name) {
                return Ok(Some(name));
            }
        }
    }
    Ok(None)
}

fn read_available_rates(base: &Path, channels: &[DiscoveredChannel], prefix: &str) -> Vec<f64> {
    for chan in channels {
        if let Ok(v) = read_trim(&base.join(format!("{}_sampling_frequency_available", chan.id))) {
            let rates: Vec<f64> = v
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if !rates.is_empty() {
                return rates;
            }
        }
    }
    read_trim(&base.join(format!("in_{prefix}_sampling_frequency_available")))
        .map(|v| {
            v.split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Write a sampling rate to the device. Tries per-channel first, then falls
/// back to the device-level attribute.
fn write_sample_rate(base: &Path, channels: &[DiscoveredChannel], prefix: &str, rate: f64) {
    for chan in channels {
        let path = base.join(format!("{}_sampling_frequency", chan.id));
        if path.exists() {
            match fs::write(&path, rate.to_string()) {
                Ok(_) => {
                    if let Ok(actual) = read_trim(&path) {
                        log::info!("Set {} sampling_frequency to {actual} Hz", chan.id);
                    }
                    return;
                }
                Err(e) => log::warn!(
                    "Per-channel sampling_frequency write failed for {}: {e:?}",
                    chan.id
                ),
            }
        }
    }

    let attr = format!("in_{prefix}_sampling_frequency");
    let path = base.join(&attr);
    match fs::write(&path, rate.to_string()) {
        Ok(_) => {
            if let Ok(actual) = read_trim(&path) {
                log::info!("Set device-level {attr} to {actual} Hz");
            }
        }
        Err(e) => log::warn!("Failed to set {attr}: {e:?}"),
    }
}

fn negotiate_sample_rate(
    base: &Path,
    channels: &[DiscoveredChannel],
    prefix: &str,
    target: Option<f64>,
) -> f64 {
    if channels.is_empty() {
        return DEFAULT_SAMPLE_RATE;
    }
    let rate = target.unwrap_or(DEFAULT_SAMPLE_RATE);
    let avail = read_available_rates(base, channels, prefix);

    let chosen = if avail.is_empty() {
        log::warn!("No available sample rates reported for {prefix}; requesting {rate} Hz anyway");
        rate
    } else if avail.contains(&rate) {
        rate
    } else {
        let max = avail.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        log::warn!(
            "Requested {rate} Hz not in available rates {avail:?} for {prefix}; using max {max} Hz"
        );
        max
    };

    write_sample_rate(base, channels, prefix, chosen);
    chosen
}

/// Clear any leftover enabled/triggered state from a previous crashed
/// instance before reconfiguring.
fn warm_reset(base: &Path, buffer_dir: &Path) {
    let _ = fs::write(base.join("trigger/current_trigger"), "");
    if fs::write(buffer_dir.join("enable"), "0").is_err() {
        thread::sleep(Duration::from_millis(100));
        let _ = fs::write(buffer_dir.join("enable"), "0");
    }
    thread::sleep(Duration::from_millis(50));
}

/// Result of `configure_buffer`. `trigger_guard` must be kept alive while
/// the buffer is in use, and is only `Some` for `TriggerStrategy::Hrtimer`.
pub struct BufferConfig {
    pub rate: f64,
    pub trigger_guard: Option<HrtimerTriggerGuard>,
}

/// Run the setup sequence
pub fn configure_buffer(
    base: &Path,
    accel: &[DiscoveredChannel],
    gyro: &[DiscoveredChannel],
    sample_rate: Option<f64>,
    trigger: TriggerStrategy,
) -> Result<BufferConfig, Box<dyn Error + Send + Sync>> {
    let buffer_dir = if base.join("buffer0").is_dir() {
        base.join("buffer0")
    } else {
        base.join("buffer")
    };
    let scan_dir = base.join("scan_elements");

    warm_reset(base, &buffer_dir);

    write_bool_verified(&buffer_dir.join("enable"), false)
        .map_err(|e| format!("failed to disable buffer before reconfiguring: {e}"))?;

    let accel_rate = negotiate_sample_rate(base, accel, "accel", sample_rate);
    let gyro_rate = negotiate_sample_rate(base, gyro, "anglvel", sample_rate);
    let effective_rate = if !accel.is_empty() {
        accel_rate
    } else {
        gyro_rate
    };

    let trigger_guard = bind_trigger(base, &trigger, effective_rate)?;

    for chan in accel.iter().chain(gyro.iter()) {
        write_bool_verified(&scan_dir.join(format!("{}_en", chan.id)), true)
            .map_err(|e| format!("failed to enable scan element {}: {e}", chan.id))?;
    }

    // Timestamp channel isn't decoded, but leaving it disabled would
    // change bytes_per_datum out from under compute_layout's count.
    let timestamp_en = scan_dir.join("in_timestamp_en");
    if timestamp_en.exists() {
        write_bool_verified(&timestamp_en, true)
            .map_err(|e| format!("failed to enable in_timestamp: {e}"))?;
    }

    // Lowest-latency watermark: wake as soon as a single sample is ready,
    // and never let the kernel accumulate more than one unread record.
    let watermark_path = buffer_dir.join("watermark");
    if watermark_path.exists() {
        write_str_verified(&watermark_path, "1")
            .map_err(|e| format!("failed to set watermark to 1: {e}"))?;
    }

    write_bool_verified(&buffer_dir.join("enable"), true)
        .map_err(|e| format!("failed to arm buffer: {e}"))?;

    Ok(BufferConfig {
        rate: effective_rate,
        trigger_guard,
    })
}
