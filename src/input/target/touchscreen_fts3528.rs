use std::{
    error::Error,
    fs::File,
    thread,
    time::{self, Instant},
};

use packed_struct::{
    types::{Integer, SizedInteger},
    PackedStruct,
};
use tokio::sync::mpsc::{self, error::TryRecvError};
use uhid_virt::{Bus, CreateParams, OutputEvent, StreamError, UHIDDevice};
use zbus::Connection;

use crate::{
    dbus::interface::target::TargetInterface,
    drivers::fts3528::{
        driver::{PID, VID},
        hid_report::{PackedInputDataReport, TOUCHSCREEN_X_MAX, TOUCHSCREEN_Y_MAX},
        report_descriptor::TOUCHSCREEN_DESCRIPTOR,
    },
    input::{
        capability::{Capability, Touch},
        composite_device::client::CompositeDeviceClient,
        event::{native::NativeEvent, value::InputValue},
        source::hidraw::fts3528::CAPABILITIES,
    },
};

use super::{client::TargetDeviceClient, command::TargetCommand};

const POLL_INTERVAL_MS: u64 = 10;
const BUFFER_SIZE: usize = 2048;

#[derive(Debug)]
pub struct Fts3528TouchscreenDevice {
    conn: Connection,
    dbus_path: Option<String>,
    tx: mpsc::Sender<TargetCommand>,
    rx: mpsc::Receiver<TargetCommand>,
    state: PackedInputDataReport,
    composite_device: Option<CompositeDeviceClient>,
}

impl Fts3528TouchscreenDevice {
    pub fn new(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel(BUFFER_SIZE);
        Self {
            conn,
            dbus_path: None,
            tx,
            rx,
            state: PackedInputDataReport::default(),
            composite_device: None,
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
        let conn = self.conn.clone();
        self.dbus_path = Some(path.clone());
        tokio::spawn(async move {
            log::debug!("Starting dbus interface: {path}");
            let iface = TargetInterface::new(path.clone());
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
        log::debug!("Creating virtual FTS3528 Touchscreen");
        let (device_tx, mut device_rx) = mpsc::channel::<PackedInputDataReport>(BUFFER_SIZE);
        let mut device = self.create_virtual_device()?;

        // Spawn the device in its own blocking thread
        tokio::task::spawn_blocking(move || {
            let start_time = Instant::now();
            let mut state = PackedInputDataReport::default();
            'main: loop {
                // Handle reading from the device
                // https://www.kernel.org/doc/html/latest/hid/uhid.html#read
                let result = device.read();
                match result {
                    Ok(event) => {
                        match event {
                            OutputEvent::Start { dev_flags: _ } => {
                                log::debug!("Start event received");
                            }
                            OutputEvent::Stop => {
                                log::debug!("Stop event received");
                            }
                            OutputEvent::Open => {
                                log::debug!("Open event received");
                            }
                            OutputEvent::Close => {
                                log::debug!("Close event received");
                            }
                            OutputEvent::Output { data } => {
                                log::debug!("Got output data: {:?}", data);
                            }
                            OutputEvent::GetReport {
                                id,
                                report_number,
                                report_type,
                            } => {
                                log::debug!("Received GetReport event: id: {id}, num: {report_number}, type: {:?}", report_type);
                                let _ = device.write_get_report_reply(
                                    id,
                                    0,
                                    TOUCHSCREEN_DESCRIPTOR.to_vec(),
                                );
                            }
                            OutputEvent::SetReport {
                                id,
                                report_number,
                                report_type,
                                data,
                            } => {
                                log::debug!("Received SetReport event: id: {id}, num: {report_number}, type: {:?}, data: {:?}", report_type, data);
                                let _ = device.write_set_report_reply(id, 0);
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

                // Try to receive input events from the channel
                loop {
                    match device_rx.try_recv() {
                        Ok(new_state) => {
                            state = new_state;
                        }
                        Err(e) => match e {
                            TryRecvError::Empty => break,
                            TryRecvError::Disconnected => break 'main,
                        },
                    };
                }

                // Update the scan time every iteration
                let now = Instant::now();
                let scan_time = now.duration_since(start_time);
                state.scan_time = Integer::from_primitive(scan_time.as_millis() as u16);

                // Pack the state into a binary array
                let data = state.pack();
                if let Err(e) = data {
                    log::debug!("Failed to pack input report: {:?}", e);
                    continue;
                }
                let data = data.unwrap();

                // Only write reports if touches are detected
                if state.touch1.is_touching()
                    || state.touch2.is_touching()
                    || state.touch3.is_touching()
                    || state.touch4.is_touching()
                {
                    // Write the state to the virtual HID
                    if let Err(e) = device.write(&data) {
                        log::error!("Failed to write input data report: {:?}", e);
                        break;
                    }
                }

                let duration = time::Duration::from_millis(POLL_INTERVAL_MS);
                thread::sleep(duration);
            }

            log::debug!("Destroying HID device");
            if let Err(e) = device.destroy() {
                log::error!("Failed to destroy device: {:?}", e);
            }
        });

        // Listen for send events
        log::debug!("Started listening for events to send");
        while let Some(command) = self.rx.recv().await {
            match command {
                TargetCommand::SetCompositeDevice(composite_device) => {
                    self.set_composite_device(composite_device);
                }
                TargetCommand::WriteEvent(event) => {
                    // Update internal state
                    self.update_state(event);

                    // Send the state to the device
                    device_tx.send(self.state).await?;
                }
                TargetCommand::GetCapabilities(tx) => {
                    let caps = CAPABILITIES.to_vec();
                    if let Err(e) = tx.send(caps).await {
                        log::error!("Failed to send target capabilities: {e:?}");
                    }
                }
                TargetCommand::GetType(tx) => {
                    if let Err(e) = tx.send("touchscreen-fts3528".to_string()).await {
                        log::error!("Failed to send target type: {e:?}");
                    }
                }
                TargetCommand::Stop => break,
            };
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
                    .remove::<TargetInterface, String>(path.clone())
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

    /// Create the virtual device to emulate
    fn create_virtual_device(&self) -> Result<UHIDDevice<File>, Box<dyn Error>> {
        let device = UHIDDevice::create(CreateParams {
            name: String::from("FTS3528:00 2808:1015"),
            phys: String::from(""),
            uniq: String::from(""),
            bus: Bus::USB,
            vendor: VID as u32,
            product: PID as u32,
            version: 0,
            country: 0,
            rd_data: TOUCHSCREEN_DESCRIPTOR.to_vec(),
        })?;

        Ok(device)
    }

    /// Update the internal touchscreen state when events are emitted.
    fn update_state(&mut self, event: NativeEvent) {
        let value = event.get_value();
        let capability = event.as_capability();
        if capability == Capability::Touchscreen(Touch::Motion) {
            match value {
                InputValue::None => (),
                InputValue::Bool(_) => (),
                InputValue::Float(_) => (),
                InputValue::Vector2 { x: _, y: _ } => (),
                InputValue::Vector3 { x: _, y: _, z: _ } => (),
                InputValue::Touch {
                    index,
                    is_touching,
                    pressure: _,
                    x,
                    y,
                } => {
                    // TODO: Use all available slots for multi-touch
                    self.state.touch1.contact_id = index;
                    self.state.touch1.tip_switch = match is_touching {
                        true => 1,
                        false => 0,
                    };
                    // NOTE: The Deck touchscreen is rotated, so the x/y values
                    // are flipped.
                    if let Some(x) = x {
                        let denormal_y = denormalize_unsigned_value(x, TOUCHSCREEN_Y_MAX as f64);
                        self.state.touch1.set_y(denormal_y);
                        self.state.touch1.set_y2(denormal_y);
                    }
                    if let Some(y) = y {
                        // Touch events come in where (0, 0) is the top-left, but
                        // because of screen rotation, these values are different:
                        //   Top of the screen: x == 1.0
                        //   Bottom of the screen: x == 0.0
                        let denormal_x =
                            denormalize_unsigned_value(1.0 - y, TOUCHSCREEN_X_MAX as f64);
                        self.state.touch1.set_x(denormal_x);
                        self.state.touch1.set_x2(denormal_x);
                    }
                    if is_touching {
                        self.state.contact_count = 1;
                    } else {
                        self.state.contact_count = 0;
                    }
                }
            };
        }
    }
}

/// De-normalizes the given value from 0.0 - 1.0 into a real value based on
/// the maximum axis range.
fn denormalize_unsigned_value(normal_value: f64, max: f64) -> u16 {
    (normal_value * max).round() as u16
}
