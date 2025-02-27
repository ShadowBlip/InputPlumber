use std::{collections::HashSet, error::Error, fmt::Debug, fs::File};

use packed_struct::prelude::*;
use tokio::sync::mpsc::{self, error::TryRecvError, Receiver};
use uhid_virt::{Bus, CreateParams, StreamError, UHIDDevice};
use zbus::Connection;

use crate::{
    dbus::interface::target::{gamepad::TargetGamepadInterface, TargetInterface},
    drivers::unified_gamepad::{
        capability::InputCapability,
        driver::{UNIFIED_CONTROLLER_PID, UNIFIED_CONTROLLER_VID},
        reports::{
            input_capability_report::{InputCapabilityInfo, InputCapabilityReport},
            input_data_report::{
                BoolUpdate, InputDataReport, Int16Vector3Update, StateUpdate, UInt16Vector2Update,
                UInt8Update, ValueUpdate,
            },
            ReportType, ValueType, REPORT_DESCRIPTOR,
        },
        value::TouchValue,
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Touch, Touchpad,
        },
        composite_device::client::CompositeDeviceClient,
        event::{native::NativeEvent, value::InputValue},
        output_capability::OutputCapability,
        output_event::OutputEvent,
    },
};

use super::{
    client::TargetDeviceClient, InputError, OutputError, TargetDeviceTypeId, TargetInputDevice,
    TargetOutputDevice,
};

/// A [UnifiedGamepadDevice] implements the Unified Controller Input Specification
pub struct UnifiedGamepadDevice {
    device: UHIDDevice<File>,
    dbus_path: Option<String>,
    composite_device: Option<CompositeDeviceClient>,
    capabilities: HashSet<Capability>,
    capabilities_rx: Option<Receiver<HashSet<Capability>>>,
    capability_report: InputCapabilityReport,
    state: InputDataReport,
    clients_count: u16,
}

impl UnifiedGamepadDevice {
    /// Create a new [UnifiedGamepadDevice]
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // Create the virtual device
        let uhid_device = UnifiedGamepadDevice::create_virtual_device()?;
        let device = Self {
            device: uhid_device,
            dbus_path: None,
            composite_device: None,
            capabilities: HashSet::new(),
            capabilities_rx: None,
            capability_report: InputCapabilityReport::default(),
            state: InputDataReport::default(),
            clients_count: 0,
        };

        Ok(device)
    }

    /// Create the virtual device to emulate
    fn create_virtual_device() -> Result<UHIDDevice<File>, Box<dyn Error>> {
        let device = UHIDDevice::create(CreateParams {
            name: "InputPlumber Unified Gamepad".to_string(),
            phys: "".to_string(),
            uniq: "".to_string(),
            bus: Bus::USB,
            // TODO: Find an appropriate VID/PID to use. For the time being, this
            // is using the OpenInput VID/PID. We may be able to ask for a VID/PID
            // from OpenMoko: https://github.com/openmoko/openmoko-usb-oui/tree/master
            vendor: UNIFIED_CONTROLLER_VID as u32,
            product: UNIFIED_CONTROLLER_PID as u32,
            version: 1,
            country: 0,
            rd_data: REPORT_DESCRIPTOR.to_vec(),
        })?;

        Ok(device)
    }

    /// Checks to see if new capabilities are available in the capabilities channel
    fn receive_new_capabilities(&mut self) -> Option<HashSet<Capability>> {
        let rx = self.capabilities_rx.as_mut()?;

        match rx.try_recv() {
            Ok(capabilities) => Some(capabilities),
            Err(e) => match e {
                TryRecvError::Empty => None,
                TryRecvError::Disconnected => {
                    self.capabilities_rx = None;
                    None
                }
            },
        }
    }

    /// Update the device capabilities with the given capabilities
    fn update_capabilities(&mut self, capabilities: HashSet<Capability>) {
        log::debug!("Updating device capabilities with: {capabilities:?}");
        let Some(composite_device) = self.composite_device.as_ref() else {
            log::warn!("No composite device set to update capabilities");
            return;
        };

        // Set the capabilities of the device
        self.capabilities = capabilities.clone();

        // Update the capability report with the source capabilities
        let mut cap_info: Vec<InputCapabilityInfo> = capabilities
            .clone()
            .into_iter()
            .map(|cap| cap.into())
            .collect();
        cap_info.sort_by_key(|cap| cap.value_type.order_priority());

        // Update the capability report
        self.capability_report = InputCapabilityReport::default();
        for info in cap_info {
            log::trace!("Updating report with info: {info}");
            if let Err(e) = self.capability_report.add_capability(info) {
                log::warn!("Failed to add input capability for gamepad: {e}");
            }
        }
        log::debug!("Using capability report: {}", self.capability_report);

        // Inform the composite device that the capabilities have changed
        if let Some(dbus_path) = self.dbus_path.as_ref() {
            log::debug!("Updating composite device with new capabilities");
            if let Err(e) = composite_device
                .blocking_update_target_capabilities(dbus_path.clone(), capabilities)
            {
                log::warn!("Failed to update target capabilities: {e:?}");
            }
        }

        log::debug!("Updated capabilities");
    }

    /// Write the current device state to the virtual device
    fn write_state(&mut self) -> Result<(), Box<dyn Error>> {
        let data = self.state.pack()?;

        // Write the state to the virtual HID
        if let Err(e) = self.device.write(&data) {
            let err = format!("Failed to write input data report: {:?}", e);
            return Err(err.into());
        }

        Ok(())
    }

    /// Handle [OutputEvent::Output] events from the HIDRAW device. These are
    /// output events which should be forwarded back to source devices.
    fn handle_output(&mut self, _data: Vec<u8>) -> Result<Vec<OutputEvent>, Box<dyn Error>> {
        // TODO: Implement output events
        Ok(vec![])
    }

    /// Handle [OutputEvent::GetReport] events from the HIDRAW device
    fn handle_get_report(
        &mut self,
        id: u32,
        report_number: u8,
        report_type: uhid_virt::ReportType,
    ) -> Result<(), Box<dyn Error>> {
        // We only support getting feature reports
        if report_type != uhid_virt::ReportType::Feature {
            return Ok(());
        }

        let report_type = ReportType::from(report_number);
        match report_type {
            ReportType::Unknown => (),
            ReportType::InputCapabilityReport => {
                log::debug!("GetFeatureReport: InputCapabilityReport");
                // Write the input capabilities of the device
                let response = self.capability_report.pack_to_vec()?;
                self.device.write_get_report_reply(id, 0, response)?;
                // NOTE: Feature reports only support a maximum of 64 bytes?
            }
            ReportType::InputDataReport => (),
            ReportType::OutputCapabilityReport => (),
            ReportType::OutputDataReport => (),
        }

        Ok(())
    }
}

impl TargetInputDevice for UnifiedGamepadDevice {
    /// Start the DBus interface for this target device
    fn start_dbus_interface(
        &mut self,
        dbus: Connection,
        path: String,
        client: TargetDeviceClient,
        type_id: TargetDeviceTypeId,
    ) {
        log::debug!("Starting dbus interface: {path}");
        log::trace!("Using device client: {client:?}");
        self.dbus_path = Some(path.clone());
        tokio::task::spawn(async move {
            let generic_interface = TargetInterface::new(&type_id);
            let iface = TargetGamepadInterface::new(type_id.name().to_owned());

            let object_server = dbus.object_server();
            let (gen_result, result) = tokio::join!(
                object_server.at(path.clone(), generic_interface),
                object_server.at(path.clone(), iface)
            );

            if gen_result.is_err() || result.is_err() {
                log::debug!("Failed to start dbus interface: {path} generic: {gen_result:?} type-specific: {result:?}");
            } else {
                log::debug!("Started dbus interface: {path}");
            }
        });
    }

    fn on_composite_device_attached(
        &mut self,
        composite_device: CompositeDeviceClient,
    ) -> Result<(), InputError> {
        self.composite_device = Some(composite_device.clone());

        // Spawn a task to asyncronously fetch the source capabilities of
        // the composite device.
        let (tx, rx) = mpsc::channel(1);
        tokio::task::spawn(async move {
            log::debug!("Getting capabilities from the composite device!");
            let capabilities = match composite_device.get_capabilities().await {
                Ok(caps) => caps,
                Err(e) => {
                    log::warn!("Failed to fetch composite device capabilities: {e:?}");
                    return;
                }
            };
            if let Err(e) = tx.send(capabilities).await {
                log::warn!("Failed to send composite device capabilities: {e:?}");
            }
        });

        // Keep a reference to the receiver so it can be checked every poll iteration
        self.capabilities_rx = Some(rx);

        Ok(())
    }

    fn write_event(&mut self, event: NativeEvent) -> Result<(), InputError> {
        log::trace!("Received event: {event:?}");

        // Update the internal controller state when events are emitted.
        if let Err(e) = self.state.update(&self.capability_report, event.into()) {
            log::warn!("Failed to update gamepad state: {e}");
            log::warn!("Current capability report: {}", self.capability_report);
        }

        Ok(())
    }

    fn get_capabilities(&self) -> Result<Vec<Capability>, InputError> {
        // Get the input capabilities from the source device(s)
        let capabilities = Vec::from_iter(self.capabilities.iter().cloned());
        Ok(capabilities)
    }

    fn stop(&mut self) -> Result<(), InputError> {
        let _ = self.device.destroy();
        Ok(())
    }
}

impl TargetOutputDevice for UnifiedGamepadDevice {
    /// Handle reading from the device and processing input events from source
    /// devices.
    /// https://www.kernel.org/doc/html/latest/hid/uhid.html#read
    fn poll(
        &mut self,
        _composite_device: &Option<CompositeDeviceClient>,
    ) -> Result<Vec<OutputEvent>, OutputError> {
        // Check to see if there are any capability updates
        if let Some(new_capabilities) = self.receive_new_capabilities() {
            self.update_capabilities(new_capabilities);
        }

        // Read output events
        let event = match self.device.read() {
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
                Ok(vec![])
            }
            // This is sent when the HID device is stopped. Consider this as an answer to
            // UHID_DESTROY.
            uhid_virt::OutputEvent::Stop => {
                log::debug!("Stop event received");
                Ok(vec![])
            }
            // This is sent when the HID device is opened. That is, the data that the HID
            // device provides is read by some other process. You may ignore this event but
            // it is useful for power-management. As long as you haven't received this event
            // there is actually no other process that reads your data so there is no need to
            // send UHID_INPUT events to the kernel.
            uhid_virt::OutputEvent::Open => {
                self.clients_count = self.clients_count.wrapping_add(1);
                log::debug!(
                    "Open event received. Current clients: {}",
                    self.clients_count
                );
                Ok(vec![])
            }
            // This is sent when there are no more processes which read the HID data. It is
            // the counterpart of UHID_OPEN and you may as well ignore this event.
            uhid_virt::OutputEvent::Close => {
                if self.clients_count != 0 {
                    self.clients_count = self.clients_count.wrapping_sub(1)
                }
                log::debug!(
                    "Close event received. Current clients: {}",
                    self.clients_count
                );
                Ok(vec![])
            }
            // This is sent if the HID device driver wants to send raw data to the I/O
            // device. You should read the payload and forward it to the device.
            uhid_virt::OutputEvent::Output { data } => {
                log::trace!("Got output data: {:?}", data);
                let result = self.handle_output(data);
                match result {
                    Ok(events) => Ok(events),
                    Err(e) => {
                        let err = format!("Failed process output event: {:?}", e);
                        Err(err.into())
                    }
                }
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
                    "Received GetReport event: id: {id}, num: {report_number}, type: {report_type:?}"
                );
                let result = self.handle_get_report(id, report_number, report_type);
                if let Err(e) = result {
                    let err = format!("Failed to process GetReport event: {e:?}");
                    return Err(err.into());
                }
                Ok(vec![])
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
                log::debug!("Received SetReport event: id: {id}, num: {report_number}, type: {:?}, data: {:?}", report_type, data);
                Ok(vec![])
            }
        };

        // Write the current state
        if self.clients_count > 0 {
            self.write_state()?;
        }

        output_events
    }

    fn get_output_capabilities(&self) -> Result<Vec<OutputCapability>, OutputError> {
        // TODO: Get the output capabilities from the source device(s)
        Ok(vec![])
    }
}

impl Debug for UnifiedGamepadDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedGamepadDevice")
            .field("state", &self.state)
            .finish()
    }
}

/// Implementation to convert an InputPlumber [NativeEvent] into a Unified Controller [StateUpdate]
impl From<NativeEvent> for StateUpdate {
    fn from(event: NativeEvent) -> Self {
        // TODO: We need a consistent way to scale small float values to integers
        const GYRO_SCALE_FACTOR: f64 = 10.0; // amount to scale imu data
        const ACCEL_SCALE_FACTOR: f64 = 3000.0; // amount to scale imu data
        let event_capability = event.as_capability();
        let capability = event_capability.clone().into();
        match event_capability {
            Capability::None => Self::default(),
            Capability::NotImplemented => Self::default(),
            Capability::Sync => Self::default(),
            Capability::DBus(_) => Self::default(),
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(_) => {
                    let value = match event.get_value() {
                        InputValue::Bool(value) => BoolUpdate { value },
                        InputValue::Float(value) => BoolUpdate { value: value > 0.5 },
                        _ => {
                            // Cannot convert other values to bool
                            return Self::default();
                        }
                    };
                    let value = ValueUpdate::Bool(value);

                    Self { capability, value }
                }
                Gamepad::Axis(_) => {
                    let value = match event.get_value() {
                        InputValue::Vector2 { x, y } => {
                            // Normalize the x and y values from -1.0 -> 1.0 to a value between 0
                            // and u16::MAX.
                            let x = x.map(|x| (((x + 1.0) / 2.0) * u16::MAX as f64) as u16);
                            let y = y.map(|y| (((y + 1.0) / 2.0) * u16::MAX as f64) as u16);
                            UInt16Vector2Update { x, y }
                        }
                        _ => {
                            // Cannot convert other values
                            return Self::default();
                        }
                    };
                    let value = ValueUpdate::UInt16Vector2(value);

                    Self { capability, value }
                }
                Gamepad::Trigger(_) => {
                    let value = match event.get_value() {
                        InputValue::Float(value) => {
                            // Normalize the value from 0.0 -> 1.0 to a value between 0 and
                            // u8::MAX.
                            let value = (value * u8::MAX as f64) as u8;
                            UInt8Update { value }
                        }
                        _ => {
                            return Self::default();
                        }
                    };
                    let value = ValueUpdate::UInt8(value);

                    Self { capability, value }
                }
                Gamepad::Accelerometer => {
                    let value = match event.get_value() {
                        InputValue::Vector3 { x, y, z } => Int16Vector3Update {
                            x: x.map(|x| (x * ACCEL_SCALE_FACTOR) as i16),
                            y: y.map(|y| (y * ACCEL_SCALE_FACTOR) as i16),
                            z: z.map(|z| (z * ACCEL_SCALE_FACTOR) as i16),
                        },
                        _ => {
                            return Self::default();
                        }
                    };
                    let value = ValueUpdate::Int16Vector3(value);

                    Self { capability, value }
                }
                Gamepad::Gyro => {
                    let value = match event.get_value() {
                        InputValue::Vector3 { x, y, z } => Int16Vector3Update {
                            x: x.map(|x| (x * GYRO_SCALE_FACTOR) as i16),
                            y: y.map(|y| (y * GYRO_SCALE_FACTOR) as i16),
                            z: z.map(|z| (z * GYRO_SCALE_FACTOR) as i16),
                        },
                        _ => {
                            return Self::default();
                        }
                    };
                    let value = ValueUpdate::Int16Vector3(value);

                    Self { capability, value }
                }
            },
            Capability::Mouse(_) => Self::default(),
            Capability::Keyboard(_) => Self::default(),
            Capability::Touchpad(touchpad) => match touchpad {
                Touchpad::LeftPad(pad) => match pad {
                    Touch::Motion => {
                        let value = match event.get_value() {
                            InputValue::Touch {
                                index,
                                is_touching,
                                pressure,
                                x,
                                y,
                            } => TouchValue {
                                index: Integer::from_primitive(index),
                                is_touching,
                                pressure: pressure
                                    .map(|p| (p * u8::MAX as f64) as u8)
                                    .unwrap_or(u8::MAX),
                                x: x.map(|x| (x * u16::MAX as f64) as u16).unwrap_or(0),
                                y: y.map(|y| (y * u16::MAX as f64) as u16).unwrap_or(0),
                            },
                            _ => {
                                return Self::default();
                            }
                        };
                        let value = ValueUpdate::Touch(value);

                        Self { capability, value }
                    }
                    Touch::Button(_) => {
                        let value = match event.get_value() {
                            InputValue::Bool(value) => BoolUpdate { value },
                            InputValue::Float(value) => BoolUpdate { value: value > 0.5 },
                            _ => {
                                // Cannot convert other values to bool
                                return Self::default();
                            }
                        };
                        let value = ValueUpdate::Bool(value);

                        Self { capability, value }
                    }
                },
                Touchpad::RightPad(pad) => match pad {
                    Touch::Motion => {
                        let value = match event.get_value() {
                            InputValue::Touch {
                                index,
                                is_touching,
                                pressure,
                                x,
                                y,
                            } => TouchValue {
                                index: Integer::from_primitive(index),
                                is_touching,
                                pressure: pressure
                                    .map(|p| (p * u8::MAX as f64) as u8)
                                    .unwrap_or(u8::MAX),
                                x: x.map(|x| (x * u16::MAX as f64) as u16).unwrap_or(0),
                                y: y.map(|y| (y * u16::MAX as f64) as u16).unwrap_or(0),
                            },
                            _ => {
                                return Self::default();
                            }
                        };
                        let value = ValueUpdate::Touch(value);

                        Self { capability, value }
                    }
                    Touch::Button(_) => {
                        let value = match event.get_value() {
                            InputValue::Bool(value) => BoolUpdate { value },
                            InputValue::Float(value) => BoolUpdate { value: value > 0.5 },
                            _ => {
                                // Cannot convert other values to bool
                                return Self::default();
                            }
                        };
                        let value = ValueUpdate::Bool(value);

                        Self { capability, value }
                    }
                },
                Touchpad::CenterPad(pad) => match pad {
                    Touch::Motion => {
                        let value = match event.get_value() {
                            InputValue::Touch {
                                index,
                                is_touching,
                                pressure,
                                x,
                                y,
                            } => TouchValue {
                                index: Integer::from_primitive(index),
                                is_touching,
                                pressure: pressure
                                    .map(|p| (p * u8::MAX as f64) as u8)
                                    .unwrap_or(u8::MAX),
                                x: x.map(|x| (x * u16::MAX as f64) as u16).unwrap_or(0),
                                y: y.map(|y| (y * u16::MAX as f64) as u16).unwrap_or(0),
                            },
                            _ => {
                                return Self::default();
                            }
                        };
                        let value = ValueUpdate::Touch(value);

                        Self { capability, value }
                    }
                    Touch::Button(_) => {
                        let value = match event.get_value() {
                            InputValue::Bool(value) => BoolUpdate { value },
                            InputValue::Float(value) => BoolUpdate { value: value > 0.5 },
                            _ => {
                                // Cannot convert other values to bool
                                return Self::default();
                            }
                        };
                        let value = ValueUpdate::Bool(value);

                        Self { capability, value }
                    }
                },
            },
            Capability::Touchscreen(touch) => match touch {
                Touch::Motion => {
                    let value = match event.get_value() {
                        InputValue::Touch {
                            index,
                            is_touching,
                            pressure,
                            x,
                            y,
                        } => TouchValue {
                            index: Integer::from_primitive(index),
                            is_touching,
                            pressure: pressure
                                .map(|p| (p * u8::MAX as f64) as u8)
                                .unwrap_or(u8::MAX),
                            x: x.map(|x| (x * u16::MAX as f64) as u16).unwrap_or(0),
                            y: y.map(|y| (y * u16::MAX as f64) as u16).unwrap_or(0),
                        },
                        _ => {
                            return Self::default();
                        }
                    };
                    let value = ValueUpdate::Touch(value);

                    Self { capability, value }
                }
                Touch::Button(_) => {
                    let value = match event.get_value() {
                        InputValue::Bool(value) => BoolUpdate { value },
                        InputValue::Float(value) => BoolUpdate { value: value > 0.5 },
                        _ => {
                            // Cannot convert other values to bool
                            return Self::default();
                        }
                    };
                    let value = ValueUpdate::Bool(value);

                    Self { capability, value }
                }
            },
        }
    }
}

/// Implementation to convert an InputPlumber [Capability] into Unified Controller [InputCapability]
impl From<Capability> for InputCapability {
    fn from(capability: Capability) -> Self {
        // TODO: Finish implementing this
        match capability {
            Capability::None => Self::default(),
            Capability::NotImplemented => Self::default(),
            Capability::Sync => Self::default(),
            Capability::DBus(_) => Self::default(),
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(button) => match button {
                    GamepadButton::South => Self::GamepadButtonSouth,
                    GamepadButton::East => Self::GamepadButtonEast,
                    GamepadButton::North => Self::GamepadButtonNorth,
                    GamepadButton::West => Self::GamepadButtonWest,
                    GamepadButton::Start => Self::GamepadButtonStart,
                    GamepadButton::Select => Self::GamepadButtonSelect,
                    GamepadButton::Guide => Self::GamepadButtonGuide,
                    GamepadButton::QuickAccess => Self::GamepadButtonQuick,
                    GamepadButton::QuickAccess2 => Self::GamepadButtonQuick2,
                    GamepadButton::Keyboard => Self::GamepadButtonKeyboard,
                    GamepadButton::Screenshot => Self::GamepadButtonScreenshot,
                    GamepadButton::Mute => Self::GamepadButtonMute,
                    GamepadButton::DPadUp => Self::GamepadButtonDpadUp,
                    GamepadButton::DPadDown => Self::GamepadButtonDpadDown,
                    GamepadButton::DPadLeft => Self::GamepadButtonDpadLeft,
                    GamepadButton::DPadRight => Self::GamepadButtonDpadRight,
                    GamepadButton::LeftBumper => Self::GamepadButtonLeftBumper,
                    GamepadButton::LeftTop => Self::GamepadButtonLeftTop,
                    GamepadButton::LeftTrigger => Self::GamepadButtonLeftTrigger,
                    GamepadButton::LeftPaddle1 => Self::GamepadButtonLeftPaddle1,
                    GamepadButton::LeftPaddle2 => Self::GamepadButtonLeftPaddle2,
                    GamepadButton::LeftPaddle3 => Self::GamepadButtonLeftPaddle3,
                    GamepadButton::LeftStick => Self::GamepadButtonLeftStick,
                    GamepadButton::LeftStickTouch => Self::GamepadButtonLeftStickTouch,
                    GamepadButton::RightBumper => Self::GamepadButtonRightBumper,
                    GamepadButton::RightTop => Self::GamepadButtonRightTop,
                    GamepadButton::RightTrigger => Self::GamepadButtonRightTrigger,
                    GamepadButton::RightPaddle1 => Self::GamepadButtonRightPaddle1,
                    GamepadButton::RightPaddle2 => Self::GamepadButtonRightPaddle2,
                    GamepadButton::RightPaddle3 => Self::GamepadButtonRightPaddle3,
                    GamepadButton::RightStick => Self::GamepadButtonRightStick,
                    GamepadButton::RightStickTouch => Self::GamepadButtonRightStickTouch,
                },
                Gamepad::Axis(axis) => match axis {
                    GamepadAxis::LeftStick => Self::GamepadAxisLeftStick,
                    GamepadAxis::RightStick => Self::GamepadAxisRightStick,
                    GamepadAxis::Hat0 => Self::default(),
                    GamepadAxis::Hat1 => Self::default(),
                    GamepadAxis::Hat2 => Self::default(),
                    GamepadAxis::Hat3 => Self::default(),
                },
                Gamepad::Trigger(trigger) => match trigger {
                    GamepadTrigger::LeftTrigger => Self::GamepadTriggerLeft,
                    GamepadTrigger::LeftTouchpadForce => Self::GamepadTriggerLeftTouchpadForce,
                    GamepadTrigger::LeftStickForce => Self::GamepadTriggerLeftStickForce,
                    GamepadTrigger::RightTrigger => Self::GamepadTriggerRight,
                    GamepadTrigger::RightTouchpadForce => Self::GamepadTriggerRightTouchpadForce,
                    GamepadTrigger::RightStickForce => Self::GamepadTriggerRightStickForce,
                },
                Gamepad::Accelerometer => Self::GamepadAccelerometerCenter,
                Gamepad::Gyro => Self::GamepadGyroCenter,
            },
            Capability::Mouse(_) => Self::default(),
            Capability::Keyboard(_) => Self::default(),
            Capability::Touchpad(touchpad) => match touchpad {
                Touchpad::LeftPad(pad) => match pad {
                    Touch::Motion => Self::TouchpadLeftMotion,
                    Touch::Button(_) => Self::TouchpadLeftButton,
                },
                Touchpad::RightPad(pad) => match pad {
                    Touch::Motion => Self::TouchpadRightMotion,
                    Touch::Button(_) => Self::TouchpadRightButton,
                },
                Touchpad::CenterPad(pad) => match pad {
                    Touch::Motion => Self::TouchpadCenterMotion,
                    Touch::Button(_) => Self::TouchpadCenterButton,
                },
            },
            Capability::Touchscreen(touch) => match touch {
                Touch::Motion => Self::TouchscreenMotion,
                Touch::Button(_) => Self::default(),
            },
        }
    }
}

/// Implementation to convert an InputPlumber [Capability] into [InputCapabilityInfo]
impl From<Capability> for InputCapabilityInfo {
    // TODO: Finish implementing this
    fn from(value: Capability) -> Self {
        let capability = value.clone().into();
        match value {
            Capability::None => Self::default(),
            Capability::NotImplemented => Self::default(),
            Capability::Sync => Self::default(),
            Capability::DBus(_) => Self::default(),
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(_) => Self::new(capability, ValueType::Bool),
                Gamepad::Axis(_) => Self::new(capability, ValueType::UInt16Vector2),
                Gamepad::Trigger(_) => Self::new(capability, ValueType::UInt8),
                Gamepad::Accelerometer => Self::new(capability, ValueType::Int16Vector3),
                Gamepad::Gyro => Self::new(capability, ValueType::Int16Vector3),
            },
            Capability::Mouse(_) => Self::default(),
            Capability::Keyboard(_) => Self::new(capability, ValueType::Bool),
            Capability::Touchpad(touchpad) => match touchpad {
                Touchpad::LeftPad(pad) => match pad {
                    Touch::Motion => Self::new(capability, ValueType::Touch),
                    Touch::Button(_) => Self::new(capability, ValueType::Bool),
                },
                Touchpad::RightPad(pad) => match pad {
                    Touch::Motion => Self::new(capability, ValueType::Touch),
                    Touch::Button(_) => Self::new(capability, ValueType::Bool),
                },
                Touchpad::CenterPad(pad) => match pad {
                    Touch::Motion => Self::new(capability, ValueType::Touch),
                    Touch::Button(_) => Self::new(capability, ValueType::Bool),
                },
            },
            Capability::Touchscreen(touch) => match touch {
                Touch::Motion => Self::new(capability, ValueType::Touch),
                Touch::Button(_) => Self::new(capability, ValueType::Bool),
            },
        }
    }
}
