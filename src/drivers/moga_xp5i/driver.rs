use std::{error::Error, ffi::CString};

use hidapi::HidDevice;

use crate::udev::device::UdevDevice;

use super::{
    event::{
        BinaryInput, ButtonEvent, Event, JoystickEvent, JoystickInput, TriggerEvent, TriggerInput,
    },
    hid_report::MogaReport,
    HID_TIMEOUT, PACKET_SIZE, PID, VID,
};

/// Driver for the PowerA MOGA XP5-i Plus.
///
/// The controller is an Apple MFi gamepad that presents a standard Bluetooth
/// HID interface with Apple's MFi report descriptor. The kernel's hid-input
/// mapping gets two things wrong for this descriptor: the analog triggers are
/// declared as Button usages (so they become deadzoneless digital buttons),
/// and the d-pad is encoded as four separate 1-bit Generic Desktop usages
/// (so it produces no events). This driver reads the raw reports instead.
pub struct Driver {
    /// HIDRAW device instance
    device: HidDevice,
    /// State for the device
    state: Option<MogaReport>,
    filtered_events: Vec<Event>,
}

impl Driver {
    pub fn new(udevice: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = udevice.devnode();

        let cs_path = CString::new(path.clone())?;
        let api = hidapi::HidApi::new()?;
        let device = api.open_path(&cs_path)?;

        let info = device.get_device_info()?;
        if info.vendor_id() != VID || info.product_id() != PID {
            return Err(format!("Device '{path}' is not a MOGA XP5-i Plus").into());
        }

        Ok(Self {
            device,
            state: None,
            filtered_events: vec![],
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0u8; PACKET_SIZE];
        let bytes_read = self.device.read_timeout(&mut buf[..], HID_TIMEOUT)?;
        if bytes_read == 0 {
            return Ok(vec![]);
        }
        if bytes_read != PACKET_SIZE {
            log::warn!("Unexpected MOGA XP5-i report size {bytes_read}, expected {PACKET_SIZE}");
            return Ok(vec![]);
        }

        let Some(report) = MogaReport::from_bytes(&buf) else {
            return Ok(vec![]);
        };

        let old_state = self.state.replace(report);
        Ok(self.translate_events(old_state))
    }

    /// Update the list of events that should be filtered out
    pub fn update_filtered_events(&mut self, events: Vec<Event>) {
        self.filtered_events = events;
    }

    /// Whether the given event type is filtered out by the active profile.
    fn is_filtered(&self, event: &Event) -> bool {
        self.filtered_events.iter().any(|f| f.variant_eq(event))
    }

    /// Queue the event for the composite device unless it is filtered out.
    fn emit(&mut self, events: &mut Vec<Event>, event: Event) {
        if !self.is_filtered(&event) {
            events.push(event);
        }
    }

    /// Emit a button event when the button state changed.
    fn emit_button<F>(&mut self, events: &mut Vec<Event>, old: bool, new: bool, make: F)
    where
        F: FnOnce(BinaryInput) -> ButtonEvent,
    {
        if old != new {
            self.emit(events, Event::Button(make(BinaryInput { pressed: new })));
        }
    }

    /// Translate the new state into individual events
    fn translate_events(&mut self, old_state: Option<MogaReport>) -> Vec<Event> {
        let Some(state) = self.state else {
            return vec![];
        };
        let Some(old) = old_state else {
            return vec![];
        };

        let mut events = Vec::new();

        // Buttons
        self.emit_button(&mut events, old.a, state.a, ButtonEvent::A);
        self.emit_button(&mut events, old.b, state.b, ButtonEvent::B);
        self.emit_button(&mut events, old.x, state.x, ButtonEvent::X);
        self.emit_button(&mut events, old.y, state.y, ButtonEvent::Y);
        self.emit_button(&mut events, old.lb, state.lb, ButtonEvent::LB);
        self.emit_button(&mut events, old.rb, state.rb, ButtonEvent::RB);
        self.emit_button(&mut events, old.select, state.select, ButtonEvent::View);
        self.emit_button(&mut events, old.start, state.start, ButtonEvent::Menu);
        self.emit_button(&mut events, old.guide, state.guide, ButtonEvent::Guide);
        self.emit_button(&mut events, old.l3, state.l3, ButtonEvent::LSClick);
        self.emit_button(&mut events, old.r3, state.r3, ButtonEvent::RSClick);
        self.emit_button(&mut events, old.dpad_up, state.dpad_up, ButtonEvent::DPadUp);
        self.emit_button(
            &mut events,
            old.dpad_down,
            state.dpad_down,
            ButtonEvent::DPadDown,
        );
        self.emit_button(
            &mut events,
            old.dpad_left,
            state.dpad_left,
            ButtonEvent::DPadLeft,
        );
        self.emit_button(
            &mut events,
            old.dpad_right,
            state.dpad_right,
            ButtonEvent::DPadRight,
        );

        // Axis events
        if state.left_stick_x != old.left_stick_x || state.left_stick_y != old.left_stick_y {
            self.emit(
                &mut events,
                Event::Joystick(JoystickEvent::LStick(JoystickInput {
                    x: state.left_stick_x,
                    y: state.left_stick_y,
                })),
            );
        }
        if state.right_stick_x != old.right_stick_x || state.right_stick_y != old.right_stick_y {
            self.emit(
                &mut events,
                Event::Joystick(JoystickEvent::RStick(JoystickInput {
                    x: state.right_stick_x,
                    y: state.right_stick_y,
                })),
            );
        }
        if state.left_trigger != old.left_trigger {
            self.emit(
                &mut events,
                Event::Trigger(TriggerEvent::LTAnalog(TriggerInput {
                    value: state.left_trigger,
                })),
            );
        }
        if state.right_trigger != old.right_trigger {
            self.emit(
                &mut events,
                Event::Trigger(TriggerEvent::RTAnalog(TriggerInput {
                    value: state.right_trigger,
                })),
            );
        }

        log::trace!("Got events: {events:?}");

        events
    }
}
