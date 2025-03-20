use std::{
    error::Error,
    ffi::CString,
    time::{Duration, Instant},
};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use super::{
    event::{AxisEvent, BinaryInput, ButtonEvent, Event, TouchAxisInput},
    hid_report::TouchpadDataReport,
};

// Hardware ID's
pub const VID: u16 = 0x1a86;
pub const XINPUT_PID: u16 = 0xe310;
pub const DINPUT_PID: u16 = 0xe311;
pub const PIDS: [u16; 2] = [XINPUT_PID, DINPUT_PID];
pub const IID: i32 = 0x2;

// Input report sizes
const TOUCH_PACKET_SIZE: usize = 10;
const HID_TIMEOUT: i32 = 10;

const TOUCH_REPORT_ID: u8 = 0x31;

// Input report axis ranges
pub const PAD_X_MAX: f64 = 400.0;
pub const PAD_Y_MAX: f64 = 400.0;

pub struct TouchpadDriver {
    /// HIDRAW device instance
    device: HidDevice,
    /// State for the touchpad device
    touchpad_state: Option<TouchpadDataReport>,
    /// Timestamp of the last touch event.
    last_touch: Instant,
    /// Whether or not we are detecting a touch event currently.
    is_touching: bool,
}

impl TouchpadDriver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fmtpath = path.clone();
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;

        if info.vendor_id() != VID
            || !PIDS.contains(&info.product_id())
            || info.interface_number() != IID
        {
            return Err(format!("Device '{fmtpath}' is not a Legion Go S Controller").into());
        }
        Ok(Self {
            device,
            is_touching: false,
            last_touch: Instant::now(),
            touchpad_state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut events = vec![];
        let mut buf = [0; TOUCH_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;
        let report_id = buf[0];

        if bytes_read == TOUCH_PACKET_SIZE && report_id == TOUCH_REPORT_ID {
            events = match self.handle_touch_report(buf) {
                Ok(events) => events,
                Err(e) => {
                    log::error!("Got error processing TouchInputDataReport: {e:?}");
                    vec![]
                }
            };
        }

        // There is no release event, so check to see if we are still touching.
        if self.is_touching && (self.last_touch.elapsed() > Duration::from_millis(25)) {
            let event: Event = self.release_touch();
            events.push(event);
        }

        Ok(events)
    }

    /* Touchpad */
    /// Unpacks the buffer into a [TouchInputDataReport] structure and updates
    /// the internal touch_state.
    fn handle_touch_report(
        &mut self,
        buf: [u8; TOUCH_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = TouchpadDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::debug!("--- Input report ---");
        //log::debug!("{input_report}");
        //log::debug!(" ---- End Report ----");

        // Update the state
        let old_dinput_state = self.update_touchpad_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_touch(old_dinput_state);

        Ok(events)
    }

    /// Update touchinput state
    fn update_touchpad_state(
        &mut self,
        input_report: TouchpadDataReport,
    ) -> Option<TouchpadDataReport> {
        let old_state = self.touchpad_state;
        self.touchpad_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_touch(&mut self, old_state: Option<TouchpadDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.touchpad_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(old_state) = old_state else {
            return events;
        };

        // Binary events
        if state.pressed != old_state.pressed {
            events.push(Event::Button(ButtonEvent::RPadPress(BinaryInput {
                pressed: state.pressed,
            })))
        }

        // Axis events
        if state.touch_x != old_state.touch_x || state.touch_y != old_state.touch_y {
            if !self.is_touching {
                self.is_touching = true;
            }

            self.last_touch = Instant::now();
            events.push(Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
                index: 0,
                is_touching: true,
                x: state.touch_x,
                y: state.touch_y,
            })));
        }
        events
    }

    fn release_touch(&mut self) -> Event {
        self.is_touching = false;
        Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
            index: 0,
            is_touching: false,
            x: 0,
            y: 0,
        }))
    }
}
