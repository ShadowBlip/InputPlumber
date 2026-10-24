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
    PAD_FORCE_NORMAL, RELEASE_CONFIRM, RELEASE_TIMEOUT_DINPUT, RELEASE_TIMEOUT_XINPUT,
    TAP_MAX_DISTANCE_SQ, TIMEOUT, TOUCHPAD_DATA, TOUCHPAD_PACKET_SIZE, TP_IID, VID,
};

pub struct Driver {
    /// HIDRAW device instance
    device: HidDevice,
    /// List of events that should be filtered and not emitted
    filtered_events: Vec<Event>,
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
    /// Whether the active touch continues a previous click.
    is_continuation: bool,
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
            filtered_events: Default::default(),
            first_touch: Instant::now(),
            first_touch_pos: Default::default(),
            is_clicked: false,
            is_touching: false,
            last_touch: Instant::now(),
            touch_started: false,
            is_continuation: false,
            touchpad_state: None,
            is_dinput,
            release_delay,
            drag_timeout,
        })
    }

    pub fn update_filtered_events(&mut self, events: Vec<Event>) {
        self.filtered_events = events;
    }

    fn is_filtered(&self, event: &Event) -> bool {
        self.filtered_events.iter().any(|f| f.variant_eq(event))
    }

    fn emit(&mut self, events: &mut Vec<Event>, event: Event) {
        if !self.is_filtered(&event) {
            events.push(event);
        }
    }

    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0; TOUCHPAD_PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], TIMEOUT)?;

        if bytes_read == 0 {
            // In XInput mode data only flows when the touchpad is touched. Confirm a lift
            // only after the pad has been silent long enough to bypass the device's
            // phantom click.
            if (self.touch_started || self.is_clicked)
                && (self.last_touch.elapsed() >= RELEASE_CONFIRM)
            {
                return Ok(self.translate_touch(false));
            }
            return Ok(Vec::new());
        }

        let report_id = buf[0];
        if report_id != TOUCHPAD_DATA {
            if (self.touch_started || self.is_clicked)
                && (self.last_touch.elapsed() >= RELEASE_CONFIRM)
            {
                return Ok(self.translate_touch(false));
            }
            return Ok(Vec::new());
        }

        if bytes_read != TOUCHPAD_PACKET_SIZE {
            return Err("Invalid packet size for Keyboard or Touchpad Data.".into());
        }

        let sized_buf: &[u8; TOUCHPAD_PACKET_SIZE] = (&buf[..bytes_read]).try_into()?;
        let input_report = TouchpadDataReport::unpack(sized_buf)?;

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
        let Some(state) = self.touchpad_state else {
            return events;
        };

        // Determine if we are touching. Dinput uses tip switch and sends additional reports. Xinput
        // stops sending reports before tip switch is cleared, so treat new data as touching.
        self.is_touching = if self.is_dinput {
            state.tip_switch_0 && state.contact_count == 1
        } else {
            new_data && state.tip_switch_0 && state.contact_count == 1
        };

        if self.is_touching {
            if !self.touch_started {
                log::trace!("START Touch");
                log::trace!("Last touch elapsed: {:?}", self.last_touch.elapsed());
                self.touch_started = true;
                self.first_touch = Instant::now();
                self.first_touch_pos = [state.touch_x_0, state.touch_y_0];
                // A second finger while a click is held ends that click so it stays short.
                if self.is_clicked {
                    self.is_continuation = true;
                    self.release_click(&mut events);
                }
            }
            // Once the continuation moves it is a drag, so press now.
            if self.is_continuation && !self.is_clicked && self.is_valid_move(state) {
                self.start_click(&mut events);
            }
            self.last_touch = Instant::now();
            self.emit(&mut events, self.touch_event(state));
            return events;
        }

        if self.touch_started {
            self.touch_started = false;
            // Close this touch at its last position so the target opens a fresh
            // tracking id for the next touch.
            self.emit(&mut events, self.touch_event(state));

            // A continuation that moved is a drag to release; one that stayed put is another click.
            if self.is_continuation {
                self.is_continuation = false;
                if self.is_clicked {
                    self.release_click(&mut events);
                } else {
                    self.start_click(&mut events);
                }
                return events;
            }

            let moved = self.is_valid_move(state);
            let elapsed = self.last_touch.duration_since(self.first_touch);
            let held = elapsed >= self.release_delay;

            let is_tap = !moved && !held;
            log::trace!(
                "release decision: moved={moved} held={held} elapsed={elapsed:?} (max={:?}) -> {}",
                self.release_delay,
                if is_tap { "tap" } else { "swipe" }
            );
            if is_tap {
                self.start_click(&mut events);
                return events;
            }

            log::trace!("END Touch");
            return events;
        }

        if self.is_clicked {
            if self.last_touch.elapsed() < self.drag_timeout {
                self.emit(&mut events, self.touch_event(state));
            } else {
                self.release_click(&mut events);
            }
        }

        events
    }

    /// Start a click event
    fn start_click(&mut self, events: &mut Vec<Event>) {
        if self.is_clicked {
            return;
        }
        log::trace!("START Click");
        self.is_clicked = true;
        self.emit(
            events,
            Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: true })),
        );
        self.emit(
            events,
            Event::Trigger(TriggerEvent::RpadForce(TriggerInput {
                value: PAD_FORCE_NORMAL,
            })),
        );
    }

    /// End a click event
    fn release_click(&mut self, events: &mut Vec<Event>) {
        log::trace!("END Click");
        self.is_clicked = false;
        self.emit(
            events,
            Event::TouchButton(TouchButtonEvent::Left(BinaryInput { pressed: false })),
        );
        self.emit(
            events,
            Event::Trigger(TriggerEvent::RpadForce(TriggerInput { value: 0 })),
        );
    }

    /// Whether the touch has moved far enough outside the deadzone for natural
    /// jitter to count as a move.
    fn is_valid_move(&self, state: TouchpadDataReport) -> bool {
        let dx = (state.touch_x_0 as i64) - (self.first_touch_pos[0] as i64);
        let dy = (state.touch_y_0 as i64) - (self.first_touch_pos[1] as i64);
        let dist_sq = dx * dx + dy * dy;
        let valid = dist_sq > TAP_MAX_DISTANCE_SQ;
        log::trace!(
            "is_valid_move: dx={dx} dy={dy} dist_sq={dist_sq} (max={})",
            TAP_MAX_DISTANCE_SQ
        );
        valid
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
