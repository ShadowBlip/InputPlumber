use std::convert::TryFrom;
use std::time::Duration;
use std::{cmp::Ordering, error::Error, thread};

use packed_struct::{
    types::{Integer, SizedInteger},
    PackedStruct,
};
use tokio::sync::mpsc::{self, error::TryRecvError};
use virtual_usb::vhci_hcd::load_vhci_hcd;
use virtual_usb::{
    usb::{
        hid::{HidInterfaceBuilder, HidReportType, HidRequest, HidSubclass, InterfaceProtocol},
        ConfigurationBuilder, DeviceClass, Direction, EndpointBuilder, LangId, SynchronizationType,
        TransferType, Type, UsageType,
    },
    usbip::UsbIpDirection,
    virtual_usb::{Reply, VirtualUSBDevice, VirtualUSBDeviceBuilder, Xfer},
};
use zbus::Connection;

use crate::drivers::steam_deck::hid_report::{
    PAD_FORCE_MAX, PAD_X_MAX, PAD_X_MIN, PAD_Y_MAX, PAD_Y_MIN, STICK_FORCE_MAX, TRIGG_MAX,
};
use crate::input::capability::GamepadTrigger;
use crate::{
    dbus::interface::target::gamepad::TargetGamepadInterface,
    drivers::steam_deck::{
        driver::{PID, VID},
        hid_report::{
            PackedInputDataReport, ReportType, STICK_X_MAX, STICK_X_MIN, STICK_Y_MAX, STICK_Y_MIN,
        },
        report_descriptor::{CONTROLLER_DESCRIPTOR, KEYBOARD_DESCRIPTOR, MOUSE_DESCRIPTOR},
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, Touch, TouchButton, Touchpad,
        },
        event::{native::NativeEvent, value::InputValue},
        source::hidraw::steam_deck::CAPABILITIES,
    },
};

use super::{client::TargetDeviceClient, command::TargetCommand};

const POLL_INTERVAL: Duration = Duration::from_millis(1);
const BUFFER_SIZE: usize = 2048;

#[derive(Debug)]
pub struct SteamDeckDevice {
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<TargetCommand>,
    rx: Option<mpsc::Receiver<TargetCommand>>,
}

impl SteamDeckDevice {
    pub fn new(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            conn,
            dbus_path: None,
            tx,
            rx: Some(rx),
        }
    }

    /// Returns a client channel that can be used to send events to this device
    pub fn client(&self) -> TargetDeviceClient {
        self.tx.clone().into()
    }

    /// Creates a new instance of the dbus device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());
        tokio::spawn(async move {
            log::debug!("Starting dbus interface: {path}");
            let name = "Steam Deck Controller".to_string();
            let iface = TargetGamepadInterface::new(name);
            if let Err(e) = conn.object_server().at(path.clone(), iface).await {
                log::debug!("Failed to start dbus interface {path}: {e:?}");
            } else {
                log::debug!("Started dbus interface on {path}");
            }
        });
        Ok(())
    }

    /// Creates and runs the target device
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Ensure the vhci_hcd kernel module is loaded
        log::debug!("Ensuring vhci_hcd kernel module is loaded");
        if let Err(e) = load_vhci_hcd() {
            return Err(e.to_string().into());
        }

        // Create the virtual USB device
        log::debug!("Creating virtual deck controller");
        let Some(mut rx) = self.rx.take() else {
            return Err("No target command receiver exists".to_string().into());
        };
        let mut device = VirtualDeckController::new();
        let mut _composite_device = None;

        // Spawn the device in its own blocking thread
        let task = tokio::task::spawn_blocking(move || {
            if let Err(e) = device.start() {
                log::error!("Error starting USB device: {e:?}");
                return;
            }
            const MAX_COMMANDS: u16 = 1024;
            'main: loop {
                // Process any target commands
                if !rx.is_empty() {
                    let mut commands_processed = 0;
                    loop {
                        match rx.try_recv() {
                            Ok(command) => match command {
                                TargetCommand::WriteEvent(event) => {
                                    // Update device state with input events
                                    device.update_state(event);
                                }
                                TargetCommand::SetCompositeDevice(composite_dev) => {
                                    _composite_device = Some(composite_dev);
                                }
                                TargetCommand::GetCapabilities(tx) => {
                                    let caps = CAPABILITIES.to_vec();
                                    if let Err(e) = tx.blocking_send(caps) {
                                        log::error!("Failed to send target capabilities: {e:?}");
                                    }
                                }
                                TargetCommand::GetType(tx) => {
                                    if let Err(e) = tx.blocking_send("steam-deck".to_string()) {
                                        log::error!("Failed to send target type: {e:?}");
                                    }
                                }
                                TargetCommand::Stop => break 'main,
                            },
                            Err(e) => match e {
                                TryRecvError::Empty => break,
                                TryRecvError::Disconnected => break 'main,
                            },
                        }

                        // Only process MAX_COMMANDS messages at a time
                        commands_processed += 1;
                        if commands_processed >= MAX_COMMANDS {
                            break;
                        }
                    }
                }

                // Read/write from the device
                if let Err(e) = device.update() {
                    log::error!("Error updating device: {e:?}");
                    break;
                }

                thread::sleep(POLL_INTERVAL);
            }

            log::debug!("Destroying Virtual USB device");
            device.stop();
        });

        // Wait for the task to complete
        task.await?;

        // Remove the DBus interface
        if let Some(path) = self.dbus_path.clone() {
            let conn = self.conn.clone();
            let path = path.clone();
            tokio::task::spawn(async move {
                log::debug!("Stopping dbus interface for {path}");
                let result = conn
                    .object_server()
                    .remove::<TargetGamepadInterface, String>(path.clone())
                    .await;
                if let Err(e) = result {
                    log::error!("Failed to stop dbus interface {path}: {e:?}");
                } else {
                    log::debug!("Stopped dbus interface for {path}");
                }
            });
        }

        Ok(())
    }
}

/// Virtual USB implementation of the Steam Deck Controller
struct VirtualDeckController {
    device: VirtualUSBDevice,
    state: PackedInputDataReport,
    /// Steam will send 'SetReport' commands with a report type, so it can fetch
    /// a particular result with 'GetReport'
    current_report: ReportType,
    lizard_mode_enabled: bool,
    serial_number: String,
}

impl VirtualDeckController {
    fn new() -> Self {
        Self {
            device: VirtualDeckController::create_virtual_device().unwrap(),
            state: PackedInputDataReport::default(),
            current_report: ReportType::InputData,
            lizard_mode_enabled: false,
            serial_number: "INPU7PLUMB3R".to_string(),
        }
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

    /// Start the read/write threads
    fn start(&mut self) -> Result<(), Box<dyn Error>> {
        self.device.start()
    }

    /// Stop the read/write threads
    fn stop(&mut self) {
        self.device.stop()
    }

    /// Update the virtual device with its current state, and read unhandled
    /// USB transfers.
    fn update(&mut self) -> Result<(), Box<dyn Error>> {
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

        Ok(())
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
                log::trace!("GetReport: {req}");
                let interface = req.interface.to_primitive();
                log::trace!("Got GetReport data for iface {interface}");
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
        log::trace!("Got OUT transfer for endpoint: {}", xfer.ep);

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
            HidRequest::SetIdle(req) => {
                log::trace!("SetIdle: {req}");
            }
            // The host wants to set the given report on the device
            HidRequest::SetReport(req) => {
                log::trace!("SetReport: {req}");
                let interface = req.interface.to_primitive();
                let data = xfer.data;
                log::trace!("Got SetReport data for iface {interface}: {data:?}");

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
                        log::trace!("Disabling lizard mode");
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
                    ReportType::TriggerHapticCommand => (),
                    ReportType::TriggerRumbleCommand => (),
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
                            is_touching: _,
                            pressure: _,
                            x,
                            y,
                        } = value
                        {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, PAD_X_MIN, PAD_X_MAX);
                                self.state.l_pad_x = Integer::from_primitive(value);
                            };
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, PAD_Y_MIN, PAD_Y_MAX);
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
                            is_touching: _,
                            pressure: _,
                            x,
                            y,
                        } = value
                        {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, PAD_X_MIN, PAD_X_MAX);
                                self.state.r_pad_x = Integer::from_primitive(value);
                            };
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, PAD_Y_MIN, PAD_Y_MAX);
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

/// Convert the given normalized value to the real value based on the given
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

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_unsigned_value(normal_value: f64, max: f64) -> u16 {
    (normal_value * max).round() as u16
}
