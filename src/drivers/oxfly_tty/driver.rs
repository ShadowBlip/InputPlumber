use std::{
    collections::HashSet,
    error::Error,
    ffi::CString,
    io::{Read, Write},
    time::Duration,
};

use clap::builder;
use hidapi::HidDevice;
use packed_struct::PackedStruct;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits, TTYPort};
use tokio::time::Instant;

use crate::drivers::oxfly_tty::{
    event::{AxisEvent, BinaryInput, Event, GamepadButtonEvent, JoyAxisInput, TriggerInput},
    serial_report::{ButtonDataReport, ButtonId, ButtonStatus, JoystickDataReport},
    BUTTON_DATA_REPORT, GAMPAD_DATA_SIZE, INPUT_REPORT_SIZE, JOYSTICK_DATA_REPORT,
    TAKEOVER_COMMAND, TAKEOVER_DATA_REPORT,
};
use crate::input::capability::Source;
use crate::udev::device::UdevDevice;

pub struct Driver {
    udev_device: UdevDevice,
    // TTY Port that events will flow to/from
    port: TTYPort,
    // If the device needs to return to front-end mode or not
    takeover_needed: bool,
    // Timestamp of the last unkown data report.
    last_data: Instant,
    /// State for the internal gamepad controller joysticks
    joystick_state: Option<JoystickDataReport>,
}

impl Driver {
    pub fn new(udev_device: UdevDevice) -> Result<Self, Box<dyn Error + Send + Sync>> {
        log::debug!("{} {}", udev_device.syspath(), udev_device.devnode());

        let port = serialport::new(udev_device.devnode(), 115_200)
            .data_bits(DataBits::Eight)
            .parity(Parity::Even)
            .stop_bits(StopBits::One)
            .timeout(Duration::from_millis(1));
        let mut port = TTYPort::open(&port)?;
        log::debug!("Started OneXFly TTY Driver.");
        Ok(Self {
            udev_device,
            port,
            takeover_needed: true,
            last_data: Instant::now(),
            joystick_state: None,
        })
    }

    /// Poll the device and read input reports
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        // Read data from the device into a buffer
        let mut buf = [0; INPUT_REPORT_SIZE];

        if self.takeover_needed && self.last_data.elapsed() > Duration::from_millis(2) {
            log::debug!("Take over the device again.");
            self.port.write_all(&TAKEOVER_COMMAND)?;
            self.port.flush()?;
            self.takeover_needed = false;
        }

        let events = match self.port.read_exact(&mut buf) {
            Ok(_) => {
                //log::trace!("Got bytes: {:02x?}", &buf);
                let report_id = buf[1];
                let events = match report_id {
                    BUTTON_DATA_REPORT => {
                        let slice = &buf[..GAMPAD_DATA_SIZE];
                        let sized_buf = slice.try_into()?;
                        self.handle_button_report(sized_buf)?
                    }
                    JOYSTICK_DATA_REPORT => {
                        let slice = &buf[..GAMPAD_DATA_SIZE];
                        let sized_buf = slice.try_into()?;
                        self.handle_joystick_report(sized_buf)?
                    }
                    TAKEOVER_DATA_REPORT => Vec::new(), // IGNORE ME
                    _ => {
                        // Mouse mode sends packets of a different size, so the data is jumbled.
                        // We don't care about them, but when it exits the device will return to
                        // normal gamepad mode. Flag the state change so we can automatically send
                        // the mode switch command again.
                        if !self.takeover_needed {
                            log::debug!("Unkown data report: {:02x?}", report_id);
                            self.takeover_needed = true;
                        }
                        self.last_data = Instant::now();
                        Vec::new()
                    }
                };
                events
            }
            Err(e) => {
                //log::debug!("Got error: {e}");
                vec![]
            }
        };

        Ok(events)
    }

    /// Unpacks the buffer into a [ButtonDataReport] structure and updates
    /// the internal button_state
    fn handle_button_report(
        &mut self,
        buf: [u8; GAMPAD_DATA_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = ButtonDataReport::unpack(&buf)?;

        // Print input report for debugging
        //log::debug!("--- ButtonDataReport ---");
        //log::debug!("{input_report}");
        //log::debug!(" ---- ButtonDataReport ----");

        //// Translate the input report into an event
        let event = self.translate_button(input_report);

        Ok(vec![event])
    }

    fn translate_button(&mut self, state: ButtonDataReport) -> Event {
        match state.button_id {
            ButtonId::None | ButtonId::Guide => Event::GamepadButton(GamepadButtonEvent::None), // Dont actually exist.
            ButtonId::A => Event::GamepadButton(GamepadButtonEvent::A(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::B => Event::GamepadButton(GamepadButtonEvent::B(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::X => Event::GamepadButton(GamepadButtonEvent::X(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::Y => Event::GamepadButton(GamepadButtonEvent::Y(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::LeftBumper => Event::GamepadButton(GamepadButtonEvent::LB(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::RightBumper => Event::GamepadButton(GamepadButtonEvent::RB(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::LeftTrigger => {
                Event::GamepadButton(GamepadButtonEvent::TriggerL(BinaryInput {
                    pressed: state.status == ButtonStatus::Pressed,
                }))
            }
            ButtonId::RightTrigger => {
                Event::GamepadButton(GamepadButtonEvent::TriggerR(BinaryInput {
                    pressed: state.status == ButtonStatus::Pressed,
                }))
            }
            ButtonId::Menu => Event::GamepadButton(GamepadButtonEvent::Menu(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::View => Event::GamepadButton(GamepadButtonEvent::View(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::LeftStick => Event::GamepadButton(GamepadButtonEvent::ThumbL(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::RightStick => Event::GamepadButton(GamepadButtonEvent::ThumbR(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::DpadUp => Event::GamepadButton(GamepadButtonEvent::DPadUp(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::DpadDown => Event::GamepadButton(GamepadButtonEvent::DPadDown(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::DpadLeft => Event::GamepadButton(GamepadButtonEvent::DPadLeft(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::DpadRight => {
                Event::GamepadButton(GamepadButtonEvent::DPadRight(BinaryInput {
                    pressed: state.status == ButtonStatus::Pressed,
                }))
            }
            ButtonId::M1 => Event::GamepadButton(GamepadButtonEvent::M1(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
            ButtonId::M2 => Event::GamepadButton(GamepadButtonEvent::M2(BinaryInput {
                pressed: state.status == ButtonStatus::Pressed,
            })),
        }
    }
    /// Unpacks the buffer into a [JoystickDataReport] structure and updates
    /// the internal button_state
    fn handle_joystick_report(
        &mut self,
        buf: [u8; GAMPAD_DATA_SIZE],
    ) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let input_report = JoystickDataReport::unpack(&buf)?;

        //// Print input report for debugging
        //log::debug!("--- JoystickDataReport ---");
        //log::debug!("{input_report}");
        //log::debug!(" ---- JoystickDataReport ----");

        // Update the state
        let old_state = self.update_joystick_state(input_report);

        // Translate the state into a stream of input events
        let events = self.translate_joystick(old_state);

        Ok(events)
    }

    /// Update joystick state
    fn update_joystick_state(
        &mut self,
        input_report: JoystickDataReport,
    ) -> Option<JoystickDataReport> {
        let old_state = self.joystick_state;
        self.joystick_state = Some(input_report);
        old_state
    }

    /// Translate the state into individual events
    fn translate_joystick(&mut self, old_state: Option<JoystickDataReport>) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(state) = self.joystick_state else {
            return events;
        };

        // Translate state changes into events if they have changed
        if let Some(old_state) = old_state {
            if state.left_trigger != old_state.left_trigger {
                events.push(Event::Axis(AxisEvent::TriggerL(TriggerInput {
                    value: state.left_trigger,
                })));
            }
            if state.right_trigger != old_state.right_trigger {
                events.push(Event::Axis(AxisEvent::TriggerR(TriggerInput {
                    value: state.right_trigger,
                })));
            }
            if state.left_stick_x != old_state.left_stick_x
                || state.left_stick_y != old_state.left_stick_y
            {
                events.push(Event::Axis(AxisEvent::LStick(JoyAxisInput {
                    x: state.left_stick_x,
                    y: state.left_stick_y,
                })));
            }
            if state.right_stick_x != old_state.right_stick_x
                || state.right_stick_y != old_state.right_stick_y
            {
                events.push(Event::Axis(AxisEvent::RStick(JoyAxisInput {
                    x: state.right_stick_x,
                    y: state.right_stick_y,
                })));
            }
        }
        events
    }
}
