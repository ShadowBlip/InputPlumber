use packed_struct::{
    types::{Integer, SizedInteger},
    PackedStruct,
};
use std::{
    cmp::Ordering,
    collections::HashMap,
    error::Error,
    fmt::Debug,
    time::{Duration, Instant},
};
use virtual_usb::{
    usb::{
        hid::{HidInterfaceBuilder, HidReportType, HidRequest, HidSubclass, InterfaceProtocol},
        ConfigurationBuilder, DeviceClass, Direction, EndpointBuilder, LangId, SynchronizationType,
        TransferType, Type, UsageType,
    },
    usbip::UsbIpDirection,
    vhci_hcd::load_vhci_hcd,
    virtual_usb::{Reply, VirtualUSBDevice, VirtualUSBDeviceBuilder, Xfer},
};

use crate::{
    drivers::steam_deck::{
        driver::{PID, VID},
        hid_report::{
            PackedHapticReport, PackedInputDataReport, PackedRumbleReport, ReportType,
            PAD_FORCE_MAX, PAD_X_MAX, PAD_X_MIN, PAD_Y_MAX, PAD_Y_MIN, STICK_FORCE_MAX,
            STICK_X_MAX, STICK_X_MIN, STICK_Y_MAX, STICK_Y_MIN, TRIGG_MAX,
        },
        report_descriptor::{CONTROLLER_DESCRIPTOR, KEYBOARD_DESCRIPTOR, MOUSE_DESCRIPTOR},
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
    },
};

use super::{InputError, OutputError, TargetInputDevice, TargetOutputDevice};

// The minimum amount of time that button up events must wait after
// a button down event.
const MIN_FRAME_TIME: Duration = Duration::from_millis(80);

pub struct SteamDeckDevice {
    device: VirtualUSBDevice,
    state: PackedInputDataReport,
    /// Steam will send 'SetReport' commands with a report type, so it can fetch
    /// a particular result with 'GetReport'
    current_report: ReportType,
    lizard_mode_enabled: bool,
    serial_number: String,
    queued_events: Vec<ScheduledNativeEvent>,
    pressed_events: HashMap<Capability, Instant>,
    output_event: Option<OutputEvent>,
}

impl SteamDeckDevice {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // Ensure the vhci_hcd kernel module is loaded
        log::debug!("Ensuring vhci_hcd kernel module is loaded");
        if let Err(e) = load_vhci_hcd() {
            return Err(e.to_string().into());
        }

        // Create and start the virtual USB device
        let mut device = SteamDeckDevice::create_virtual_device()?;
        device.start()?;

        Ok(Self {
            device,
            state: PackedInputDataReport::default(),
            current_report: ReportType::InputData,
            lizard_mode_enabled: false,
            serial_number: "INPU7PLUMB3R".to_string(),
            queued_events: vec![],
            pressed_events: HashMap::new(),
            output_event: None,
        })
    }

    /// Create the virtual device to emulate
    fn create_virtual_device() -> Result<VirtualUSBDevice, Box<dyn Error>> {
        // Configuration values can be obtained from a real device with "sudo lsusb -v"
        let virtual_device = VirtualUSBDeviceBuilder::new(VID, PID)
            .class(DeviceClass::UseInterface)
            .supported_langs(vec![LangId::EnglishUnitedStates])
            .manufacturer("Valve Software")
            .product("Steam Controller")
            .max_packet_size(64)
            .configuration(
                ConfigurationBuilder::new()
                    .max_power(500)
                    // Mouse (iface 0)
                    .interface(
                        HidInterfaceBuilder::new()
                            .country_code(0)
                            .protocol(InterfaceProtocol::Mouse)
                            .subclass(HidSubclass::None)
                            .report_descriptor(&MOUSE_DESCRIPTOR)
                            .endpoint_descriptor(
                                EndpointBuilder::new()
                                    .address_num(1)
                                    .direction(Direction::In)
                                    .transfer_type(TransferType::Interrupt)
                                    .sync_type(SynchronizationType::NoSynchronization)
                                    .usage_type(UsageType::Data)
                                    .max_packet_size(0x0008)
                                    .build(),
                            )
                            .build(),
                    )
                    // Keyboard (iface 1)
                    .interface(
                        HidInterfaceBuilder::new()
                            .country_code(33)
                            .protocol(InterfaceProtocol::Keyboard)
                            .subclass(HidSubclass::Boot)
                            .report_descriptor(&KEYBOARD_DESCRIPTOR)
                            .endpoint_descriptor(
                                EndpointBuilder::new()
                                    .address_num(2)
                                    .direction(Direction::In)
                                    .transfer_type(TransferType::Interrupt)
                                    .sync_type(SynchronizationType::NoSynchronization)
                                    .usage_type(UsageType::Data)
                                    .max_packet_size(0x0008)
                                    .build(),
                            )
                            .build(),
                    )
                    // Controller (iface 2)
                    .interface(
                        HidInterfaceBuilder::new()
                            .country_code(33)
                            .protocol(InterfaceProtocol::None)
                            .subclass(HidSubclass::None)
                            .report_descriptor(&CONTROLLER_DESCRIPTOR)
                            .endpoint_descriptor(
                                EndpointBuilder::new()
                                    .address_num(3)
                                    .direction(Direction::In)
                                    .transfer_type(TransferType::Interrupt)
                                    .sync_type(SynchronizationType::NoSynchronization)
                                    .usage_type(UsageType::Data)
                                    .max_packet_size(0x0040)
                                    .build(),
                            )
                            .build(),
                    )
                    // CDC
                    //.interface(HidInterfaceBuilder::new().build())
                    // CDC Data
                    //.interface(HidInterfaceBuilder::new().build())
                    .build(),
            )
            .build();

        Ok(virtual_device)
    }

    /// Handle any non-standard transfers
    fn handle_xfer(&mut self, xfer: Xfer) -> Option<Reply> {
        match xfer.direction() {
            UsbIpDirection::Out => {
                self.handle_xfer_out(xfer);
                None
            }
            UsbIpDirection::In => self.handle_xfer_in(xfer),
        }
    }

    /// Handle any non-standard IN transfers (device -> host) for the gamepad iface
    fn handle_xfer_in(&self, xfer: Xfer) -> Option<Reply> {
        // IN transfers do not have a setup request.
        let endpoint = xfer.ep;

        // If a setup header exists, we need to reply to it.
        if xfer.header().is_some() {
            return self.handle_xfer_in_request(xfer);
        };

        // Create a reply based on the endpoint
        let reply = match endpoint {
            // Gamepad
            3 => self.handle_xfer_in_gamepad(xfer),
            // All other endpoints, write empty data for now
            _ => Reply::from_xfer(xfer, &[]),
        };

        Some(reply)
    }

    // Handle IN transfers (device -> host) for feature requests
    fn handle_xfer_in_request(&self, xfer: Xfer) -> Option<Reply> {
        let setup = xfer.header()?;

        // Only handle Class requests
        if setup.request_type() != Type::Class {
            log::warn!("Unknown request type");
            return Some(Reply::from_xfer(xfer, &[]));
        }

        // Interpret the setup request as an HID request
        let request = HidRequest::from(setup);

        let reply = match request {
            HidRequest::Unknown => {
                log::warn!("Unknown HID request!");
                Reply::from_xfer(xfer, &[])
            }
            HidRequest::GetReport(req) => {
                //log::trace!("GetReport: {req}");
                let _interface = req.interface.to_primitive();
                //log::trace!("Got GetReport data for iface {interface}");
                let report_type = req.report_type;

                // Handle GetReport
                match report_type {
                    HidReportType::Input => Reply::from_xfer(xfer, &[]),
                    HidReportType::Output => Reply::from_xfer(xfer, &[]),
                    HidReportType::Feature => {
                        // Reply based on the currently set report
                        match self.current_report {
                            ReportType::GetAttrib => {
                                log::debug!("Sending attribute data");
                                // No idea what these bytes mean, but this is
                                // what is sent from the real device.
                                let data = [
                                    ReportType::GetAttrib as u8,
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
                                    0x00,
                                ];
                                Reply::from_xfer(xfer, &data)
                            }
                            ReportType::GetSerial => {
                                // Reply with the serial number
                                // [ReportType::GetSerial, 0x14, 0x01, ..serial?]?
                                log::debug!("Sending serial number: {}", self.serial_number);
                                let mut data = vec![ReportType::GetSerial as u8, 0x14, 0x01];
                                let mut serial_data = self.serial_number.as_bytes().to_vec();
                                data.append(&mut serial_data);
                                data.resize(64, 0);
                                Reply::from_xfer(xfer, data.as_slice())
                            }
                            // Don't care about other types
                            _ => Reply::from_xfer(xfer, &[]),
                        }
                    }
                }
            }
            // Ignore other types of requests
            _ => Reply::from_xfer(xfer, &[]),
        };

        Some(reply)
    }

    // Handle IN transfers (device -> host) for the gamepad interface
    fn handle_xfer_in_gamepad(&self, xfer: Xfer) -> Reply {
        // Pack the state
        let report_data = match self.state.pack() {
            Ok(data) => data,
            Err(e) => {
                log::error!("Failed to pack input data report: {e:?}");
                return Reply::from_xfer(xfer, &[]);
            }
        };

        Reply::from_xfer(xfer, &report_data)
    }

    /// Handle any non-standard OUT transfers (host -> device) for the gamepad iface.
    /// Out transfers do not have any replies.
    fn handle_xfer_out(&mut self, xfer: Xfer) {
        // OUT transfers (host -> device) are generally always to ep 0
        //log::trace!("Got OUT transfer for endpoint: {}", xfer.ep);

        let Some(setup) = xfer.header() else {
            log::debug!("No setup request in OUT xfer");
            return;
        };

        // Only handle Class requests
        if setup.request_type() != Type::Class {
            log::debug!("Unknown request type");
            return;
        }

        // Interpret the setup request as an HID request
        let request = HidRequest::from(setup);

        match request {
            HidRequest::Unknown => {
                log::warn!("Unknown HID request!");
            }
            HidRequest::SetIdle(_req) => {
                //log::trace!("SetIdle: {req}");
            }
            // The host wants to set the given report on the device
            HidRequest::SetReport(req) => {
                //log::trace!("SetReport: {req}");
                let _interface = req.interface.to_primitive();
                let data = xfer.data;
                //log::trace!("Got SetReport data for iface {interface}: {data:?}");

                // The first byte contains the report type
                let Some(first_byte) = data.first() else {
                    log::debug!("Unable to determine report type from empty report");
                    return;
                };

                let Ok(report_type) = ReportType::try_from(*first_byte) else {
                    log::debug!("Invalid report type: {first_byte}");
                    return;
                };

                // https://github.com/libsdl-org/SDL/blob/f0363a0466f72655a1081fb96a90e1b9602ee571/src/joystick/hidapi/SDL_hidapi_steamdeck.c
                match report_type {
                    ReportType::InputData => (),
                    ReportType::SetMappings => (),
                    // ClearMappings gets called to take the controller out of lizard
                    // mode so that Steam can control it directly.
                    ReportType::ClearMappings => {
                        //log::trace!("Disabling lizard mode");
                        self.lizard_mode_enabled = false;
                    }
                    ReportType::GetMappings => (),
                    ReportType::GetAttrib => {
                        log::debug!("Attribute requested");
                        self.current_report = ReportType::GetAttrib;
                    }
                    ReportType::GetAttribLabel => (),
                    // DefaultMappings sets the device in lizard mode, so it can run
                    // without Steam.
                    ReportType::DefaultMappings => {
                        log::debug!("Setting lizard mode enabled");
                        self.lizard_mode_enabled = true;
                    }
                    ReportType::FactoryReset => (),
                    // When Steam boots up, it writes to a register with this data:
                    // Got SetReport data: [135, 3, 8, 7, 0, 0, 0, ...]
                    ReportType::WriteRegister => (),
                    ReportType::ClearRegister => (),
                    ReportType::ReadRegister => (),
                    ReportType::GetRegisterLabel => (),
                    ReportType::GetRegisterMax => (),
                    ReportType::GetRegisterDefault => (),
                    ReportType::SetMode => (),
                    ReportType::DefaultMouse => (),
                    ReportType::TriggerHapticPulse => (),
                    ReportType::RequestCommStatus => (),
                    // Configure the next GET_REPORT call to return the serial
                    // number.
                    ReportType::GetSerial => {
                        log::debug!("Serial number requested");
                        self.current_report = ReportType::GetSerial;
                    }
                    ReportType::TriggerHapticCommand => {
                        self.current_report = ReportType::TriggerHapticCommand;

                        let buf = match data.as_slice().try_into() {
                            Ok(buffer) => buffer,
                            Err(e) => {
                                log::error!("Failed to process Haptic Command: {e}");
                                return;
                            }
                        };

                        let packed_haptic_report = match PackedHapticReport::unpack(buf) {
                            Ok(report) => report,
                            Err(e) => {
                                log::error!("Failed to process Haptic Command: {e}");
                                return;
                            }
                        };
                        //log::trace!("Got PackedHapticReport: {packed_haptic_report}");
                        let event = OutputEvent::SteamDeckHaptics(packed_haptic_report);
                        self.output_event = Some(event);
                    }
                    ReportType::TriggerRumbleCommand => {
                        self.current_report = ReportType::TriggerRumbleCommand;

                        let buf = match data.as_slice().try_into() {
                            Ok(buffer) => buffer,
                            Err(e) => {
                                log::error!("Failed to process Rumble Command: {e}");
                                return;
                            }
                        };

                        let packed_rumble_report = match PackedRumbleReport::unpack(buf) {
                            Ok(report) => report,
                            Err(e) => {
                                log::error!("Failed to process Rumble Command: {e}");
                                return;
                            }
                        };
                        //log::trace!("Got PackedRumbleReport: {packed_rumble_report}");
                        let event = OutputEvent::SteamDeckRumble(packed_rumble_report);
                        self.output_event = Some(event);
                    }
                }
            }
            // Ignore other types of requests
            _ => {}
        }
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
}

impl TargetInputDevice for SteamDeckDevice {
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
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
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

    /// Stop the virtual USB read/write threads
    fn stop(&mut self) -> Result<(), InputError> {
        log::debug!("Stopping virtual Deck controller");
        self.device.stop();

        // Read from the device
        let xfer = self.device.blocking_read()?;

        // Handle any non-standard transfers
        if let Some(xfer) = xfer {
            let reply = self.handle_xfer(xfer);

            // Write to the device if a reply is necessary
            if let Some(reply) = reply {
                self.device.write(reply)?;
            }
        }

        log::debug!("Finished stopping");
        Ok(())
    }

    /// Clear any local state on the target device.
    fn clear_state(&mut self) {
        let caps = self.get_capabilities().unwrap_or_else(|_| {
            log::error!("No target device capabilities found while clearing state.");
            Vec::new()
        });
        for cap in caps {
            let ev = NativeEvent::new(cap, InputValue::Bool(false));
            self.queued_events
                .push(ScheduledNativeEvent::new(ev, Duration::from_millis(0)));
        }
    }
}

impl TargetOutputDevice for SteamDeckDevice {
    /// Update the virtual device with its current state, and read unhandled
    /// USB transfers.
    fn poll(&mut self, _: &Option<CompositeDeviceClient>) -> Result<Vec<OutputEvent>, OutputError> {
        // Increment the frame
        let frame = self.state.frame.to_primitive();
        self.state.frame = Integer::from_primitive(frame.wrapping_add(1));

        // Read from the device
        let xfer = self.device.blocking_read()?;

        // Handle any non-standard transfers
        if let Some(xfer) = xfer {
            let reply = self.handle_xfer(xfer);

            // Write to the device if a reply is necessary
            if let Some(reply) = reply {
                self.device.write(reply)?;
            }
        }

        // Handle [OutputEvent] if it was created
        let event = self.output_event.take();
        if let Some(event) = event {
            return Ok(vec![event]);
        }

        Ok(vec![])
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

impl Debug for SteamDeckDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteamDeckDevice")
            .field("device", &self.device)
            .field("state", &self.state)
            .field("lizard_mode_enabled", &self.lizard_mode_enabled)
            .field("serial_number", &self.serial_number)
            .field("queued_events", &self.queued_events)
            .field("pressed_events", &self.pressed_events)
            .finish()
    }
}

/// Convert the given normalized signed value to the real value based on the given
/// minimum and maximum axis range.
fn denormalize_signed_value(normal_value: f64, min: f64, max: f64) -> i16 {
    let mid = (max + min) / 2.0;
    let normal_value_abs = normal_value.abs();
    if normal_value >= 0.0 {
        let maximum = max - mid;
        let value = normal_value * maximum + mid;
        value as i16
    } else {
        let minimum = min - mid;
        let value = normal_value_abs * minimum + mid;
        value as i16
    }
}

/// Convert the given normalized unsigned value to the real value based on the given
/// minimum and maximum axis range.
fn denormalize_unsigned_to_signed_value(normal_value: f64, min: f64, max: f64) -> i16 {
    let normal_value = (normal_value * 2.0) - 1.0;
    denormalize_signed_value(normal_value, min, max)
}

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_unsigned_value(normal_value: f64, max: f64) -> u16 {
    (normal_value * max).round() as u16
}
