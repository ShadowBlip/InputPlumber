use std::collections::HashSet;
use std::error::Error;
use tokio::{sync::mpsc, sync::mpsc::error::TryRecvError};

use crate::config::MountMatrix;
use crate::input::capability::{Capability, Source};

use super::bindings::glib::GFALSE;
use super::event::{AxisData, Event};
use super::runtime::{SscObject, SscRuntime};

pub struct Driver {
    runtime: SscRuntime,
    _gyroscope: SscObject,
    _accelerometer: SscObject,

    // Event handling / polling
    rx: mpsc::Receiver<Event>,
    filtered_events: HashSet<Capability>,

    mount_matrix: MountMatrix,
}

impl Driver {
    pub fn new(mount_matrix: Option<MountMatrix>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let runtime = SscRuntime::load()?;
        let (tx, rx) = mpsc::channel::<Event>(1024);

        let gyroscope = runtime.create_gyroscope()?;
        let gyroscope_tx = tx.clone();
        gyroscope.set_measurement_handler(&runtime, move |x, y, z| {
            _ = gyroscope_tx.try_send(Event::Gyro(AxisData {
                roll: x as f64,
                pitch: y as f64,
                yaw: z as f64,
            }));
        });

        let accelerometer = runtime.create_accelerometer()?;
        let accelerometer_tx = tx.clone();
        accelerometer.set_measurement_handler(&runtime, move |x, y, z| {
            _ = accelerometer_tx.try_send(Event::Accelerometer(AxisData {
                roll: x as f64,
                pitch: y as f64,
                yaw: z as f64,
            }));
        });

        Ok(Self {
            runtime,
            _gyroscope: gyroscope,
            _accelerometer: accelerometer,
            rx,
            filtered_events: Default::default(),
            mount_matrix: mount_matrix.unwrap_or_default(),
        })
    }

    pub fn update_filtered_events(&mut self, events: HashSet<Capability>) {
        self.filtered_events = events;
    }

    pub fn get_default_event_filter(
        &self,
    ) -> Result<HashSet<Capability>, Box<dyn Error + Send + Sync>> {
        Ok(HashSet::new())
    }

    /// Poll the device for data
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // libssc uses GLib and relies on the GLib main loop
        // Instead of creating a main loop and main context then managing it alongside InputPlumber's loop etc,
        // we can just perform an iteration of the GLib context every poll
        // Not sure how this will work if other parts of InputPlumber start using the GLib main loop too for some reason
        unsafe { (self.runtime.libglib.g_main_context_iteration)(std::ptr::null_mut(), GFALSE) };

        let mut events: Vec<Event> = vec![];

        loop {
            match self.rx.try_recv() {
                Ok(Event::Gyro(mut event)) => {
                    if self
                        .filtered_events
                        .contains(&Capability::Gyroscope(Source::Center))
                    {
                        continue;
                    }

                    self.rotate_value(&mut event);

                    events.push(Event::Gyro(event));
                }

                Ok(Event::Accelerometer(mut event)) => {
                    if self
                        .filtered_events
                        .contains(&Capability::Accelerometer(Source::Center))
                    {
                        continue;
                    }

                    self.rotate_value(&mut event);

                    events.push(Event::Accelerometer(event));
                }

                Err(TryRecvError::Empty) => break,
                Err(error) => return Err(format!("Error when handling SSC events: {error}").into()),
            }
        }

        Ok(events)
    }

    /// Rotate the given axis data according to the mount matrix. This is used
    /// to calculate the final value according to the sensor oritentation.
    /// This is taken from iio_imu/driver.rs
    // Values are intended to be multiplied as:
    //   x' = mxx * x + myx * y + mzx * z
    //   y' = mxy * x + myy * y + mzy * z
    //   z' = mxz * x + myz * y + mzz * z
    fn rotate_value(&self, value: &mut AxisData) {
        let x = value.roll;
        let y = value.pitch;
        let z = value.yaw;
        let mxx = self.mount_matrix.x[0];
        let myx = self.mount_matrix.x[1];
        let mzx = self.mount_matrix.x[2];
        let mxy = self.mount_matrix.y[0];
        let myy = self.mount_matrix.y[1];
        let mzy = self.mount_matrix.y[2];
        let mxz = self.mount_matrix.z[0];
        let myz = self.mount_matrix.z[1];
        let mzz = self.mount_matrix.z[2];
        value.roll = mxx * x + myx * y + mzx * z;
        value.pitch = mxy * x + myy * y + mzy * z;
        value.yaw = mxz * x + myz * y + mzz * z;
    }
}
