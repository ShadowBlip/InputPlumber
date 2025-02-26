use std::{error::Error, time::Duration};

use futures::StreamExt;
use packed_struct::PackedStruct;
use ratatui::{
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout},
    prelude::*,
    symbols::border,
    widgets::{Block, Widget},
};
use tokio::sync::mpsc;
use zbus::{fdo::ObjectManagerProxy, Connection};

use crate::{
    cli::ui::{
        widgets::{
            axis_gauge::AxisGauge, button_gauge::ButtonGauge, gyro_gauge::GyroGauge,
            touch_gauge::TouchGauge, trigger_gauge::TriggerGauge,
        },
        InterfaceCommand,
    },
    config::path::get_profiles_path,
    constants::{BUS_NAME, BUS_PREFIX},
    dbus::interface::{
        composite_device::CompositeDeviceInterfaceProxy,
        target::{debug::TargetDebugInterfaceProxy, TargetInterfaceProxy},
    },
    drivers::unified_gamepad::{
        capability::InputCapability,
        reports::{
            input_capability_report::InputCapabilityReport, input_data_report::InputDataReport,
            ValueType,
        },
        value::Value,
    },
    input::target::TargetDeviceTypeId,
};

use super::MenuWidget;

/// Menu for testing an input device
#[derive(Debug)]
pub struct DeviceTestMenu {
    conn: Connection,
    capability_report: Option<InputCapabilityReport>,
    rx_disconnect: mpsc::Receiver<()>,
    rx_reports: mpsc::Receiver<Vec<u8>>,
    rx_capabilities: mpsc::Receiver<Vec<u8>>,
    device_path: String,
    profile: Option<String>,
    profile_path: Option<String>,
    target_device_types: Vec<TargetDeviceTypeId>,
    intercept_mode: u32,
    ui_buttons: Vec<ButtonGauge>,
    ui_triggers: Vec<TriggerGauge>,
    ui_axes: Vec<AxisGauge>,
    ui_gyro: Vec<GyroGauge>,
    ui_touch: Vec<TouchGauge>,
}

impl DeviceTestMenu {
    pub async fn new(conn: &Connection, dbus_path: &str) -> Result<Self, Box<dyn Error>> {
        // Get a reference to the device to debug
        let device = CompositeDeviceInterfaceProxy::builder(conn)
            .path(dbus_path.to_string())?
            .build()
            .await?;

        // Save the current target devices used so they can be restored
        let mut target_device_types = vec![];
        let target_devices = device.target_devices().await?;
        for path in target_devices {
            let target_device = TargetInterfaceProxy::builder(conn)
                .path(path)?
                .build()
                .await?;
            let device_type = target_device.device_type().await?;
            let device_type =
                TargetDeviceTypeId::try_from(device_type.as_str()).map_err(|e| e.to_string())?;
            target_device_types.push(device_type);
        }

        // Save the current profile path so it can be restored
        let profile_path = {
            let path = device.profile_path().await?;
            if path.is_empty() {
                None
            } else {
                Some(path)
            }
        };
        let profile = 'profile: {
            if profile_path.is_some() {
                break 'profile None;
            }
            let data = device.get_profile_yaml().await?;
            Some(data)
        };

        // Save the current intercept mode so it can be restored
        let intercept_mode = device.intercept_mode().await?;

        // Add the debug target device if it does not exist
        if !target_device_types.iter().any(|t| t.as_str() == "debug") {
            let mut target_devices = vec!["debug".into()];
            for kind in target_device_types.iter() {
                target_devices.push(kind.as_str().to_string());
            }
            device.set_target_devices(target_devices).await?;
        }

        // Disable intercept mode
        if intercept_mode != 0 {
            device.set_intercept_mode(0).await?;
        }

        // Set the debug profile
        {
            let profile_dir = get_profiles_path();
            let profile_path = profile_dir.join("debug.yaml");
            let profile_path = profile_path.to_string_lossy().to_string();
            device.load_profile_path(profile_path).await?;
        }

        // Create channels to listen for input reports
        let (tx_reports, rx_reports) = mpsc::channel(2048);
        let (tx_capabilities, rx_capabilities) = mpsc::channel(16);
        let (tx_disconnect, rx_disconnect) = mpsc::channel(1);

        // Spawn a task to listen for input reports
        let conn_clone = conn.clone();
        let path_clone = dbus_path.to_string();
        tokio::task::spawn(async move {
            let _ = Self::listen_for_signals(
                &conn_clone,
                path_clone,
                tx_disconnect,
                tx_reports,
                tx_capabilities,
            )
            .await;
        });

        Ok(Self {
            conn: conn.clone(),
            device_path: dbus_path.to_string(),
            profile,
            profile_path,
            rx_disconnect,
            rx_reports,
            rx_capabilities,
            capability_report: None,
            target_device_types,
            intercept_mode,
            ui_buttons: Default::default(),
            ui_triggers: Default::default(),
            ui_axes: Default::default(),
            ui_gyro: Default::default(),
            ui_touch: Default::default(),
        })
    }

    // Stop the test, restoring the device state
    fn stop(&self) {
        // Restore the state of the device
        let conn = self.conn.clone();
        let dbus_path = self.device_path.clone();
        let target_device_types = self.target_device_types.clone();
        let intercept_mode = self.intercept_mode;
        let profile = self.profile.clone();
        let profile_path = self.profile_path.clone();
        tokio::task::spawn(async move {
            // Create a reference to the composite device
            let device = CompositeDeviceInterfaceProxy::builder(&conn)
                .path(dbus_path)
                .unwrap()
                .build()
                .await
                .unwrap();

            // Restore the profile
            if let Some(profile_path) = profile_path {
                let _ = device.load_profile_path(profile_path).await;
            } else if let Some(profile) = profile {
                let _ = device.load_profile_from_yaml(profile).await;
            }

            // Restore the target devices of the device
            let target_devices = target_device_types
                .clone()
                .into_iter()
                .map(|kind| kind.as_str().to_string())
                .collect();
            let _ = device.set_target_devices(target_devices).await;

            // Restore the intercept mode
            let _ = device.set_intercept_mode(intercept_mode).await;
        });

        // Wait a beat for the target devices to be restored
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

impl DeviceTestMenu {
    /// Listen for device signals over dbus
    async fn listen_for_signals(
        conn: &Connection,
        dbus_path: String,
        tx_disconnect: mpsc::Sender<()>,
        tx_reports: mpsc::Sender<Vec<u8>>,
        tx_capabilities: mpsc::Sender<Vec<u8>>,
    ) -> Result<(), Box<dyn Error>> {
        // Get a reference to the device to debug
        let device = CompositeDeviceInterfaceProxy::builder(conn)
            .path(dbus_path.to_string())?
            .build()
            .await?;

        // Wait until the debug target is available
        let mut debug_target_path = None;
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let target_devices = device.target_devices().await?;
            for path in target_devices {
                if !path.contains("/debug") {
                    continue;
                }
                debug_target_path = Some(path);
            }
            if debug_target_path.is_some() {
                break;
            }
        }
        let Some(target_path) = debug_target_path else {
            return Err("Failed to find target debug device!".to_string().into());
        };

        // Get a reference to the debug target device
        let debug_device = TargetDebugInterfaceProxy::builder(conn)
            .path(target_path)?
            .build()
            .await?;

        // Get the current capability report
        let capability_report = debug_device.input_capability_report().await?;
        tx_capabilities.send(capability_report).await?;

        // Listen for input reports
        let mut receive_report = debug_device.receive_input_report().await?;
        tokio::task::spawn(async move {
            while let Some(signal) = receive_report.next().await {
                let Ok(args) = signal.args() else { break };
                let _ = tx_reports.send(args.data).await;
            }
        });

        // Listen for capability changes
        let mut receive_caps = debug_device.receive_input_capability_report_changed().await;
        tokio::task::spawn(async move {
            while let Some(change) = receive_caps.next().await {
                let Ok(value) = change.get().await else { break };
                let _ = tx_capabilities.send(value).await;
            }
        });

        // Get a reference to the object manager to listen for controller disconnects
        let object_manager = ObjectManagerProxy::builder(conn)
            .destination(BUS_NAME)?
            .path(BUS_PREFIX)?
            .build()
            .await?;
        let mut ifaces_removed = object_manager.receive_interfaces_removed().await?;
        tokio::task::spawn(async move {
            while let Some(change) = ifaces_removed.next().await {
                let Ok(args) = change.args() else { break };
                let path = args.object_path.to_string();
                if path != dbus_path {
                    continue;
                }
                let _ = tx_disconnect.send(()).await;
            }
        });

        Ok(())
    }

    /// Render all the buttons for the device in the given area
    fn render_buttons(&self, area: Rect, buf: &mut Buffer) {
        // Create a block for the buttons
        let block = Block::bordered()
            .title("Buttons")
            .border_set(border::ROUNDED)
            .border_style(Style::new().green());
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Define the grid for the buttons
        let cells = create_grid(inside_block, 5, 9);

        // Render each gauge
        for (btn, area) in self.ui_buttons.iter().zip(cells.iter()) {
            btn.render(*area, buf);
        }
    }

    fn render_triggers(&self, area: Rect, buf: &mut Buffer) {
        // Create a block
        let block = Block::bordered()
            .title("Triggers")
            .border_set(border::ROUNDED)
            .border_style(Style::new().yellow());
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Define the grid for the widgets
        let cells = create_grid(inside_block, 4, 2);

        // Render each gauge
        for (widget, area) in self.ui_triggers.iter().zip(cells.iter()) {
            widget.render(*area, buf);
        }
    }

    fn render_axes(&self, area: Rect, buf: &mut Buffer) {
        // Create a block
        let block = Block::bordered()
            .title("Axes")
            .border_set(border::ROUNDED)
            .border_style(Style::new().yellow());
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Define the grid for the buttons
        let cells = create_grid(inside_block, 1, 2);

        // Render each widget
        for (widget, area) in self.ui_axes.iter().zip(cells.iter()) {
            widget.render(*area, buf);
        }
    }

    fn render_gyro(&self, area: Rect, buf: &mut Buffer) {
        // Create a block
        let block = Block::bordered()
            .title("Gyro")
            .border_set(border::ROUNDED)
            .border_style(Style::new().blue());
        let inside_block = block.inner(area);
        block.render(area, buf);

        let cells = create_grid(inside_block, 1, 2);

        // Render each gauge
        for (widget, area) in self.ui_gyro.iter().zip(cells.iter()) {
            widget.render(*area, buf);
        }
    }

    fn render_touch(&self, area: Rect, buf: &mut Buffer) {
        // Create a block
        let block = Block::bordered()
            .title("Touch")
            .border_set(border::ROUNDED)
            .border_style(Style::new().red());
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Define the grid for the widgets
        let cells = create_grid(inside_block, 1, 2);

        // Render each gauge
        for (widget, area) in self.ui_touch.iter().zip(cells.iter()) {
            widget.render(*area, buf);
        }
    }
}

impl MenuWidget for DeviceTestMenu {
    fn update(&mut self) -> Vec<InterfaceCommand> {
        if self.rx_capabilities.is_closed()
            || self.rx_reports.is_closed()
            || self.rx_disconnect.is_closed()
        {
            self.stop();
            return vec![InterfaceCommand::Quit];
        }

        // Check to see if the device has disconnected
        if !self.rx_disconnect.is_empty() {
            self.stop();
            return vec![InterfaceCommand::Quit];
        }

        // Check for capability report updates
        let mut capabilities_updated = false;
        while !self.rx_capabilities.is_empty() {
            let Some(data) = self.rx_capabilities.blocking_recv() else {
                self.stop();
                return vec![InterfaceCommand::Quit];
            };
            self.capability_report = InputCapabilityReport::unpack(data.as_slice()).ok();
            capabilities_updated = true;
        }

        // Update the UI if capabilities have been updated
        if capabilities_updated {
            // Clear the old UI elements
            self.ui_buttons.clear();
            self.ui_triggers.clear();
            self.ui_axes.clear();
            self.ui_gyro.clear();
            self.ui_touch.clear();

            let Some(capability_report) = self.capability_report.as_ref() else {
                self.stop();
                return vec![InterfaceCommand::Quit];
            };

            for cap in capability_report.get_capabilities() {
                match cap.value_type {
                    ValueType::None => (),
                    ValueType::Bool => {
                        let label = format!("{:?}", cap.capability);
                        let button = ButtonGauge::new(label.as_str());
                        self.ui_buttons.push(button);
                    }
                    ValueType::UInt8 => {
                        let label = format!("{:?}", cap.capability);
                        let trigger = TriggerGauge::new(label.as_str());
                        self.ui_triggers.push(trigger);
                    }
                    ValueType::UInt16 => {
                        let label = format!("{:?}", cap.capability);
                        let trigger = TriggerGauge::new(label.as_str());
                        self.ui_triggers.push(trigger);
                    }
                    ValueType::UInt16Vector2 => match cap.capability {
                        InputCapability::GamepadAxisLeftStick
                        | InputCapability::GamepadAxisRightStick => {
                            let label = format!("{:?}", cap.capability);
                            let gauge = AxisGauge::new(label.as_str());
                            self.ui_axes.push(gauge);
                        }
                        // Assume touch for everything else
                        _ => {
                            let label = format!("{:?}", cap.capability);
                            let gauge = TouchGauge::new(label.as_str());
                            self.ui_touch.push(gauge);
                        }
                    },
                    ValueType::Int16Vector3 => {
                        let label = format!("{:?}", cap.capability);
                        let gauge = GyroGauge::new(label.as_str());
                        self.ui_gyro.push(gauge);
                    }
                    ValueType::Touch => {
                        let label = format!("{:?}", cap.capability);
                        let gauge = TouchGauge::new(label.as_str());
                        self.ui_touch.push(gauge);
                    }
                }
            }
        }

        // Check for input reports
        let mut state_bytes = None;
        while !self.rx_reports.is_empty() {
            let Some(data) = self.rx_reports.blocking_recv() else {
                self.stop();
                return vec![InterfaceCommand::Quit];
            };
            state_bytes = Some(data);
        }
        let Some(state_bytes) = state_bytes else {
            return vec![];
        };
        let state_slice = state_bytes.as_slice().try_into().unwrap();
        let state = InputDataReport::unpack(state_slice).unwrap();

        // Use the capability report to decode the input report
        let Some(capability_report) = self.capability_report.as_ref() else {
            return vec![];
        };
        let values = capability_report.decode_data_report(&state).unwrap();

        // Clear the old UI elements
        self.ui_buttons.clear();
        self.ui_triggers.clear();
        self.ui_axes.clear();
        self.ui_gyro.clear();
        self.ui_touch.clear();

        // Update the interface with the values
        let capabilities = capability_report.get_capabilities();
        for (cap, value) in capabilities.iter().zip(values.iter()) {
            match value {
                Value::None => (),
                Value::Bool(value) => {
                    let label = format!("{:?}", cap.capability);
                    let mut button = ButtonGauge::new(label.as_str());
                    button.set_value(value.value);
                    self.ui_buttons.push(button);
                }
                Value::UInt8(value) => {
                    let label = format!("{:?}", cap.capability);
                    let mut trigger = TriggerGauge::new(label.as_str());
                    trigger.set_value(value.value as f64 / u8::MAX as f64);
                    self.ui_triggers.push(trigger);
                }
                Value::UInt16(value) => {
                    let label = format!("{:?}", cap.capability);
                    let mut trigger = TriggerGauge::new(label.as_str());
                    trigger.set_value(value.value as f64 / u16::MAX as f64);
                    self.ui_triggers.push(trigger);
                }
                Value::UInt16Vector2(value) => match cap.capability {
                    InputCapability::GamepadAxisLeftStick
                    | InputCapability::GamepadAxisRightStick => {
                        let label = format!("{:?}", cap.capability);
                        let mut gauge = AxisGauge::new(label.as_str());
                        let (x, y) = {
                            let x = value.x as f64 / u16::MAX as f64;
                            // Convert from 0.0 - 1.0 to -1.0 - 1.0
                            let x = 1.0 - (x * 2.0);
                            let y = value.y as f64 / u16::MAX as f64;
                            // Convert from 0.0 - 1.0 to -1.0 - 1.0
                            let y = 1.0 - (y * 2.0);
                            // X-axis is flipped?
                            (-x, y)
                        };
                        gauge.set_value(x, y);
                        self.ui_axes.push(gauge);
                    }
                    // Assume touch for everything else
                    _ => {
                        let label = format!("{:?}", cap.capability);
                        let mut gauge = TouchGauge::new(label.as_str());
                        let (x, y) = {
                            let x = value.x as f64 / u16::MAX as f64;
                            let y = value.y as f64 / u16::MAX as f64;
                            (x, y)
                        };
                        gauge.set_value(x, y, true);
                        self.ui_touch.push(gauge);
                    }
                },
                Value::Int16Vector3(value) => {
                    let label = format!("{:?}", cap.capability);
                    let mut gauge = GyroGauge::new(label.as_str());
                    //let x = value.x / i16::MAX;
                    //let y = value.y / i16::MAX;
                    //let z = value.z / i16::MAX;
                    //gauge.set_value(x as f64, y as f64, z as f64);
                    gauge.set_value(value.x as f64, value.y as f64, value.z as f64);
                    self.ui_gyro.push(gauge);
                }
                Value::Touch(value) => {
                    let label = format!("{:?}", cap.capability);
                    let mut gauge = TouchGauge::new(label.as_str());
                    let (x, y) = {
                        let x = value.x as f64 / u16::MAX as f64;
                        let y = value.y as f64 / u16::MAX as f64;
                        (x, y)
                    };
                    gauge.set_value(x, y, value.is_touching);
                    self.ui_touch.push(gauge);
                }
            }
        }

        vec![]
    }

    fn handle_key_event(
        &mut self,
        key_event: ratatui::crossterm::event::KeyEvent,
    ) -> Vec<InterfaceCommand> {
        let commands = match key_event.code {
            KeyCode::Char('q') => vec![InterfaceCommand::Quit],
            _ => vec![],
        };
        if commands.is_empty() {
            return commands;
        }

        self.stop();

        commands
    }
}

impl Widget for &DeviceTestMenu {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Split the area into two parts vertically
        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Top layout
        let top_layout = outer_layout[0];
        self.render_buttons(top_layout, buf);

        // Bottom layout
        // Split into 2 parts
        let bottom_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(outer_layout[1]);

        // Bottom-left
        // Split vertically
        let bottom_left_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(bottom_layout[0]);
        let axes_triggers_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(bottom_left_layout[0]);
        self.render_axes(axes_triggers_layout[0], buf);
        self.render_triggers(axes_triggers_layout[1], buf);
        self.render_gyro(bottom_left_layout[1], buf);

        // Bottom-right
        let bottom_right_layout = bottom_layout[1];
        self.render_touch(bottom_right_layout, buf);
    }
}

/// Creates a grid with the given rows and columns for the given area
fn create_grid(area: Rect, rows: u16, columns: u16) -> Vec<Rect> {
    // Create the column areas
    let constraints: Vec<Constraint> = (0..columns).map(|_| Constraint::Fill(1)).collect();
    let column_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    // Create the individual grid cell areas
    let mut cells = Vec::with_capacity((rows * columns) as usize);
    for column in column_areas.iter() {
        let constraints: Vec<Constraint> = (0..rows).map(|_| Constraint::Fill(1)).collect();
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(*column);
        for cell in rows.iter() {
            cells.push(*cell);
        }
    }

    cells
}
