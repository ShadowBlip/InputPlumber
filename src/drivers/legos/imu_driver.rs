use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::{types::SizedInteger, PackedStruct};

use super::{
    event::{Event, InertialEvent, InertialInput},
    hid_report::{InertialDataReport, InputReportType},
    GYRO_SCALE, HID_TIMEOUT, IMU_IID, INERTIAL_PACKET_SIZE, PIDS, VID,
};

pub struct IMUDriver {
    /// HIDRAW device instance
    device: HidDevice,
    /// State for the IMU Accelerometer
    accel_state: Option<InertialDataReport>,
    /// State for the IMU Gyroscope
    gyro_state: Option<InertialDataReport>,
}

impl IMUDriver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fmtpath = path.clone();
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;

        if info.vendor_id() != VID
            || !PIDS.contains(&info.product_id())
            || info.interface_number() != IMU_IID
        {
            return Err(format!("Device '{fmtpath}' is not a Legion Go S IMU Device").into());
        }
        Ok(Self {
            accel_state: None,
            device,
            gyro_state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; INERTIAL_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        if bytes_read != INERTIAL_PACKET_SIZE {
            return Ok(vec![]);
        }
        match self.handle_inertial_report(buf) {
            Ok(events) => Ok(events),
            Err(e) => {
                log::error!("Got error processing XinputDataReport: {e:?}");
                Ok(vec![])
            }
        }
    }

    /* Inertial Measurement Unit */
    /// Unpacks the buffer into a [InertialDataReport] structure and updates
    /// the internal accel_state and gyro_state.
    fn handle_inertial_report(
        &mut self,
        buf: [u8; INERTIAL_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = InertialDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::debug!("--- Input report ---");
        //log::debug!("{input_report}");
        //log::debug!(" ---- End Report ----");

        let report_type = match input_report.report_id {
            1 => InputReportType::AccelData,
            2 => InputReportType::GyroData,
            _ => {
                let report_id = input_report.report_id;
                return Err(format!("Unknown report type: {report_id}").into());
            }
        };

        match report_type {
            InputReportType::AccelData => {
                // Update the state
                let old_state = self.update_accel_state(input_report);
                // Translate the state into a stream of input events
                let events = self.translate_accel_data(old_state);
                Ok(events)
            }
            InputReportType::GyroData => {
                // Update the state
                let old_state = self.update_gyro_state(input_report);
                // Translate the state into a stream of input events
                let events = self.translate_gyro_data(old_state);
                Ok(events)
            }
        }
    }

    /// Update accel_state
    fn update_accel_state(
        &mut self,
        input_report: InertialDataReport,
    ) -> Option<InertialDataReport> {
        let old_state = self.accel_state;
        self.accel_state = Some(input_report);
        old_state
    }

    /// Update gyro_state
    fn update_gyro_state(
        &mut self,
        input_report: InertialDataReport,
    ) -> Option<InertialDataReport> {
        let old_state = self.gyro_state;
        self.gyro_state = Some(input_report);
        old_state
    }

    /// Translate the accel_state into individual events
    fn translate_accel_data(&self, old_state: Option<InertialDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.accel_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(old_state) = old_state else {
            return events;
        };
        if state.x != old_state.x || state.y != old_state.y || state.z != old_state.z {
            events.push(Event::Inertia(InertialEvent::Accelerometer(
                InertialInput {
                    x: -state.x.to_primitive(),
                    y: -state.y.to_primitive(),
                    z: -state.z.to_primitive(),
                },
            )))
        };

        events
    }

    /// Translate the gyro_state into individual events
    fn translate_gyro_data(&self, old_state: Option<InertialDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.gyro_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(old_state) = old_state else {
            return events;
        };

        if state.x != old_state.x || state.y != old_state.y || state.z != old_state.z {
            events.push(Event::Inertia(InertialEvent::Gyro(InertialInput {
                x: -state.x.to_primitive() * GYRO_SCALE,
                y: -state.y.to_primitive() * GYRO_SCALE,
                z: -state.z.to_primitive() * GYRO_SCALE,
            })))
        };

        events
    }
}
