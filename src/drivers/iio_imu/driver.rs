use std::{
    collections::HashSet,
    error::Error,
    fmt, fs,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use crate::input::capability::{Capability, Source};

use super::{
    event::{AxisData, Event},
    info::{
        compute_layout, configure_buffer, decode_group, discover_group, discover_timestamp,
        read_mount_matrix, BlockingFdSource, HrtimerTriggerGuard, MountMatrix, RecordLayout,
        RecordSource, TriggerStrategy,
    },
};

pub struct Driver {
    mount_matrix: MountMatrix,
    layout: RecordLayout,
    source: Box<dyn RecordSource>,
    record_buf: Vec<u8>,
    filtered_events: HashSet<Capability>,
    poll_interval: Duration,
    accel_state: Option<AxisData>,
    gyro_state: Option<AxisData>,
    // Kept alive only so its Drop unbinds/removes the hrtimer trigger.
    _trigger_guard: Option<HrtimerTriggerGuard>,
}

impl fmt::Debug for Driver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Driver")
            .field("layout", &self.layout)
            .finish()
    }
}

impl Driver {
    /// Create a new IIO IMU driver instance bound to the given trigger strategy.
    pub fn new(
        name: String,
        matrix: Option<MountMatrix>,
        sample_rate: Option<f64>,
        trigger: TriggerStrategy,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        log::debug!("Creating IIO IMU driver instance for {name}");

        let base = PathBuf::from(format!("/sys/bus/iio/devices/{name}"));
        if !base.is_dir() {
            return Err(format!("IIO device path not found: {}", base.display()).into());
        }
        let devnode = PathBuf::from(format!("/dev/{name}"));

        let mount_matrix = if let Some(matrix) = matrix {
            matrix
        } else if let Some(matrix_str) = read_mount_matrix(&base) {
            log::debug!("Found mount matrix: {matrix_str}");
            let matrix = MountMatrix::new(matrix_str)?;
            log::debug!("Decoded mount matrix: {matrix}");
            matrix
        } else {
            MountMatrix::default()
        };

        let accel = discover_group(&base, "accel")?;
        let gyro = discover_group(&base, "anglvel")?;
        let timestamp = discover_timestamp(&base)?;

        log::debug!("accel present: {}", !accel.is_empty());
        log::debug!("gyro present: {}", !gyro.is_empty());
        log::debug!("timestamp present: {}", timestamp.is_some());

        // Devices bound via TriggerStrategy::FindExisting (e.g. HID sensor
        // hub devices) don't reliably push samples on their own; reading
        // their `_raw` attribute nudges the kernel into producing one.
        let kick_path = match &trigger {
            TriggerStrategy::FindExisting => accel
                .first()
                .or(gyro.first())
                .map(|chan| base.join(format!("{}_raw", chan.id))),
            TriggerStrategy::Hrtimer(_) => None,
        };

        let buffer_config = configure_buffer(&base, &accel, &gyro, sample_rate, trigger)?;
        let poll_interval = Duration::from_secs_f64(1.0 / buffer_config.rate);

        let layout = compute_layout(accel, gyro, timestamp);
        log::debug!("Computed record layout: {layout:?}");

        let source = BlockingFdSource::open(&devnode, kick_path)
            .map_err(|e| format!("failed to open {}: {e}", devnode.display()))?;

        let record_buf = vec![0u8; layout.record_len];

        Ok(Self {
            mount_matrix,
            layout,
            source: Box::new(source),
            record_buf,
            filtered_events: Default::default(),
            poll_interval,
            accel_state: None,
            gyro_state: None,
            _trigger_guard: buffer_config.trigger_guard,
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
        match fs::read_to_string("/proc/modules") {
            Ok(modules) if modules.contains("hid_lenovo_go") => Ok(HashSet::from([
                Capability::Accelerometer(Source::Center),
                Capability::Gyroscope(Source::Center),
            ])),
            Ok(_) => Ok(HashSet::new()),
            Err(e) => Err(format!("Failed to read '/proc/modules': {e:?}").into()),
        }
    }

    /// Skips a group's event if unchanged since the last poll; paces total call time to poll_interval.
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let start = Instant::now();

        self.source.read_one(&mut self.record_buf)?;

        let mut events = Vec::new();

        if !self.layout.accel.is_empty()
            && !self
                .filtered_events
                .contains(&Capability::Accelerometer(Source::Center))
        {
            let mut data = decode_group(&self.layout.accel, &self.record_buf);
            self.rotate_value(&mut data);
            if self.accel_state.as_ref() != Some(&data) {
                self.accel_state = Some(data.clone());
                events.push(Event::Accelerometer(data));
            }
        }

        if !self.layout.gyro.is_empty()
            && !self
                .filtered_events
                .contains(&Capability::Gyroscope(Source::Center))
        {
            let mut data = decode_group(&self.layout.gyro, &self.record_buf);
            self.rotate_value(&mut data);
            if self.gyro_state.as_ref() != Some(&data) {
                self.gyro_state = Some(data.clone());
                events.push(Event::Gyro(data));
            }
        }

        log::trace!("Got IIO IMU events: {:?}", events);

        if let Some(remaining) = self.poll_interval.checked_sub(start.elapsed()) {
            thread::sleep(remaining);
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
