use std::{
    error::Error,
    ffi::CString,
    time::{Duration, Instant},
};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use super::{
    event::{Event, TouchAxisInput},
    hid_report::{PackedInputDataReport, TouchData},
};

/// Vendor ID
pub const VID: u16 = 0x2808;
/// Product ID
pub const PID: u16 = 0x1015;
/// Size of the HID packet
const PACKET_SIZE: usize = 60;
/// Timeout in milliseconds for reading an HID packet
//const HID_TIMEOUT: i32 = 5000;

/// State of a touch event and when it was last touched for detecting "up" events
#[derive(Debug, Clone)]
struct TouchState {
    /// Timestamp of the last touch event.
    last_touch: Instant,
    /// Whether or not we are detecting a touch event currently.
    is_touching: bool,
}

impl Default for TouchState {
    fn default() -> Self {
        Self {
            last_touch: Instant::now(),
            is_touching: false,
        }
    }
}

#[derive(Debug)]
pub struct Driver {
    state: Option<PackedInputDataReport>,
    device: HidDevice,
    touch_state: [TouchState; 15],
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err("Device '{path}' is not a FTS3528 Touchscreen".into());
        }

        let touch_state = [
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
            TouchState::default(),
        ];

        Ok(Self {
            device,
            state: Some(PackedInputDataReport::default()),
            touch_state,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; PACKET_SIZE];
        let bytes_read = self.device.read(&mut buf[..])?;

        // All report descriptors are 60 bytes, so this is just to be safe
        if bytes_read != PACKET_SIZE {
            let msg = format!("Invalid input report size was received from touchscreen device: {bytes_read}/{PACKET_SIZE}");
            return Err(msg.into());
        }

        // Handle the incoming input report
        let mut events = self.handle_input_report(buf)?;

        // Handle touch "up" events
        for (index, state) in self.touch_state.iter_mut().enumerate() {
            if state.is_touching && (state.last_touch.elapsed() > Duration::from_millis(4)) {
                log::trace!("Released TOUCH event for contact {index}");
                state.is_touching = false;
                let event = Event::Touch(TouchAxisInput {
                    index: index as u8,
                    is_touching: false,
                    x: 0,
                    y: 0,
                });

                events.push(event);
            }
        }

        Ok(events)
    }

    fn handle_input_report(
        &mut self,
        buf: [u8; 60],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = PackedInputDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::info!("--- Input report ---");
        //log::info!("{input_report}");
        //log::info!("---- End Report ----");

        // Update the state
        let old_state = self.update_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate(old_state);

        Ok(events)
    }

    /// Update internal gamepad state
    fn update_state(
        &mut self,
        input_report: PackedInputDataReport,
    ) -> Option<PackedInputDataReport> {
        // Keep a copy of the old state
        let old_state = self.state;

        // Update the touch state from each "slot" of the report
        self.update_touch_state(&input_report.touch1);
        self.update_touch_state(&input_report.touch2);
        self.update_touch_state(&input_report.touch3);
        self.update_touch_state(&input_report.touch4);

        // Set the current state
        self.state = Some(input_report);

        old_state
    }

    // Update the touch state for the contact id
    fn update_touch_state(&mut self, touch_data: &TouchData) {
        let index = touch_data.contact_id as usize;
        if let Some(touch_state) = self.touch_state.get_mut(index) {
            touch_state.is_touching = true;
            if touch_data.is_touching() {
                touch_state.last_touch = Instant::now();
            }
        }
    }

    /// Translate the state into individual events
    fn translate(&self, old_state: Option<PackedInputDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(old_state) = old_state else {
            return events;
        };

        if old_state.touch1 != state.touch1 {
            log::trace!(
                "Touch1: [{}] {}, {}",
                state.touch1.contact_id,
                state.touch1.get_x(),
                state.touch1.get_y()
            );
            if state.touch1.is_touching() {
                let event = state.touch1.into();
                events.push(Event::Touch(event));
            }
        }
        if old_state.touch2 != state.touch2 {
            log::trace!(
                "Touch2: [{}] {}, {}",
                state.touch2.contact_id,
                state.touch2.get_x(),
                state.touch2.get_y()
            );
            if state.touch2.is_touching() {
                let event = state.touch2.into();
                events.push(Event::Touch(event));
            }
        }
        if old_state.touch3 != state.touch3 {
            log::trace!(
                "Touch3: [{}] {}, {}",
                state.touch3.contact_id,
                state.touch3.get_x(),
                state.touch3.get_y()
            );
            if state.touch3.is_touching() {
                let event = state.touch3.into();
                events.push(Event::Touch(event));
            }
        }
        if old_state.touch4 != state.touch4 {
            log::trace!(
                "Touch4: [{}] {}, {}",
                state.touch4.contact_id,
                state.touch4.get_x(),
                state.touch4.get_y()
            );
            if state.touch4.is_touching() {
                let event = state.touch4.into();
                events.push(Event::Touch(event));
            }
        }

        events
    }
}
