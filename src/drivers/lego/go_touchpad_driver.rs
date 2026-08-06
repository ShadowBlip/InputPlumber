use std::{
    error::Error,
    ffi::CString,
    time::{Duration, Instant},
};

use hidapi::HidDevice;
use packed_struct::PackedStruct;

use super::{
    event::{
        AxisEvent, BinaryInput, Event, TouchAxisInput, TouchButtonEvent, TriggerEvent, TriggerInput,
    },
    hid_report::TouchpadDataReport,
    DRAG_TIMEOUT_DINPUT, DRAG_TIMEOUT_XINPUT, GO_TOUCHPAD_D_PIDS, GO_TOUCHPAD_X_PIDS,
    PAD_FORCE_NORMAL, RELEASE_TIMEOUT_DINPUT, RELEASE_TIMEOUT_XINPUT, TAP_MAX_DISTANCE_SQ,
    TOUCHPAD_DATA, TOUCHPAD_PACKET_SIZE, TP_IID, TP_TIMEOUT, VID,
};

pub struct Driver {
    /// HIDRAW device instance
    device: HidDevice,
    /// Timestamp of the first touch event.
    first_touch: Instant,
    /// [x, y] of the first sample of the current touch, used for tap displacement gating.
    first_touch_pos: [u16; 2],
    /// Whether or not we are currently holding a tap-to-click.
    is_clicked: bool,
    /// Whether or not we are detecting a touch event currently.
    is_touching: bool,
    /// Timestamp of the last touch event.
    last_touch: Instant,
    /// Whether or not a touch event was started that hasn't been cleared.
    touch_started: bool,
    /// State for the touchpad device
    touchpad_state: Option<TouchpadDataReport>,
    /// Determine if this is dinput or xinput, used for touch detection.
    is_dinput: bool,
    /// Configure the release delay for detecting tap to click, determined by poll rate.
    release_delay: Duration,
    /// Configures the tap to drag timeout for an idle pad.
    drag_timeout: Duration,
}

impl Driver {
    pub fn new(path: String) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let fmtpath = path.clone();
        let path = CString::new(path)?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&path)?;
        let info = device.get_device_info()?;

        let pid = &info.product_id();

        if info.vendor_id() != VID
            || (!GO_TOUCHPAD_D_PIDS.contains(pid) && !GO_TOUCHPAD_X_PIDS.contains(pid))
            || info.interface_number() != TP_IID
        {
            return Err(format!("Device '{fmtpath}' is not a Legion Go Touchpad").into());
        }

        let is_dinput = GO_TOUCHPAD_D_PIDS.contains(pid);
        let (release_delay, drag_timeout) = if is_dinput {
            (RELEASE_TIMEOUT_DINPUT, DRAG_TIMEOUT_DINPUT)
        } else {
            (RELEASE_TIMEOUT_XINPUT, DRAG_TIMEOUT_XINPUT)
        };

        Ok(Self {
            device,
            first_touch: Instant::now(),
            first_touch_pos: Default::default(),
            is_clicked: false,
            is_touching: false,
            last_touch: Instant::now(),
            touch_started: false,
            touchpad_state: None,
            is_dinput,
            release_delay,
            drag_timeout,
        })
    }

    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0; TOUCHPAD_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], TP_TIMEOUT)?;

        if bytes_read == 0 {
            // In XInput mode data only flows when the touchpad is touched.
            // Flush state transitions if a touch session was active.
            if self.touch_started || self.is_clicked {
                return Ok(self.translate_touch(false));
            }
            return Ok(Vec::new());
        }

        let report_id = buf[0];
        if report_id != TOUCHPAD_DATA {
            if self.touch_started || self.is_clicked {
                return Ok(self.translate_touch(false));
            }
            return Ok(Vec::new());
        }

        if bytes_read != TOUCHPAD_PACKET_SIZE {
            return Err("Invalid packet size for Keyboard or Touchpad Data.".into());
        }

        let sized_buf: &[u8; TOUCHPAD_PACKET_SIZE] = (&buf[..bytes_read]).try_into()?;
        let input_report = TouchpadDataReport::unpack(sized_buf)?;

        // Print input report for tracing
        log::trace!("--- Input report ---");
        log::trace!("{input_report}");
        log::trace!("---- End Report ----");

        // Dinput mode runs at the same poll rate but duplicates each input frame once. This
        // effectively halves the real poll rate. Since we use scan time deltas for
        // is_pressed detection every poll, reject the duplicate reports.
        if let Some(prev) = self.touchpad_state {
            if prev.scan_time == input_report.scan_time {
                if self.is_touching {
                    self.last_touch = Instant::now();
                }
                return Ok(Vec::new());
            }
        }

        self.touchpad_state = Some(input_report);
        Ok(self.translate_touch(true))
    }

    /// Translate the state into individual events
    fn translate_touch(&mut self, new_data: bool) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(mut state) = self.touchpad_state else {
            return events;
        };

        // Determine if we are touching. Dinput uses tip switch and sends additional reports. Xinput
        // stops sending reports before tip switch is cleared, so treat new data as touching.
        self.is_touching = if self.is_dinput {
            state.tip_switch_0
        } else {
            new_data
        };

        if self.is_touching {
            if !self.touch_started {
                log::trace!("START Touch");
                log::trace!("Last touch elapsed: {:?}", self.last_touch.elapsed());
                self.touch_started = true;
                self.first_touch = Instant::now();
                self.first_touch_pos = [state.touch_x_0, state.touch_y_0];
            }
            self.last_touch = Instant::now();
            events.push(self.touch_event(state));
        } else if self.is_clicked {
            if self.last_touch.elapsed() < self.drag_timeout {
                events.push(self.touch_event(state));
                return events;
            }
            self.touch_started = false;
            let mut new_events = self.release_click();
            events.append(&mut new_events);
        } else if self.touch_started {
            let dx = (state.touch_x_0 as i64) - (self.first_touch_pos[0] as i64);
            let dy = (state.touch_y_0 as i64) - (self.first_touch_pos[1] as i64);
            let dist_sq = dx * dx + dy * dy;

            if self.first_touch.elapsed() < self.release_delay && dist_sq < TAP_MAX_DISTANCE_SQ {
                let mut click_events = self.start_click();
                events.append(&mut click_events);
                return events;
            }

            // Explicitly send release event with zeroed coordinates and is_touching = false
            self.touch_started = false;
            state.touch_x_0 = 0;
            state.touch_y_0 = 0;
            self.touchpad_state = Some(state);
            events.push(self.touch_event(state));
            log::trace!("END Touch");
        }

        events
    }

    /// Start a click event
    fn start_click(&mut self) -> Vec<Event> {
        if self.is_clicked {
            return vec![];
        }
        log::trace!("START Click");
        self.is_clicked = true;
        vec![
            Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: true })),
            Event::Trigger(TriggerEvent::RpadForce(TriggerInput {
                value: PAD_FORCE_NORMAL,
            })),
        ]
    }

    /// End a click event
    fn release_click(&mut self) -> Vec<Event> {
        log::trace!("END Click");
        self.is_clicked = false;
        vec![
            Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: false })),
            Event::Trigger(TriggerEvent::RpadForce(TriggerInput { value: 0 })),
        ]
    }

    /// Generate a touch event from the device state
    fn touch_event(&self, state: TouchpadDataReport) -> Event {
        Event::Axis(AxisEvent::Touchpad(TouchAxisInput {
            index: 0,
            is_touching: self.is_touching,
            x: state.touch_x_0,
            y: state.touch_y_0,
        }))
    }
}
