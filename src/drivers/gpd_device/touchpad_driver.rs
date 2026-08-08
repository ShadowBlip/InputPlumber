use std::{error::Error, ffi::CString, time::{Duration, Instant}};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use crate::{udev::device::UdevDevice};

use super::{
    event::{BinaryInput, Event, TouchAxisEvent, TouchButtonEvent, TriggerEvent, TriggerInput},
    hid_report::TouchpadDataReport
};

pub const VID: u16 = 0x093A;
pub const PID: u16 = 0x0255;
pub const IID: i32 = 0x00;

const CLICK_DELAY: Duration = Duration::from_millis(150);
const RELEASE_DELAY: Duration = Duration::from_millis(50);
const MAX_TAP_DISTANCE_SQ: u32 = 10000;

// Report ID
pub const TOUCH_DATA: u8 = 0x01;

// Input report size
const TOUCHPAD_PACKET_SIZE: usize = 30;

// HID buffer read timeout
const HID_TIMEOUT: i32 = 10;

// Axis ranges
pub const TOUCHPAD_X_MAX: f64 = 2559.0;
pub const TOUCHPAD_Y_MAX: f64 = 1535.0;

pub const PAD_FORCE_MAX: f64 = 127.0;
pub const PAD_FORCE_NORMAL: u8 = 32; /* Simulated average */

pub struct TouchpadDriver {
    /// HIDRAW device instance
    device: HidDevice,
    /// Timestamp of the first touch event.
    first_touch: Instant,
    /// X position of the first touch
    first_touch_x: u16,
    /// Y position of the first touch
    first_touch_y: u16,
    /// Whether or not we are currently holding a tap-to-click.
    is_clicked: bool,
    /// Whether or not we are detecting a touch event currently.
    is_touching: bool,
    /// Timestamp of the last touch event.
    last_touch: Instant,
    /// Timestamp of the last release event.
    last_release: Instant,
    /// Whether or not a touch event was started that hasn't been cleared.
    touch_started: bool,
    /// State for the touchpad device
    touchpad_state: Option<TouchpadDataReport>,
}

impl TouchpadDriver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = udevice.devnode();
        let cs_path = CString::new(path.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;
        let info = device.get_device_info()?;
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{path}' is not a GPD Win Mini touchpad").into());
        }

        Ok(Self {
            device,
            first_touch: Instant::now(),
            first_touch_x: 0,
            first_touch_y: 0,
            is_clicked: false,
            is_touching: false,
            last_touch: Instant::now(),
            last_release: Instant::now(),
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
            TOUCH_DATA => {
                log::trace!("Got touch data.");
                if bytes_read != TOUCHPAD_PACKET_SIZE {
                    return Err("Invalid packet size for Touchpad Data.".into());
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

        // Check for release conditions
        if !self.is_touching && self.is_clicked && self.last_touch.elapsed() > RELEASE_DELAY {
            let mut new_events = self.release_click();
            events.append(&mut new_events);
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
        let Some(_) = old_state else {
            return events;
        };

        self.is_touching = state.tip_switch0;

        //// Axis events
        if self.is_touching {
            self.last_touch = Instant::now();

            // If this is the first event of a new touch, log the time.
            if !self.touch_started {
                self.touch_started = true;
                self.first_touch = Instant::now();
                self.first_touch_x = state.touch_x0;
                self.first_touch_y = state.touch_y0;
                log::trace!("Started TOUCH event");

                // If this happened soon after a click, drag
                if self.last_release.elapsed() <= CLICK_DELAY {
                    log::trace!("Started DRAG event");
                    let mut click_events = self.start_click();
                    events.append(&mut click_events);
                }
            }
        // Handle tap-to-click
        } else if !self.is_touching
            && self.touch_started
            && self.first_touch.elapsed() <= CLICK_DELAY
            && self.distance_sq(
                self.first_touch_x,
                self.first_touch_y,
                state.touch_x0,
                state.touch_y0
            ) < MAX_TAP_DISTANCE_SQ
        {
            // Handle double click
            if self.is_clicked {
                log::trace!("Double Click");
                let mut new_events = self.release_click();
                events.append(&mut new_events);
            }
            let mut click_events = self.start_click();
            events.append(&mut click_events);
        // Handle release events
        } else if !self.is_touching {
            // For double clicking, ensure the previous click is cleared.
            if self.is_clicked {
                let mut new_events = self.release_click();
                events.append(&mut new_events);
            }

            // Clear this touch sequence
            if self.touch_started {
                self.touch_started = false;
                log::trace!("END Touch");
            }
        }

        events.push(Event::TouchAxis(TouchAxisEvent {
            index: 0,
            is_touching: self.is_touching,
            x: state.touch_x0,
            y: state.touch_y0,
        }));

        events
    }

    fn distance_sq(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) -> u32 {
        let dx: u32 = u32::from(x1.abs_diff(x2));
        let dy: u32 = u32::from(y1.abs_diff(y2));
        dx * dx + dy * dy
    }

    fn start_click(&mut self) -> Vec<Event> {
        if self.is_clicked {
            log::debug!("Rejecting extra click");
            return vec![];
        }
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
        let event = Event::Trigger(TriggerEvent::PadForce(TriggerInput {
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
        self.touch_started = false;
        self.last_release = Instant::now();
        let mut events = Vec::new();
        let event = Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: false }));
        events.push(event);
        // The touchpad doesn't have a force sensor. The deck target wont produce a "click"
        // event in desktop or lizard mode without a force value. Simulate a 1/4 press to work
        // around this.
        let event = Event::Trigger(TriggerEvent::PadForce(TriggerInput { value: 0 }));
        events.push(event);
        events
    }
}
