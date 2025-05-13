use std::{error::Error, ffi::CString};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use super::{
    event::{
        AxisEvent, BinaryInput, ButtonEvent, Event, TouchAxisInput, TriggerEvent, TriggerInput,
    },
    hid_report::TouchpadDataReport,
    HID_TIMEOUT, PAD_FORCE_NORMAL, PIDS, TOUCH_PACKET_SIZE, TOUCH_REPORT_ID, TP_IID, VID,
};

pub struct TouchpadDriver {
    /// HIDRAW device instance
    device: HidDevice,
    /// State for the touchpad device
    touchpad_state: Option<TouchpadDataReport>,
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
            || info.interface_number() != TP_IID
        {
            return Err(format!("Device '{fmtpath}' is not a Legion Go S Controller").into());
        }
        Ok(Self {
            device,
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
        let old_state = self.update_touchpad_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_touch(old_state);

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
            })));
            // The touchpad doesn't have a force sensor. TouchAxisInputhe deck target wont produce a "click"
            // event in desktop or lizard mode without a force value. Simulate a 1/4 press to work
            // around this.
            events.push(Event::Trigger(TriggerEvent::RpadForce(TriggerInput {
                value: PAD_FORCE_NORMAL * state.pressed as u8,
            })));
        }

        // Axis events
        if state.touch_x != old_state.touch_x
            || state.touch_y != old_state.touch_y
            || state.tip_switch != old_state.tip_switch
        {
            events.push(Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
                index: 0,
                is_touching: state.tip_switch,
                x: state.touch_x,
                y: state.touch_y,
            })));
        }
        events
    }
}
