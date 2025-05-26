use std::{
    cmp::Ordering,
    collections::HashMap,
    error::Error,
    fmt::Debug,
    fs::File,
    time::{Duration, Instant},
};

use packed_struct::{
    types::{Integer, SizedInteger},
    PackedStruct,
};
use rand::Rng;
use tokio::sync::mpsc::{channel, Receiver};
use uhid_virt::{Bus, CreateParams, StreamError, UHIDDevice};

use crate::{
    config::CompositeDeviceConfig,
    drivers::steam_deck::{
        driver::VID,
        hid_report::{
            PackedHapticReport, PackedInputDataReport, PackedRumbleReport, ReportType,
            PAD_FORCE_MAX, PAD_X_MAX, PAD_X_MIN, PAD_Y_MAX, PAD_Y_MIN, STICK_FORCE_MAX,
            STICK_X_MAX, STICK_X_MIN, STICK_Y_MAX, STICK_Y_MIN, TRIGG_MAX,
        },
        report_descriptor::CONTROLLER_DESCRIPTOR,
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Touch, TouchButton,
            Touchpad,
        },
        composite_device::client::CompositeDeviceClient,
        event::{
            native::{NativeEvent, ScheduledNativeEvent},
            value::InputValue,
        },
        output_capability::{Haptic, OutputCapability},
        output_event::OutputEvent,
        target::steam_deck::ProductId,
    },
};

use super::{
    steam_deck::{
        denormalize_signed_value, denormalize_unsigned_to_signed_value, denormalize_unsigned_value,
        SteamDeckConfig,
    },
    InputError, OutputError, TargetInputDevice, TargetOutputDevice,
};

// The minimum amount of time that button up events must wait after
// a button down event.
const MIN_FRAME_TIME: Duration = Duration::from_millis(80);

pub struct SteamDeckUhidDevice {
    chip_id: [u8; 15],
    config: SteamDeckConfig,
    config_rx: Option<Receiver<SteamDeckConfig>>,
    /// Steam will send 'SetReport' commands with a report type, so it can fetch
    /// a particular result with 'GetReport'
    current_report: ReportType,
    device: Option<UHIDDevice<File>>,
    lizard_mode_enabled: bool,
    pressed_events: HashMap<Capability, Instant>,
    queued_events: Vec<ScheduledNativeEvent>,
    serial_number: String,
    state: PackedInputDataReport,
}

impl SteamDeckUhidDevice {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        SteamDeckUhidDevice::new_with_config(SteamDeckConfig::default())
    }

    /// Create a new emulated Steam Deck device with the given configuration.
    pub fn new_with_config(config: SteamDeckConfig) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            chip_id: Default::default(),
            config,
            config_rx: None,
            current_report: ReportType::InputData,
            device: None,
            lizard_mode_enabled: false,
            pressed_events: HashMap::new(),
            queued_events: vec![],
            serial_number: "1NPU7PLUMB3R".to_string(),
            state: PackedInputDataReport::default(),
        })
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(config: &SteamDeckConfig) -> Result<UHIDDevice<File>, Box<dyn Error>> {
        let device = UHIDDevice::create(CreateParams {
            name: config.name.clone(),
            phys: String::from(""),
            uniq: String::from(""),
            bus: Bus::USB,
            vendor: VID as u32,
            product: config.product_id.to_u32(),
            version: 0x1000,
            country: 0,
            rd_data: CONTROLLER_DESCRIPTOR.to_vec(),
        })?;

        Ok(device)
    }

    /// Write the current device state to the device
    fn write_state(&mut self) -> Result<(), Box<dyn Error>> {
        let data = self.state.pack()?;

        // Write the state to the virtual HID
        let Some(device) = self.device.as_mut() else {
            return Ok(());
        };

        if let Err(e) = device.write(&data) {
            let err = format!("Failed to write input data report: {:?}", e);
            return Err(err.into());
        };

        Ok(())
    }

    /// Update the internal controller state when events are emitted.
    fn update_state(&mut self, event: NativeEvent) {
        let value = event.get_value();
        let capability = event.as_capability();
        match capability {
            Capability::None => (),
            Capability::NotImplemented => (),
            Capability::Sync => (),
            Capability::DBus(_) => (),
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(btn) => match btn {
                    GamepadButton::South => self.state.a = event.pressed(),
                    GamepadButton::East => self.state.b = event.pressed(),
                    GamepadButton::North => self.state.x = event.pressed(),
                    GamepadButton::West => self.state.y = event.pressed(),
                    GamepadButton::Start => self.state.menu = event.pressed(),
                    GamepadButton::Select => self.state.options = event.pressed(),
                    GamepadButton::Guide => self.state.steam = event.pressed(),
                    GamepadButton::QuickAccess => self.state.quick_access = event.pressed(),
                    GamepadButton::DPadUp => self.state.up = event.pressed(),
                    GamepadButton::DPadDown => self.state.down = event.pressed(),
                    GamepadButton::DPadLeft => self.state.left = event.pressed(),
                    GamepadButton::DPadRight => self.state.right = event.pressed(),
                    GamepadButton::LeftBumper => self.state.l1 = event.pressed(),
                    GamepadButton::LeftTrigger => self.state.l2 = event.pressed(),
                    GamepadButton::LeftPaddle1 => self.state.l4 = event.pressed(),
                    GamepadButton::LeftPaddle2 => self.state.l5 = event.pressed(),
                    GamepadButton::LeftStick => self.state.l3 = event.pressed(),
                    GamepadButton::LeftStickTouch => self.state.l_stick_touch = event.pressed(),
                    GamepadButton::RightBumper => self.state.r1 = event.pressed(),
                    GamepadButton::RightTrigger => self.state.r2 = event.pressed(),
                    GamepadButton::RightPaddle1 => self.state.r4 = event.pressed(),
                    GamepadButton::RightPaddle2 => self.state.r5 = event.pressed(),
                    GamepadButton::RightStick => self.state.r3 = event.pressed(),
                    GamepadButton::RightStickTouch => self.state.r_stick_touch = event.pressed(),
                    GamepadButton::LeftPaddle3 => (),
                    GamepadButton::RightPaddle3 => (),
                    _ => (),
                },
                Gamepad::Axis(axis) => match axis {
                    GamepadAxis::LeftStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, STICK_X_MIN, STICK_X_MAX);
                                self.state.l_stick_x = Integer::from_primitive(value);
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, STICK_Y_MIN, STICK_Y_MAX);
                                self.state.l_stick_y = Integer::from_primitive(value);
                            }
                        }
                    }
                    GamepadAxis::RightStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, STICK_X_MIN, STICK_X_MAX);
                                self.state.r_stick_x = Integer::from_primitive(value);
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, STICK_Y_MIN, STICK_Y_MAX);
                                self.state.r_stick_y = Integer::from_primitive(value);
                            }
                        }
                    }
                    GamepadAxis::Hat0 => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => {
                                        self.state.left = true;
                                        self.state.right = false;
                                    }
                                    Ordering::Equal => {
                                        self.state.left = false;
                                        self.state.right = false;
                                    }
                                    Ordering::Greater => {
                                        self.state.right = true;
                                        self.state.left = false;
                                    }
                                }
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => {
                                        self.state.up = true;
                                        self.state.down = false;
                                    }
                                    Ordering::Equal => {
                                        self.state.down = false;
                                        self.state.up = false;
                                    }
                                    Ordering::Greater => {
                                        self.state.down = true;
                                        self.state.up = false;
                                    }
                                }
                            }
                        }
                    }
                    GamepadAxis::Hat1 => (),
                    GamepadAxis::Hat2 => (),
                    GamepadAxis::Hat3 => (),
                },
                Gamepad::Trigger(trigger) => match trigger {
                    GamepadTrigger::LeftTrigger => {
                        if let InputValue::Float(value) = value {
                            self.state.l2 = value > 0.8;
                            let value = denormalize_unsigned_value(value, TRIGG_MAX);
                            self.state.l_trigg = Integer::from_primitive(value);
                        }
                    }
                    GamepadTrigger::LeftTouchpadForce => {
                        if let InputValue::Float(value) = value {
                            let value = denormalize_unsigned_value(value, PAD_FORCE_MAX);
                            self.state.l_pad_force = Integer::from_primitive(value);
                        }
                    }
                    GamepadTrigger::LeftStickForce => {
                        if let InputValue::Float(value) = value {
                            let value = denormalize_unsigned_value(value, STICK_FORCE_MAX);
                            self.state.l_stick_force = Integer::from_primitive(value);
                        }
                    }
                    GamepadTrigger::RightTrigger => {
                        if let InputValue::Float(value) = value {
                            self.state.r2 = value > 0.8;
                            let value = denormalize_unsigned_value(value, TRIGG_MAX);
                            self.state.r_trigg = Integer::from_primitive(value);
                        }
                    }
                    GamepadTrigger::RightTouchpadForce => {
                        if let InputValue::Float(value) = value {
                            let value = denormalize_unsigned_value(value, PAD_FORCE_MAX);
                            self.state.r_pad_force = Integer::from_primitive(value);
                        }
                    }
                    GamepadTrigger::RightStickForce => {
                        if let InputValue::Float(value) = value {
                            let value = denormalize_unsigned_value(value, STICK_FORCE_MAX);
                            self.state.r_stick_force = Integer::from_primitive(value);
                        }
                    }
                },
                Gamepad::Accelerometer => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            self.state.accel_x = Integer::from_primitive(x as i16);
                        }
                        if let Some(y) = y {
                            self.state.accel_y = Integer::from_primitive(y as i16);
                        }
                        if let Some(z) = z {
                            self.state.accel_z = Integer::from_primitive(z as i16);
                        }
                    }
                }
                Gamepad::Gyro => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            self.state.pitch = Integer::from_primitive(x as i16);
                        }
                        if let Some(y) = y {
                            self.state.yaw = Integer::from_primitive(y as i16);
                        }
                        if let Some(z) = z {
                            self.state.roll = Integer::from_primitive(z as i16);
                        }
                    }
                }
                Gamepad::Dial(_) => (),
            },
            Capability::Mouse(_) => (),
            Capability::Keyboard(_) => (),
            Capability::Touchpad(touch) => match touch {
                Touchpad::LeftPad(touch_event) => match touch_event {
                    Touch::Motion => {
                        if let InputValue::Touch {
                            index: _,
                            is_touching,
                            pressure: _,
                            x,
                            y,
                        } = value
                        {
                            self.state.l_pad_touch = is_touching;
                            if let Some(x) = x {
                                let value =
                                    denormalize_unsigned_to_signed_value(x, PAD_X_MIN, PAD_X_MAX);
                                self.state.l_pad_x = Integer::from_primitive(value);
                            };
                            if let Some(y) = y {
                                let value =
                                    denormalize_unsigned_to_signed_value(y, PAD_Y_MIN, PAD_Y_MAX);
                                self.state.l_pad_y = Integer::from_primitive(value);
                            };
                        }
                    }
                    Touch::Button(button) => match button {
                        TouchButton::Touch => self.state.l_pad_touch = event.pressed(),
                        TouchButton::Press => self.state.l_pad_press = event.pressed(),
                    },
                },
                Touchpad::RightPad(touch_event) => match touch_event {
                    Touch::Motion => {
                        if let InputValue::Touch {
                            index: _,
                            is_touching,
                            pressure: _,
                            x,
                            y,
                        } = value
                        {
                            self.state.r_pad_touch = is_touching;
                            if let Some(x) = x {
                                let value =
                                    denormalize_unsigned_to_signed_value(x, PAD_X_MIN, PAD_X_MAX);
                                self.state.r_pad_x = Integer::from_primitive(value);
                            };
                            if let Some(y) = y {
                                let value =
                                    denormalize_unsigned_to_signed_value(y, PAD_Y_MIN, PAD_Y_MAX);
                                self.state.r_pad_y = Integer::from_primitive(value);
                            };
                        }
                    }
                    Touch::Button(button) => match button {
                        TouchButton::Touch => self.state.r_pad_touch = event.pressed(),
                        TouchButton::Press => self.state.r_pad_press = event.pressed(),
                    },
                },
                // Treat center pad as a right pad
                Touchpad::CenterPad(_) => (),
            },
            Capability::Touchscreen(_) => (),
        };
    }

    /// Handle [OutputEvent::Output] events from the HIDRAW device. These are
    /// events which should be forwarded back to source devices.
    fn handle_output(&mut self, data: Vec<u8>) -> Result<Vec<OutputEvent>, Box<dyn Error>> {
        // The first byte should be the report id
        let Some(report_id) = data.first() else {
            return Ok(vec![]);
        };

        log::trace!("Got output report with ID: {report_id}");
        Ok(vec![])
    }

    /// Handle [OutputEvent::GetReport] events from the HIDRAW device
    fn handle_get_report(
        &mut self,
        id: u32,
        _report_number: u8,
        _report_type: uhid_virt::ReportType,
    ) -> Result<(), Box<dyn Error>> {
        let Some(device) = self.device.as_mut() else {
            return Ok(());
        };

        let data = match self.current_report {
            ReportType::GetAttributesValues => {
                log::debug!("Sending attribute data");
                // No idea what these bytes mean, but this is
                // what is sent from the real device.
                let data = [
                    0x00,
                    ReportType::GetAttributesValues as u8,
                    0x2d,
                    0x01,
                    0x05,
                    0x12,
                    0x00,
                    0x00,
                    0x02,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x0a,
                    0x2b,
                    0x12,
                    0xa9,
                    0x62,
                    0x04,
                    0xad,
                    0xf1,
                    0xe4,
                    0x65,
                    0x09,
                    0x2e,
                    0x00,
                    0x00,
                    0x00,
                    0x0b,
                    0xa0,
                    0x0f,
                    0x00,
                    0x00,
                    0x0d,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x0c,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x0e,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                ];
                data.to_vec()
            }
            ReportType::GetStringAttribute => {
                // Reply with the serial number
                // [ReportType::GetSerial, 0x14, 0x01, ..serial?]?
                log::debug!("Sending serial number: {}", self.serial_number);
                let mut data = vec![0x0, ReportType::GetStringAttribute as u8, 0x14, 0x01];
                let mut serial_data = self.serial_number.as_bytes().to_vec();
                data.append(&mut serial_data);
                data.resize(64, 0);
                data
            }
            ReportType::GetChipId => {
                log::debug!("Sending Chip ID: {:?}", self.chip_id);
                let mut data = vec![0x00, ReportType::GetChipId as u8, 0x11, 0x00];
                let mut chip_id = self.chip_id.to_vec();
                data.append(&mut chip_id);
                data.resize(64, 0);
                data
            }
            // Don't care about other types
            _ => {
                log::trace!(
                    "Got GetReport for ReportType we aren't handling: {:?}",
                    self.current_report
                );
                vec![]
            }
        };

        // Write the report reply to the HIDRAW device
        if let Err(e) = device.write_get_report_reply(id, 0, data) {
            log::warn!("Failed to write get report reply: {:?}", e);
            return Err(e.to_string().into());
        }

        Ok(())
    }

    fn handle_set_report(
        &mut self,
        id: u32,
        _report_number: u8,
        _report_type: uhid_virt::ReportType,
        mut data: Vec<u8>,
    ) -> Result<Vec<OutputEvent>, Box<dyn Error>> {
        let Some(report_id) = data.get(1) else {
            return Ok(vec![]);
        };
        self.current_report = match (*report_id).try_into() {
            Ok(id) => id,
            Err(_) => {
                log::warn!("Unknown report id: {:#04x}", (*report_id));
                return Ok(vec![]);
            }
        };
        // uhid has an extra byte prepended, remove it.
        data.remove(0);
        let output_events = match self.current_report {
            ReportType::TriggerHapticCommand => {
                let buf = data.as_slice().try_into()?;
                let packed_haptic_report = match PackedHapticReport::unpack(buf) {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("Got error unpacking buffer as PackedHapticReport {e:?}");
                        return Ok(vec![]);
                    }
                };
                log::trace!("Got PackedHapticReport: {packed_haptic_report}");
                let event = OutputEvent::SteamDeckHaptics(packed_haptic_report);
                vec![event]
            }
            ReportType::TriggerRumbleCommand => {
                let buf = data.as_slice().try_into()?;
                let packed_rumble_report = match PackedRumbleReport::unpack(buf) {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("Got error unpacking buffer as PackedRumbleReport {e:?}");
                        return Ok(vec![]);
                    }
                };
                log::trace!("Got PackedRumbleReport: {packed_rumble_report}");
                let event = OutputEvent::SteamDeckRumble(packed_rumble_report);
                vec![event]
            }
            // Don't care about other types
            _ => {
                log::trace!(
                    "Got SetReport for ReportType we aren't handling: {:?}",
                    self.current_report
                );
                vec![]
            }
        };

        let Some(device) = self.device.as_mut() else {
            return Ok(vec![]);
        };

        // Write the report reply to the HIDRAW device
        if let Err(e) = device.write_set_report_reply(id, 0) {
            log::warn!("Failed to write set report reply: {:?}", e);
            return Err(e.to_string().into());
        }

        Ok(output_events)
    }
}

impl TargetInputDevice for SteamDeckUhidDevice {
    /// Start the driver when attached to a composite device.
    fn on_composite_device_attached(
        &mut self,
        composite_device: CompositeDeviceClient,
    ) -> Result<(), InputError> {
        let (tx, rx) = channel(1);
        let mut device_config = self.config.clone();

        // Spawn a task to wait for the composite device config. This is done
        // to prevent potential deadlocks if the composite device and target
        // device are both waiting for a response from each other.
        tokio::task::spawn(async move {
            // Get the config for this composite_device.
            let cd_config: CompositeDeviceConfig = match composite_device.get_config().await {
                Ok(config) => config,
                Err(e) => {
                    log::error!("Failed to get composite device config. Got error: {e:?}");
                    return;
                }
            };

            match cd_config.name.as_str() {
                "Lenovo Legion Go" => {
                    device_config.vendor = "Lenovo".to_string();
                    device_config.name = "Legion Go Controller".to_string();
                    device_config.product_id = ProductId::LenovoLegionGo;
                }
                "Lenovo Legion Go S" => {
                    device_config.vendor = "Lenovo".to_string();
                    device_config.name = "Legion Go S Controller".to_string();
                    device_config.product_id = ProductId::LenovoLegionGoS;
                }
                "ASUS ROG Ally" => {
                    device_config.vendor = "ASUS".to_string();
                    device_config.name = "ROG Ally Controller".to_string();
                    device_config.product_id = ProductId::AsusRogAlly;
                }
                "ASUS ROG Ally X" => {
                    device_config.vendor = "ASUS".to_string();
                    device_config.name = "ROG Ally X Controller".to_string();
                    device_config.product_id = ProductId::AsusRogAlly;
                }
                "Zotac Zone" => {
                    device_config.vendor = "Zotac".to_string();
                    device_config.name = "Zone Controller".to_string();
                    device_config.product_id = ProductId::ZotacZone;
                }
                "Steam Deck" => {
                    device_config.vendor = "Valve Corporation".to_string();
                    device_config.name = "Steam Controller".to_string();
                    // True PID will only work with the VCHI target as Steam looks for a
                    // specific bInterfaceNumber when that PID is detected.
                    device_config.product_id = ProductId::Generic;
                }
                _ => {}
            };

            log::debug!(
                "Found Steam Deck target config: {} {} PID: {:?}",
                device_config.vendor,
                device_config.name,
                device_config.product_id.to_u16(),
            );

            if let Err(e) = tx.send(device_config).await {
                log::error!("Failed to send device config to target device. Got error: {e:?}");
            };
        });

        self.config_rx = Some(rx);
        Ok(())
    }

    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");

        // Check to see if this is a button event
        // In some cases, a button down and button up event can happen within
        // the same "frame", which would result in no net state change. This
        // allows us to process up events at a later time.
        let cap = event.as_capability();
        if let Capability::Gamepad(Gamepad::Button(_)) = cap {
            if event.pressed() {
                log::trace!("Button down: {cap:?}");
                // Keep track of button down events
                self.pressed_events.insert(cap.clone(), Instant::now());
            } else {
                log::trace!("Button up: {cap:?}");
                // If the event is a button up event, check to
                // see if we received a down event in the same
                // frame.
                if let Some(last_pressed) = self.pressed_events.get(&cap) {
                    log::trace!("Button was pressed {:?} ago", last_pressed.elapsed());
                    if last_pressed.elapsed() < MIN_FRAME_TIME {
                        log::trace!("Button up & down event received in the same frame. Queueing event for the next frame.");
                        let scheduled_event = ScheduledNativeEvent::new_with_time(
                            event,
                            *last_pressed,
                            MIN_FRAME_TIME,
                        );
                        self.queued_events.push(scheduled_event);
                        return Ok(());
                    } else {
                        log::trace!("Removing button from pressed");
                        // Button up event should be processed now
                        self.pressed_events.remove(&cap);
                    }
                }
            }
        }

        // Update device state with input events
        self.update_state(event);

        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        Ok(vec![
            Capability::Gamepad(Gamepad::Accelerometer),
            Capability::Gamepad(Gamepad::Axis(GamepadAxis::LeftStick)),
            Capability::Gamepad(Gamepad::Axis(GamepadAxis::RightStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadDown)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadLeft)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadRight)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::DPadUp)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::East)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Guide)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftBumper)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle1)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftPaddle2)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftStickTouch)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::QuickAccess)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightStickTouch)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
            Capability::Gamepad(Gamepad::Gyro),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftStickForce)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTouchpadForce)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightStickForce)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTouchpadForce)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
            Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Press))),
            Capability::Touchpad(Touchpad::LeftPad(Touch::Button(TouchButton::Touch))),
            Capability::Touchpad(Touchpad::LeftPad(Touch::Motion)),
            Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Press))),
            Capability::Touchpad(Touchpad::RightPad(Touch::Button(TouchButton::Touch))),
            Capability::Touchpad(Touchpad::RightPad(Touch::Motion)),
        ])
    }

    fn scheduled_events(&mut self) -> Option<Vec<ScheduledNativeEvent>> {
        if self.queued_events.is_empty() {
            return None;
        }
        Some(self.queued_events.drain(..).collect())
    }

    fn stop(&mut self) -> Result<(), InputError> {
        let Some(device) = self.device.as_mut() else {
            return Ok(());
        };

        let _ = device.destroy();
        Ok(())
    }

    /// Clear any local state on the target device.
    fn clear_state(&mut self) {
        self.state = Default::default();
    }
}

impl TargetOutputDevice for SteamDeckUhidDevice {
    /// Update the virtual device with its current state, and read unhandled
    /// USB transfers.
    fn poll(&mut self, _: &Option<CompositeDeviceClient>) -> Result<Vec<OutputEvent>, OutputError> {
        // Create and start the device if needed
        if let Some(rx) = self.config_rx.as_mut() {
            if rx.is_empty() {
                // If the queue is empty, we're still waiting for a response from
                // the composite device.
                return Ok(vec![]);
            }
            let config = match rx.blocking_recv() {
                Some(config) => config,
                None => self.config.clone(),
            };

            let device = SteamDeckUhidDevice::create_virtual_device(&config)?;
            self.device = Some(device);
            self.config = config;
            self.config_rx = None;
            self.serial_number = format!(
                "{:04x?}-{:04x?}-1ae1c0b",
                VID,
                self.config.product_id.to_u32()
            );
            let mut rng = rand::rng();
            let chip_id: [u8; 15] = [
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
                rng.random(),
            ];
            self.chip_id = chip_id;
        }

        let Some(device) = self.device.as_mut() else {
            return Ok(vec![]);
        };

        // Increment the frame
        let frame = self.state.frame.to_primitive();
        self.state.frame = Integer::from_primitive(frame.wrapping_add(1));

        let event = match device.read() {
            Ok(event) => event,
            Err(err) => match err {
                StreamError::Io(_e) => {
                    //log::error!("Error reading from UHID device: {e:?}");
                    // Write the current state
                    self.write_state()?;
                    return Ok(vec![]);
                }
                StreamError::UnknownEventType(e) => {
                    log::debug!("Unknown event type: {:?}", e);
                    // Write the current state
                    self.write_state()?;
                    return Ok(vec![]);
                }
            },
        };

        // Match the type of UHID output event
        let output_events = match event {
            // This is sent when the HID device is started. Consider this as an answer to
            // UHID_CREATE. This is always the first event that is sent.
            uhid_virt::OutputEvent::Start { dev_flags: _ } => {
                log::debug!("Start event received");
                vec![]
            }
            // This is sent when the HID device is stopped. Consider this as an answer to
            // UHID_DESTROY.
            uhid_virt::OutputEvent::Stop => {
                log::debug!("Stop event received");
                vec![]
            }
            // This is sent when the HID device is opened. That is, the data that the HID
            // device provides is read by some other process. You may ignore this event but
            // it is useful for power-management. As long as you haven't received this event
            // there is actually no other process that reads your data so there is no need to
            // send UHID_INPUT events to the kernel.
            uhid_virt::OutputEvent::Open => {
                log::debug!("Open event received");
                vec![]
            }
            // This is sent when there are no more processes which read the HID data. It is
            // the counterpart of UHID_OPEN and you may as well ignore this event.
            uhid_virt::OutputEvent::Close => {
                log::debug!("Close event received");
                vec![]
            }
            // This is sent if the HID device driver wants to send raw data to the I/O
            // device. You should read the payload and forward it to the device.
            uhid_virt::OutputEvent::Output { data } => {
                log::trace!("Got output data: {:?}", data);
                self.handle_output(data)?
            }
            // This event is sent if the kernel driver wants to perform a GET_REPORT request
            // on the control channel as described in the HID specs. The report-type and
            // report-number are available in the payload.
            // The kernel serializes GET_REPORT requests so there will never be two in
            // parallel. However, if you fail to respond with a UHID_GET_REPORT_REPLY, the
            // request might silently time out.
            // Once you read a GET_REPORT request, you shall forward it to the HID device and
            // remember the "id" field in the payload. Once your HID device responds to the
            // GET_REPORT (or if it fails), you must send a UHID_GET_REPORT_REPLY to the
            // kernel with the exact same "id" as in the request. If the request already
            // timed out, the kernel will ignore the response silently. The "id" field is
            // never re-used, so conflicts cannot happen.
            uhid_virt::OutputEvent::GetReport {
                id,
                report_number,
                report_type,
            } => {
                log::trace!(
                    "Received GetReport event: id: {id}, num: {report_number}, type: {:?}",
                    report_type
                );
                self.handle_get_report(id, report_number, report_type)?;
                vec![]
            }
            // This is the SET_REPORT equivalent of UHID_GET_REPORT. On receipt, you shall
            // send a SET_REPORT request to your HID device. Once it replies, you must tell
            // the kernel about it via UHID_SET_REPORT_REPLY.
            // The same restrictions as for UHID_GET_REPORT apply.
            uhid_virt::OutputEvent::SetReport {
                id,
                report_number,
                report_type,
                data,
            } => {
                log::trace!("Received SetReport event: id: {id}, num: {report_number}, type: {:?}, data: {:?}", report_type, data);
                self.handle_set_report(id, report_number, report_type, data)?
            }
        };

        // Write the current state
        self.write_state()?;

        Ok(output_events)
    }

    /// Returns the possible output events this device is capable of emitting
    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        Ok(vec![
            OutputCapability::ForceFeedback,
            OutputCapability::Haptics(Haptic::TrackpadLeft),
            OutputCapability::Haptics(Haptic::TrackpadRight),
        ])
    }
}

impl Debug for SteamDeckUhidDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteamDeckDevice")
            .field("state", &self.state)
            .field("lizard_mode_enabled", &self.lizard_mode_enabled)
            .field("serial_number", &self.serial_number)
            .field("queued_events", &self.queued_events)
            .field("pressed_events", &self.pressed_events)
            .finish()
    }
}
