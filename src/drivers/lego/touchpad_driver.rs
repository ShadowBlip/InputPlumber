use std::{
    error::Error,
    ffi::CString,
    time::{Duration, Instant},
};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use crate::drivers::lego::{
    event::{TriggerEvent, TriggerInput},
    CLICK_DELAY, PAD_FORCE_NORMAL,
};

use super::{
    event::{AxisEvent, BinaryInput, Event, TouchAxisInput, TouchButtonEvent},
    hid_report::TouchpadDataReport,
    HID_TIMEOUT, PIDS, TOUCHPAD_DATA, TOUCHPAD_PACKET_SIZE, TP_IID, VID,
};

pub struct Driver {
    /// HIDRAW device instance
    device: HidDevice,
    /// Timestamp of the first touch event.
    first_touch: Instant,
    /// Whether or not we are currently holding a click-to-click.
    is_clicked: bool,
    /// Whether or not we are detecting a touch event currently.
    is_touching: bool,
    /// Timestamp of the last touch event.
    last_touch: Instant,
    /// Whether or not a touch event was started that hasn't been cleared.
    touch_started: bool,
    /// State for the touchpad device
    touchpad_state: Option<TouchpadDataReport>,
}

impl Driver {
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
            first_touch: Instant::now(),
            is_clicked: false,
            is_touching: false,
            last_touch: Instant::now(),
            touch_started: false,
            touchpad_state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; TOUCHPAD_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;

        let report_id = buf[0];
        let slice = &buf[..bytes_read];

        let mut events = match report_id {
            TOUCHPAD_DATA => {
                if bytes_read != TOUCHPAD_PACKET_SIZE {
                    return Err("Invalid packet size for Keyboard or Touchpad Data.".into());
                }
                // Handle the incoming input report
                let sized_buf = slice.try_into()?;

                self.handle_touchinput_report(sized_buf)?
            }
            _ => {
                let events = vec![];
                events
            }
        };

        // There is no release event, so check to see if we are still touching.
        if self.is_touching {
            let Some(state) = self.touchpad_state else {
                return Ok(events);
            };

            // Ignore release when virtual click is held.
            if state.touch_x_1 != 0 && state.touch_y_1 != 0 {
                self.last_touch = Instant::now();
                return Ok(events);
            }

            if self.last_touch.elapsed() >= Duration::from_millis(4) {
                self.is_touching = false;
            }
            return Ok(events);
        }

        // Check for quick click conditions
        if self.touch_started && self.first_touch.elapsed() <= CLICK_DELAY * 2 {
            // For double clicking, ensure the previous click is cleared.
            if self.is_clicked {
                let mut new_events = self.release_click();
                events.append(&mut new_events);
            }

            let mut new_events = self.start_click();
            events.append(&mut new_events);

            return Ok(events);
        }

        // Check for release conditions
        if self.touch_started && self.last_touch.elapsed() > CLICK_DELAY / 2 {
            let event: Event = self.release_touch();
            events.push(event);

            // If we did a click event, see if we should release it. Accounts for click and drag.
            if self.is_clicked {
                let mut new_events = self.release_click();
                events.append(&mut new_events);
                return Ok(events);
            }
        }

        Ok(events)
    }

    /// Unpacks the buffer into a [TouchpadDataReport] structure and updates
    /// the internal touchpad_state
    fn handle_touchinput_report(
        &mut self,
        buf: [u8; TOUCHPAD_PACKET_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = TouchpadDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::trace!("--- Input report ---");
        //log::trace!("{input_report}");
        //log::trace!("---- End Report ----");

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
        let mut events: Vec<Event> = Vec::new();
        let Some(mut state) = self.touchpad_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        let Some(old_state) = old_state else {
            return events;
        };

        if !self.is_touching {
            // Check for click events
            if self.touch_started && self.last_touch.elapsed() <= CLICK_DELAY / 3 {
                let mut new_events = self.start_click();
                events.append(&mut new_events);
            }
            if !self.touch_started {
                self.touch_started = true;
                self.first_touch = Instant::now();
                log::trace!("Started TOUCH event");
            }
            self.is_touching = true;
        }

        // There is a "hold to click" event built into the firmware, ignore this event.
        if state.touch_x_1 != 0 && state.touch_y_1 != 0 {
            state.touch_x_0 = old_state.touch_x_0;
            state.touch_y_0 = old_state.touch_y_0;
            self.touchpad_state = Some(state);
        }

        events.push(Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
            index: 0,
            is_touching: true,
            x: state.touch_x_0,
            y: state.touch_y_0,
        })));

        self.last_touch = Instant::now();
        events
    }

    fn release_touch(&mut self) -> Event {
        log::trace!("Released TOUCH event.");
        self.touch_started = false;
        Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
            index: 0,
            is_touching: false,
            x: 0,
            y: 0,
        }))
    }

    fn start_click(&mut self) -> Vec<Event> {
        log::trace!("Started CLICK event.");
        log::trace!("First touch elapsed: {:?}", self.first_touch.elapsed());
        log::trace!("Last touch elapsed: {:?}", self.last_touch.elapsed());
        self.is_clicked = true;
        let mut events = Vec::new();

        let event = Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: true }));
        events.push(event);
        // The touchpad doesn't have a force sensor. The deck target wont produce a "click"
        // event in desktop or lizard mode without a force value. Simulate a 1/4 press to work
        // around this.
        let event = Event::Trigger(TriggerEvent::RpadForce(TriggerInput {
            value: PAD_FORCE_NORMAL,
        }));
        events.push(event);
        events
    }

    fn release_click(&mut self) -> Vec<Event> {
        log::trace!("Released CLICK event.");
        log::trace!("First touch elapsed: {:?}", self.first_touch.elapsed());
        log::trace!("Last touch elapsed: {:?}", self.last_touch.elapsed());
        self.is_clicked = false;
        let mut events = Vec::new();
        let event = Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: false }));
        events.push(event);
        // The touchpad doesn't have a force sensor. The deck target wont produce a "click"
        // event in desktop or lizard mode without a force value. Simulate a 1/4 press to work
        // around this.
        let event = Event::Trigger(TriggerEvent::RpadForce(TriggerInput { value: 0 }));
        events.push(event);
        events
    }
}
