//! Emulates a Sony DualSense gamepad as a target input device.
//! The DualSense implementation is based on the great work done by NeroReflex
//! and the ROGueENEMY project:
//! https://github.com/NeroReflex/ROGueENEMY/
use std::{
    cmp::Ordering,
    error::Error,
    fmt::Debug,
    fs::File,
    time::{Duration, SystemTime, UNIX_EPOCH},
    usize,
};

use packed_struct::prelude::*;
use rand::{self, Rng};
use tokio::sync::mpsc::{self, error::TryRecvError};
use uhid_virt::{Bus, CreateParams, OutputEvent, StreamError, UHIDDevice};
use zbus::Connection;

use crate::{
    dbus::interface::target::gamepad::TargetGamepadInterface,
    drivers::dualsense::{
        driver::{
            DS5_ACC_RES_PER_G, DS5_EDGE_NAME, DS5_EDGE_PID, DS5_EDGE_VERSION, DS5_EDGE_VID,
            DS5_NAME, DS5_PID, DS5_TOUCHPAD_HEIGHT, DS5_TOUCHPAD_WIDTH, DS5_VERSION, DS5_VID,
            FEATURE_REPORT_CALIBRATION, FEATURE_REPORT_FIRMWARE_INFO, FEATURE_REPORT_PAIRING_INFO,
            OUTPUT_REPORT_BT, OUTPUT_REPORT_BT_SIZE, OUTPUT_REPORT_USB,
            OUTPUT_REPORT_USB_SHORT_SIZE, OUTPUT_REPORT_USB_SIZE, STICK_X_MAX, STICK_X_MIN,
            STICK_Y_MAX, STICK_Y_MIN, TRIGGER_MAX,
        },
        hid_report::{
            Direction, PackedInputDataReport, USBPackedInputDataReport, UsbPackedOutputReport,
            UsbPackedOutputReportShort,
        },
        report_descriptor::{
            DS_BT_DESCRIPTOR, DS_EDGE_BT_DESCRIPTOR, DS_EDGE_USB_DESCRIPTOR, DS_USB_DESCRIPTOR,
        },
    },
    input::{
        capability::{
            Capability, Gamepad, GamepadAxis, GamepadButton, GamepadTrigger, Touch, TouchButton,
            Touchpad,
        },
        composite_device::client::CompositeDeviceClient,
        event::{native::NativeEvent, value::InputValue},
        output_event,
    },
};

use super::{client::TargetDeviceClient, command::TargetCommand};

const POLL_INTERVAL_MS: u64 = 4;
const BUFFER_SIZE: usize = 2048;

/// The type of DualSense device to emulate. Currently two models are supported:
/// DualSense and DualSense Edge.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ModelType {
    Normal,
    Edge,
}

/// The DualSense device can be emulated using either the USB or Bluetooth buses
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BusType {
    Usb,
    Bluetooth,
}

/// The [DualSenseHardware] defines the kind of DualSense controller to emulate
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct DualSenseHardware {
    model: ModelType,
    bus_type: BusType,
    mac_addr: [u8; 6],
}

impl DualSenseHardware {
    pub fn new(model: ModelType, bus_type: BusType) -> Self {
        // "e8:47:3a:d6:e7:74"
        //let mac_addr = [0x74, 0xe7, 0xd6, 0x3a, 0x47, 0xe8];
        let mut rng = rand::thread_rng();
        let mac_addr: [u8; 6] = [
            rng.gen(),
            rng.gen(),
            rng.gen(),
            rng.gen(),
            rng.gen(),
            rng.gen(),
        ];
        log::debug!(
            "Creating new DualSense Edge device using MAC Address: {:?}",
            mac_addr
        );

        Self {
            model,
            bus_type,
            mac_addr,
        }
    }
}

impl Default for DualSenseHardware {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let mac_addr: [u8; 6] = [
            rng.gen(),
            rng.gen(),
            rng.gen(),
            rng.gen(),
            rng.gen(),
            rng.gen(),
        ];
        Self {
            model: ModelType::Normal,
            bus_type: BusType::Usb,
            mac_addr,
        }
    }
}

/// The [DualSenseDevice] is a target input device implementation that emulates
/// a Playstation DualSense controller using uhid.
#[derive(Debug)]
pub struct DualSenseDevice {
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
    state: PackedInputDataReport,
    timestamp: u8,
    composite_device: Option<CompositeDeviceClient>,
    hardware: DualSenseHardware,
}

impl DualSenseDevice {
    pub fn new(conn: Connection, hardware: DualSenseHardware) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            conn,
            dbus_path: None,
            tx,
            rx,
            state: PackedInputDataReport::Usb(USBPackedInputDataReport::new()),
            timestamp: 0,
            composite_device: None,
            hardware,
        }
    }

    /// Returns a client channel that can be used to send events to this device
    pub fn client(&self) -> TargetDeviceClient {
        self.tx.clone().into()
    }

    /// Configures the device to send output events to the given composite device
    /// channel.
    pub fn set_composite_device(&mut self, composite_device: CompositeDeviceClient) {
        self.composite_device = Some(composite_device);
    }

    /// Creates a new instance of the dbus device interface on DBus.
    pub async fn listen_on_dbus(&mut self, path: String) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting dbus interface on {path}");
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());

        let name = match self.hardware.model {
            ModelType::Edge => match self.hardware.bus_type {
                BusType::Bluetooth => "DualSense Edge (bluetooth)".to_string(),
                BusType::Usb => "DualSense Edge".to_string(),
            },
            ModelType::Normal => match self.hardware.bus_type {
                BusType::Bluetooth => "DualSense (bluetooth)".to_string(),
                BusType::Usb => "DualSense".to_string(),
            },
        };

        tokio::spawn(async move {
            log::debug!("Starting dbus interface: {path}");
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
        log::debug!("Creating virtual dualsense controller");
        let mut device = self.create_virtual_device()?;

        // Start the main run loop
        log::debug!("Starting run loop");
        let duration = Duration::from_millis(POLL_INTERVAL_MS);
        let mut interval = tokio::time::interval(duration);
        loop {
            // Sleep for the given polling interval
            interval.tick().await;

            // Receive commands/events and update local state
            if let Err(e) = self.receive_commands().await {
                log::debug!("Error receiving commands: {:?}", e);
                break;
            }

            // Poll the HIDRaw device
            if let Err(e) = self.poll(&mut device).await {
                log::debug!("Error polling UHID device: {:?}", e);
                break;
            }

            // Check if the timestamp needs to be updated
            if self.state.state().touch_data.has_touches() {
                self.timestamp = self.timestamp.wrapping_add(3); // TODO: num?
                self.state.state_mut().touch_data.timestamp = self.timestamp;
            }

            // Write the state to the device
            if let Err(e) = self.write_state(&mut device) {
                log::debug!("Error writing state to device: {:?}", e);
                break;
            }
        }
        log::debug!("Stopped listening for events");

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

        log::debug!("Destroying device");
        if let Err(e) = device.destroy() {
            log::error!("Error destroying device: {e:?}");
        }
        log::debug!("Destroyed device");

        Ok(())
    }

    /// Create the virtual device to emulate
    fn create_virtual_device(&self) -> Result<UHIDDevice<File>, Box<dyn Error>> {
        let device = UHIDDevice::create(CreateParams {
            name: match self.hardware.model {
                ModelType::Edge => String::from(DS5_EDGE_NAME),
                ModelType::Normal => String::from(DS5_NAME),
            },
            phys: String::from(""),
            uniq: format!(
                "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                self.hardware.mac_addr[5],
                self.hardware.mac_addr[4],
                self.hardware.mac_addr[3],
                self.hardware.mac_addr[2],
                self.hardware.mac_addr[1],
                self.hardware.mac_addr[0],
            ),
            bus: match self.hardware.bus_type {
                BusType::Bluetooth => Bus::BLUETOOTH,
                BusType::Usb => Bus::USB,
            },
            vendor: match self.hardware.model {
                ModelType::Edge => DS5_EDGE_VID as u32,
                ModelType::Normal => DS5_VID as u32,
            },
            product: match self.hardware.model {
                ModelType::Edge => DS5_EDGE_PID as u32,
                ModelType::Normal => DS5_PID as u32,
            },
            version: match self.hardware.model {
                ModelType::Edge => DS5_EDGE_VERSION as u32,
                ModelType::Normal => DS5_VERSION as u32,
            },
            country: 0,
            rd_data: match self.hardware.model {
                ModelType::Edge => match self.hardware.bus_type {
                    BusType::Bluetooth => DS_EDGE_BT_DESCRIPTOR.to_vec(),
                    BusType::Usb => DS_EDGE_USB_DESCRIPTOR.to_vec(),
                },
                ModelType::Normal => match self.hardware.bus_type {
                    BusType::Bluetooth => DS_BT_DESCRIPTOR.to_vec(),
                    BusType::Usb => DS_USB_DESCRIPTOR.to_vec(),
                },
            },
        })?;

        Ok(device)
    }

    /// Read commands and events sent to this device
    async fn receive_commands(&mut self) -> Result<(), Box<dyn Error>> {
        // Read commands sent to this device from the channel until it is
        // empty.
        loop {
            match self.rx.try_recv() {
                Ok(cmd) => {
                    match cmd {
                        TargetCommand::SetCompositeDevice(composite_device) => {
                            log::trace!("Recieved command to set composite device");
                            self.set_composite_device(composite_device.clone());
                        }
                        TargetCommand::WriteEvent(event) => {
                            log::trace!("Recieved event to write: {:?}", event);
                            // Update internal state
                            self.update_state(event);
                        }
                        TargetCommand::GetCapabilities(tx) => {
                            let caps = self.get_capabilities();
                            if let Err(e) = tx.send(caps).await {
                                log::error!("Failed to send target capabilities: {e:?}");
                            }
                        }
                        TargetCommand::GetType(tx) => {
                            if let Err(e) = tx.send("ds5-edge".to_string()).await {
                                log::error!("Failed to send target type: {e:?}");
                            }
                        }

                        TargetCommand::Stop => return Err("Device stopped".into()),
                    }
                }
                Err(e) => match e {
                    TryRecvError::Empty => break,
                    TryRecvError::Disconnected => {
                        return Err("Receive channel disconnected".into());
                    }
                },
            };
        }

        Ok(())
    }

    /// Handle reading from the device and processing input events from source
    /// devices over the event channel
    /// https://www.kernel.org/doc/html/latest/hid/uhid.html#read
    async fn poll(&mut self, device: &mut UHIDDevice<File>) -> Result<(), Box<dyn Error>> {
        let result = device.read();
        match result {
            Ok(event) => {
                match event {
                    // This is sent when the HID device is started. Consider this as an answer to
                    // UHID_CREATE. This is always the first event that is sent.
                    OutputEvent::Start { dev_flags: _ } => {
                        log::debug!("Start event received");
                    }
                    // This is sent when the HID device is stopped. Consider this as an answer to
                    // UHID_DESTROY.
                    OutputEvent::Stop => {
                        log::debug!("Stop event received");
                    }
                    // This is sent when the HID device is opened. That is, the data that the HID
                    // device provides is read by some other process. You may ignore this event but
                    // it is useful for power-management. As long as you haven't received this event
                    // there is actually no other process that reads your data so there is no need to
                    // send UHID_INPUT events to the kernel.
                    OutputEvent::Open => {
                        log::debug!("Open event received");
                    }
                    // This is sent when there are no more processes which read the HID data. It is
                    // the counterpart of UHID_OPEN and you may as well ignore this event.
                    OutputEvent::Close => {
                        log::debug!("Close event received");
                    }
                    // This is sent if the HID device driver wants to send raw data to the I/O
                    // device. You should read the payload and forward it to the device.
                    OutputEvent::Output { data } => {
                        log::trace!("Got output data: {:?}", data);
                        let result = self.handle_output(data).await;
                        if let Err(e) = result {
                            let err = format!("Failed process output event: {:?}", e);
                            return Err(err.into());
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
                    OutputEvent::GetReport {
                        id,
                        report_number,
                        report_type,
                    } => {
                        log::trace!(
                            "Received GetReport event: id: {id}, num: {report_number}, type: {:?}",
                            report_type
                        );
                        let result = self.handle_get_report(device, id, report_number, report_type);
                        if let Err(e) = result {
                            let err = format!("Failed to process GetReport event: {:?}", e);
                            return Err(err.into());
                        }
                    }
                    // This is the SET_REPORT equivalent of UHID_GET_REPORT. On receipt, you shall
                    // send a SET_REPORT request to your HID device. Once it replies, you must tell
                    // the kernel about it via UHID_SET_REPORT_REPLY.
                    // The same restrictions as for UHID_GET_REPORT apply.
                    OutputEvent::SetReport {
                        id,
                        report_number,
                        report_type,
                        data,
                    } => {
                        log::trace!("Received SetReport event: id: {id}, num: {report_number}, type: {:?}, data: {:?}", report_type, data);
                    }
                };
            }
            Err(err) => match err {
                StreamError::Io(_e) => (),
                StreamError::UnknownEventType(e) => {
                    log::debug!("Unknown event type: {:?}", e);
                }
            },
        };

        Ok(())
    }

    /// Handle [OutputEvent::Output] events from the HIDRAW device. These are
    /// events which should be forwarded back to source devices.
    async fn handle_output(&mut self, data: Vec<u8>) -> Result<(), Box<dyn Error>> {
        // Validate the output report size
        let _expected_report_size = match self.hardware.bus_type {
            BusType::Bluetooth => OUTPUT_REPORT_BT_SIZE,
            BusType::Usb => OUTPUT_REPORT_USB_SIZE,
        };

        // The first byte should be the report id
        let Some(report_id) = data.first() else {
            log::warn!("Received empty output report.");
            return Ok(());
        };

        log::debug!("Got output report with ID: {report_id}");

        match *report_id {
            OUTPUT_REPORT_USB => {
                log::debug!("Received USB output report with length: {}", data.len());
                let state = match data.len() {
                    OUTPUT_REPORT_USB_SIZE => {
                        let buf: [u8; OUTPUT_REPORT_USB_SIZE] = data.try_into().unwrap();
                        let report = UsbPackedOutputReport::unpack(&buf)?;
                        report.state
                    }
                    OUTPUT_REPORT_USB_SHORT_SIZE => {
                        let buf: [u8; OUTPUT_REPORT_USB_SHORT_SIZE] = data.try_into().unwrap();
                        let report = UsbPackedOutputReportShort::unpack(&buf)?;

                        // NOTE: Hack for supporting Steam Input rumble
                        let mut state = report.state;
                        if !state.allow_audio_control
                            && !state.allow_mic_volume
                            && !state.allow_speaker_volume
                            && !state.allow_headphone_volume
                            && !state.allow_left_trigger_ffb
                            && !state.allow_right_trigger_ffb
                            && !state.use_rumble_not_haptics
                            && !state.enable_rumble_emulation
                        {
                            state.use_rumble_not_haptics = true;
                        }
                        state
                    }
                    _ => {
                        log::warn!("Failed to unpack output report. Expected size {OUTPUT_REPORT_USB_SIZE} or {OUTPUT_REPORT_USB_SHORT_SIZE}, got {}.", data.len());
                        return Ok(());
                    }
                };

                log::trace!("{}", state);

                // Send the output report to the composite device so it can
                // be processed by source devices.
                let Some(composite_device) = self.composite_device.as_ref() else {
                    log::warn!("No composite device to handle output reports");
                    return Ok(());
                };

                let event = output_event::OutputEvent::DualSense(state);
                composite_device.process_output_event(event).await?;
            }
            OUTPUT_REPORT_BT => {
                log::debug!(
                    "Received Bluetooth output report with length: {}",
                    data.len()
                );
                //
            }
            _ => {
                log::debug!("Unknown output report: {report_id}");
            }
        }

        Ok(())
    }

    /// Handle [OutputEvent::GetReport] events from the HIDRAW device
    fn handle_get_report(
        &mut self,
        device: &mut UHIDDevice<File>,
        id: u32,
        report_number: u8,
        _report_type: uhid_virt::ReportType,
    ) -> Result<(), Box<dyn Error>> {
        // Handle report pairing requests
        let data = match report_number {
            // Pairing information report
            FEATURE_REPORT_PAIRING_INFO => {
                log::debug!("Got report pairing report request");
                // TODO: Can we define this somewhere as a const?
                let data = vec![
                    FEATURE_REPORT_PAIRING_INFO,
                    self.hardware.mac_addr[0],
                    self.hardware.mac_addr[1],
                    self.hardware.mac_addr[2],
                    self.hardware.mac_addr[3],
                    self.hardware.mac_addr[4],
                    self.hardware.mac_addr[5],
                    0x08,
                    0x25,
                    0x00,
                    0x1e,
                    0x00,
                    0xee,
                    0x74,
                    0xd0,
                    0xbc,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                ];

                // If this is a bluetooth gamepad, include the crc
                if self.hardware.bus_type == BusType::Bluetooth {
                    // TODO: Handle bluetooth CRC32
                }

                data
            }
            // Firmware information report
            FEATURE_REPORT_FIRMWARE_INFO => {
                log::debug!("Got report firmware info request");
                // TODO: Can we define this somewhere as a const?
                let data = vec![
                    FEATURE_REPORT_FIRMWARE_INFO,
                    0x4a,
                    0x75,
                    0x6e,
                    0x20,
                    0x31,
                    0x39,
                    0x20,
                    0x32,
                    0x30,
                    0x32,
                    0x33,
                    0x31,
                    0x34,
                    0x3a,
                    0x34,
                    0x37,
                    0x3a,
                    0x33,
                    0x34,
                    0x03,
                    0x00,
                    0x44,
                    0x00,
                    0x08,
                    0x02,
                    0x00,
                    0x01,
                    0x36,
                    0x00,
                    0x00,
                    0x01,
                    0xc1,
                    0xc8,
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
                    0x54,
                    0x01,
                    0x00,
                    0x00,
                    0x14,
                    0x00,
                    0x00,
                    0x00,
                    0x0b,
                    0x00,
                    0x01,
                    0x00,
                    0x06,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                ];

                // If this is a bluetooth gamepad, include the crc
                if self.hardware.bus_type == BusType::Bluetooth {
                    // TODO: Handle bluetooth CRC32
                }

                data
            }
            // Calibration report
            FEATURE_REPORT_CALIBRATION => {
                log::debug!("Got report request for calibration");
                // TODO: Can we define this somewhere as a const?
                let data = vec![
                    FEATURE_REPORT_CALIBRATION,
                    0xff,
                    0xfc,
                    0xff,
                    0xfe,
                    0xff,
                    0x83,
                    0x22,
                    0x78,
                    0xdd,
                    0x92,
                    0x22,
                    0x5f,
                    0xdd,
                    0x95,
                    0x22,
                    0x6d,
                    0xdd,
                    0x1c,
                    0x02,
                    0x1c,
                    0x02,
                    0xf2,
                    0x1f,
                    0xed,
                    0xdf,
                    0xe3,
                    0x20,
                    0xda,
                    0xe0,
                    0xee,
                    0x1f,
                    0xdf,
                    0xdf,
                    0x0b,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                ];

                // If this is a bluetooth gamepad, include the crc
                if self.hardware.bus_type == BusType::Bluetooth {
                    // TODO: Handle bluetooth CRC32
                }

                data
            }
            _ => {
                let err = format!("Unknown get report request with report number: {report_number}");
                return Err(err.into());
            }
        };

        // Write the report reply to the HIDRAW device
        if let Err(e) = device.write_get_report_reply(id, 0, data) {
            log::warn!("Failed to write get report reply: {:?}", e);
            return Err(e.to_string().into());
        }

        Ok(())
    }

    /// Write the current device state to the device
    fn write_state(&self, device: &mut UHIDDevice<File>) -> Result<(), Box<dyn Error>> {
        match self.state {
            PackedInputDataReport::Usb(state) => {
                let data = state.pack()?;

                // Write the state to the virtual HID
                if let Err(e) = device.write(&data) {
                    let err = format!("Failed to write input data report: {:?}", e);
                    return Err(err.into());
                }
            }
            PackedInputDataReport::Bluetooth(state) => {
                let data = state.pack()?;

                // Write the state to the virtual HID
                if let Err(e) = device.write(&data) {
                    let err = format!("Failed to write input data report: {:?}", e);
                    return Err(err.into());
                }
            }
        };

        Ok(())
    }

    /// Update the internal controller state when events are emitted.
    fn update_state(&mut self, event: NativeEvent) {
        let value = event.get_value();
        let capability = event.as_capability();
        let state = self.state.state_mut();
        match capability {
            Capability::None => (),
            Capability::NotImplemented => (),
            Capability::Sync => (),
            Capability::Gamepad(gamepad) => match gamepad {
                Gamepad::Button(btn) => match btn {
                    GamepadButton::South => state.cross = event.pressed(),
                    GamepadButton::East => state.circle = event.pressed(),
                    GamepadButton::North => state.square = event.pressed(),
                    GamepadButton::West => state.triangle = event.pressed(),
                    GamepadButton::Start => state.options = event.pressed(),
                    GamepadButton::Select => state.create = event.pressed(),
                    GamepadButton::Guide => state.ps = event.pressed(),
                    GamepadButton::QuickAccess => (),
                    GamepadButton::DPadUp => match state.dpad {
                        Direction::North => {
                            if !event.pressed() {
                                state.dpad = Direction::None
                            }
                        }
                        Direction::NorthEast => {
                            if !event.pressed() {
                                state.dpad = Direction::East
                            }
                        }
                        Direction::East => {
                            if event.pressed() {
                                state.dpad = Direction::NorthEast
                            }
                        }
                        Direction::SouthEast => {
                            if event.pressed() {
                                state.dpad = Direction::NorthEast
                            }
                        }
                        Direction::South => {
                            if event.pressed() {
                                state.dpad = Direction::North
                            }
                        }
                        Direction::SouthWest => {
                            if event.pressed() {
                                state.dpad = Direction::NorthWest
                            }
                        }
                        Direction::West => {
                            if event.pressed() {
                                state.dpad = Direction::NorthWest
                            }
                        }
                        Direction::NorthWest => {
                            if !event.pressed() {
                                state.dpad = Direction::West
                            }
                        }
                        Direction::None => {
                            if event.pressed() {
                                state.dpad = Direction::North
                            }
                        }
                    },
                    GamepadButton::DPadDown => match state.dpad {
                        Direction::North => {
                            if event.pressed() {
                                state.dpad = Direction::South
                            }
                        }
                        Direction::NorthEast => {
                            if event.pressed() {
                                state.dpad = Direction::SouthEast
                            }
                        }
                        Direction::East => {
                            if event.pressed() {
                                state.dpad = Direction::SouthEast
                            }
                        }
                        Direction::SouthEast => {
                            if !event.pressed() {
                                state.dpad = Direction::East
                            }
                        }
                        Direction::South => {
                            if !event.pressed() {
                                state.dpad = Direction::None
                            }
                        }
                        Direction::SouthWest => {
                            if !event.pressed() {
                                state.dpad = Direction::West
                            }
                        }
                        Direction::West => {
                            if event.pressed() {
                                state.dpad = Direction::SouthWest
                            }
                        }
                        Direction::NorthWest => {
                            if event.pressed() {
                                state.dpad = Direction::SouthWest
                            }
                        }
                        Direction::None => {
                            if event.pressed() {
                                state.dpad = Direction::South
                            }
                        }
                    },
                    GamepadButton::DPadLeft => match state.dpad {
                        Direction::North => {
                            if event.pressed() {
                                state.dpad = Direction::NorthWest
                            }
                        }
                        Direction::NorthEast => {
                            if event.pressed() {
                                state.dpad = Direction::NorthWest
                            }
                        }
                        Direction::East => {
                            if event.pressed() {
                                state.dpad = Direction::West
                            }
                        }
                        Direction::SouthEast => {
                            if event.pressed() {
                                state.dpad = Direction::SouthWest
                            }
                        }
                        Direction::South => {
                            if event.pressed() {
                                state.dpad = Direction::SouthWest
                            }
                        }
                        Direction::SouthWest => {
                            if !event.pressed() {
                                state.dpad = Direction::South
                            }
                        }
                        Direction::West => {
                            if !event.pressed() {
                                state.dpad = Direction::None
                            }
                        }
                        Direction::NorthWest => {
                            if !event.pressed() {
                                state.dpad = Direction::North
                            }
                        }
                        Direction::None => {
                            if event.pressed() {
                                state.dpad = Direction::West
                            }
                        }
                    },
                    GamepadButton::DPadRight => match state.dpad {
                        Direction::North => {
                            if event.pressed() {
                                state.dpad = Direction::NorthEast
                            }
                        }
                        Direction::NorthEast => {
                            if !event.pressed() {
                                state.dpad = Direction::North
                            }
                        }
                        Direction::East => {
                            if !event.pressed() {
                                state.dpad = Direction::None
                            }
                        }
                        Direction::SouthEast => {
                            if !event.pressed() {
                                state.dpad = Direction::South
                            }
                        }
                        Direction::South => {
                            if event.pressed() {
                                state.dpad = Direction::SouthEast
                            }
                        }
                        Direction::SouthWest => {
                            if event.pressed() {
                                state.dpad = Direction::SouthEast
                            }
                        }
                        Direction::West => {
                            if event.pressed() {
                                state.dpad = Direction::East
                            }
                        }
                        Direction::NorthWest => {
                            if event.pressed() {
                                state.dpad = Direction::NorthEast
                            }
                        }
                        Direction::None => {
                            if event.pressed() {
                                state.dpad = Direction::East
                            }
                        }
                    },
                    GamepadButton::LeftBumper => state.l1 = event.pressed(),
                    GamepadButton::LeftTrigger => state.l2 = event.pressed(),
                    GamepadButton::LeftPaddle1 => state.left_fn = event.pressed(),
                    GamepadButton::LeftPaddle2 => state.left_paddle = event.pressed(),
                    GamepadButton::LeftStick => state.l3 = event.pressed(),
                    GamepadButton::LeftStickTouch => (),
                    GamepadButton::RightBumper => state.r1 = event.pressed(),
                    GamepadButton::RightTrigger => state.r2 = event.pressed(),
                    GamepadButton::RightPaddle1 => state.right_fn = event.pressed(),
                    GamepadButton::RightPaddle2 => state.right_paddle = event.pressed(),
                    GamepadButton::RightStick => state.r3 = event.pressed(),
                    GamepadButton::RightStickTouch => (),
                    GamepadButton::LeftPaddle3 => (),
                    GamepadButton::RightPaddle3 => (),
                    _ => (),
                },
                Gamepad::Axis(axis) => match axis {
                    GamepadAxis::LeftStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, STICK_X_MIN, STICK_X_MAX);
                                state.joystick_l_x = value
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, STICK_Y_MIN, STICK_Y_MAX);
                                state.joystick_l_y = value
                            }
                        }
                    }
                    GamepadAxis::RightStick => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, STICK_X_MIN, STICK_X_MAX);
                                state.joystick_r_x = value
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, STICK_Y_MIN, STICK_Y_MAX);
                                state.joystick_r_y = value
                            }
                        }
                    }
                    GamepadAxis::Hat0 => {
                        if let InputValue::Vector2 { x, y } = value {
                            if let Some(x) = x {
                                let value = denormalize_signed_value(x, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => match state.dpad {
                                        Direction::North => state.dpad = Direction::NorthWest,
                                        Direction::South => state.dpad = Direction::SouthWest,
                                        _ => state.dpad = Direction::West,
                                    },
                                    Ordering::Equal => match state.dpad {
                                        Direction::NorthWest => state.dpad = Direction::North,
                                        Direction::SouthWest => state.dpad = Direction::South,
                                        Direction::NorthEast => state.dpad = Direction::North,
                                        Direction::SouthEast => state.dpad = Direction::South,
                                        Direction::East => state.dpad = Direction::None,
                                        Direction::West => state.dpad = Direction::None,
                                        _ => (),
                                    },
                                    Ordering::Greater => match state.dpad {
                                        Direction::North => state.dpad = Direction::NorthEast,
                                        Direction::South => state.dpad = Direction::SouthEast,
                                        _ => state.dpad = Direction::East,
                                    },
                                }
                            }
                            if let Some(y) = y {
                                let value = denormalize_signed_value(y, -1.0, 1.0);
                                match value.cmp(&0) {
                                    Ordering::Less => match state.dpad {
                                        Direction::East => state.dpad = Direction::NorthEast,
                                        Direction::West => state.dpad = Direction::NorthWest,
                                        _ => state.dpad = Direction::North,
                                    },
                                    Ordering::Equal => match state.dpad {
                                        Direction::NorthWest => state.dpad = Direction::West,
                                        Direction::SouthWest => state.dpad = Direction::West,
                                        Direction::NorthEast => state.dpad = Direction::East,
                                        Direction::SouthEast => state.dpad = Direction::East,
                                        Direction::North => state.dpad = Direction::None,
                                        Direction::South => state.dpad = Direction::None,
                                        _ => (),
                                    },
                                    Ordering::Greater => match state.dpad {
                                        Direction::East => state.dpad = Direction::SouthEast,
                                        Direction::West => state.dpad = Direction::SouthWest,
                                        _ => state.dpad = Direction::South,
                                    },
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
                        if let InputValue::Float(normal_value) = value {
                            let value = denormalize_unsigned_value(normal_value, TRIGGER_MAX);
                            state.l2_trigger = value
                        }
                    }
                    GamepadTrigger::LeftTouchpadForce => (),
                    GamepadTrigger::LeftStickForce => (),
                    GamepadTrigger::RightTrigger => {
                        if let InputValue::Float(normal_value) = value {
                            let value = denormalize_unsigned_value(normal_value, TRIGGER_MAX);
                            state.r2_trigger = value
                        }
                    }
                    GamepadTrigger::RightTouchpadForce => (),
                    GamepadTrigger::RightStickForce => (),
                },
                Gamepad::Accelerometer => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            state.accel_x = Integer::from_primitive(denormalize_accel_value(x))
                        }
                        if let Some(y) = y {
                            state.accel_y = Integer::from_primitive(denormalize_accel_value(y))
                        }
                        if let Some(z) = z {
                            state.accel_z = Integer::from_primitive(denormalize_accel_value(z))
                        }
                    }
                }
                Gamepad::Gyro => {
                    if let InputValue::Vector3 { x, y, z } = value {
                        if let Some(x) = x {
                            state.gyro_x = Integer::from_primitive(denormalize_gyro_value(x));
                        }
                        if let Some(y) = y {
                            state.gyro_y = Integer::from_primitive(denormalize_gyro_value(y))
                        }
                        if let Some(z) = z {
                            state.gyro_z = Integer::from_primitive(denormalize_gyro_value(z))
                        }
                    }
                }
            },
            Capability::Touchpad(touch) => {
                match touch {
                    Touchpad::CenterPad(touch_event) => {
                        match touch_event {
                            Touch::Motion => {
                                if let InputValue::Touch {
                                    index,
                                    is_touching,
                                    pressure: _,
                                    x,
                                    y,
                                } = value
                                {
                                    // Check to see if this is the start of any touch
                                    let was_touching = state.touch_data.has_touches();

                                    let idx = index as usize;
                                    // TouchData has an array size of 2, ignore more than 2 touch events.
                                    if idx > 1 {
                                        return;
                                    }
                                    if let Some(x) = x {
                                        state.touch_data.touch_finger_data[idx]
                                            .set_x(denormalize_touch_value(x, DS5_TOUCHPAD_WIDTH));
                                    }
                                    if let Some(y) = y {
                                        state.touch_data.touch_finger_data[idx]
                                            .set_y(denormalize_touch_value(y, DS5_TOUCHPAD_HEIGHT));
                                    }

                                    if is_touching {
                                        state.touch_data.touch_finger_data[idx].context = 127;
                                    } else {
                                        state.touch_data.touch_finger_data[idx].context = 128;
                                    }

                                    // Reset the timestamp back to zero when all touches
                                    // have completed
                                    let now_touching = state.touch_data.has_touches();
                                    if was_touching && !now_touching {
                                        self.timestamp = 0;
                                    }
                                }
                            }
                            Touch::Button(button) => match button {
                                TouchButton::Touch => (),
                                TouchButton::Press => state.touchpad = event.pressed(),
                            },
                        }
                    }
                    // Not supported
                    Touchpad::RightPad(_) => {}

                    Touchpad::LeftPad(_) => {}
                }
            }
            Capability::Mouse(_) => (),
            Capability::Keyboard(_) => (),
            Capability::DBus(_) => (),
            Capability::Touchscreen(_) => (),
        };
    }

    /// Returns capabilities of the target device
    fn get_capabilities(&self) -> Vec<Capability> {
        vec![
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
            Capability::Gamepad(Gamepad::Button(GamepadButton::LeftTrigger)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::North)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightBumper)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle1)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightPaddle2)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightStick)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::RightTrigger)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Select)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::South)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::Start)),
            Capability::Gamepad(Gamepad::Button(GamepadButton::West)),
            Capability::Gamepad(Gamepad::Gyro),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::LeftTrigger)),
            Capability::Gamepad(Gamepad::Trigger(GamepadTrigger::RightTrigger)),
            Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Press))),
            Capability::Touchpad(Touchpad::CenterPad(Touch::Button(TouchButton::Touch))),
            Capability::Touchpad(Touchpad::CenterPad(Touch::Motion)),
        ]
    }
}

/// Convert the given normalized value between -1.0 - 1.0 to the real value
/// based on the given minimum and maximum axis range. Playstation gamepads
/// use a range from 0-255, with 127 being the "nuetral" point.
fn denormalize_signed_value(normal_value: f64, min: f64, max: f64) -> u8 {
    let mid = (max + min) / 2.0;
    let normal_value_abs = normal_value.abs();
    if normal_value >= 0.0 {
        let maximum = max - mid;
        let value = normal_value * maximum + mid;
        value as u8
    } else {
        let minimum = min - mid;
        let value = normal_value_abs * minimum + mid;
        value as u8
    }
}

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_unsigned_value(normal_value: f64, max: f64) -> u8 {
    (normal_value * max).round() as u8
}

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_touch_value(normal_value: f64, max: f64) -> u16 {
    (normal_value * max).round() as u16
}

/// De-normalizes the given value in meters per second into a real value that
/// the DS5 controller understands.
/// DualSense accelerometer values are measured in [DS5_ACC_RES_PER_G]
/// units of G acceleration (1G == 9.8m/s). InputPlumber accelerometer
/// values are measured in units of meters per second. To denormalize
/// the value, it needs to be converted into G units (by dividing by 9.8),
/// then multiplying that value by the [DS5_ACC_RES_PER_G].
fn denormalize_accel_value(value_meters_sec: f64) -> i16 {
    let value_g = value_meters_sec / 9.8;
    let value = value_g * DS5_ACC_RES_PER_G as f64;
    value as i16
}

/// DualSense gyro values are measured in units of degrees per second.
/// InputPlumber gyro values are also measured in degrees per second.
fn denormalize_gyro_value(value_degrees_sec: f64) -> i16 {
    let value = value_degrees_sec;
    value as i16
}
