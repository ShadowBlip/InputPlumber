use std::error::Error;

use packed_struct::PackedStruct;
use ratatui::{
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout},
    prelude::*,
    symbols::border,
    widgets::{Block, Widget},
};
use tokio::sync::mpsc;
use zbus::Connection;

use crate::{
    cli::ui::{
        widgets::{
            axis_gauge::AxisGauge, button_gauge::ButtonGauge, gyro_gauge::GyroGauge,
            touch_gauge::TouchGauge, trigger_gauge::TriggerGauge,
        },
        InterfaceCommand,
    },
    drivers::unified_gamepad::{
        capability::InputCapability,
        reports::{
            input_capability_report::InputCapabilityReport, input_data_report::InputDataReport,
            ValueType,
        },
        value::Value,
    },
};

use super::{
    debug_target::{self, SavedState},
    MenuWidget,
};

/// Strips one capability-family prefix from a capability's debug name to
/// get a display label (e.g. "GamepadButtonKeyboard" -> "Keyboard").
fn capability_label(capability: InputCapability) -> String {
    let debug_label = format!("{capability:?}");
    for prefix in [
        "GamepadButton",
        "GamepadAxis",
        "GamepadTrigger",
        "Gamepad",
        "Touchpad",
        "Touchscreen",
        "Mouse",
        "Keyboard",
    ] {
        if let Some(stripped) = debug_label.strip_prefix(prefix) {
            return stripped.to_string();
        }
    }
    debug_label
}

/// Menu for testing an input device
#[derive(Debug)]
pub struct DeviceTestMenu {
    conn: Connection,
    capability_report: Option<InputCapabilityReport>,
    rx_disconnect: mpsc::Receiver<()>,
    rx_reports: mpsc::Receiver<Vec<u8>>,
    rx_capabilities: mpsc::Receiver<Vec<u8>>,
    device_path: String,
    saved: SavedState,
    ui_buttons: Vec<ButtonGauge>,
    ui_triggers: Vec<TriggerGauge>,
    ui_axes: Vec<AxisGauge>,
    ui_gyro: Vec<GyroGauge>,
    ui_touch: Vec<TouchGauge>,
}

impl DeviceTestMenu {
    pub async fn new(conn: &Connection, dbus_path: &str) -> Result<Self, Box<dyn Error>> {
        // Attach the debug target so every source capability is visible
        let (saved, stream) = debug_target::attach(conn, dbus_path).await?;

        Ok(Self {
            conn: conn.clone(),
            device_path: dbus_path.to_string(),
            saved,
            rx_disconnect: stream.rx_disconnect,
            rx_reports: stream.rx_reports,
            rx_capabilities: stream.rx_capabilities,
            capability_report: None,
            ui_buttons: Default::default(),
            ui_triggers: Default::default(),
            ui_axes: Default::default(),
            ui_gyro: Default::default(),
            ui_touch: Default::default(),
        })
    }

    // Stop the test, restoring the device state
    fn stop(&self) {
        debug_target::restore(&self.conn, &self.device_path, &self.saved);
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

        // Calculate the number of rows/columns based on the number of
        // ui elements.
        let (columns, rows) = calculate_rows_columns(self.ui_buttons.len(), 9.0, 5.0);

        // Define the grid for the buttons
        let cells = create_grid(inside_block, rows, columns);

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

        // Calculate the number of rows/columns based on the number of
        // ui elements.
        let (columns, rows) = calculate_rows_columns(self.ui_triggers.len(), 1.0, 2.0);

        // Define the grid for the widgets
        let cells = create_grid(inside_block, rows, columns);

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

        // Calculate the number of rows/columns based on the number of
        // ui elements.
        let (columns, rows) = calculate_rows_columns(self.ui_axes.len(), 2.0, 1.0);

        // Define the grid for the buttons
        let cells = create_grid(inside_block, rows, columns);

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

        // Gyro has full panel width, so keep every gauge in a single row.
        let cells = create_grid(inside_block, 1, self.ui_gyro.len() as u16);

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

        // Calculate the number of rows/columns based on the number of
        // ui elements.
        let (columns, rows) = calculate_rows_columns(self.ui_touch.len(), 2.0, 1.0);

        // Define the grid for the widgets
        let cells = create_grid(inside_block, rows, columns);

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
                let label = capability_label(cap.capability);

                match cap.value_type {
                    ValueType::None => (),
                    ValueType::Bool => {
                        let button = ButtonGauge::new(cap.capability, &label);
                        self.ui_buttons.push(button);
                    }
                    ValueType::UInt8 | ValueType::UInt16 => {
                        let trigger = TriggerGauge::new(cap.capability, &label);
                        self.ui_triggers.push(trigger);
                    }
                    ValueType::UInt32 => (),
                    ValueType::UInt64 => (),
                    ValueType::Int8 => (),
                    ValueType::Int16 => (),
                    ValueType::Int32 => (),
                    ValueType::Int64 => (),
                    ValueType::UInt8Vector2 => (),
                    ValueType::UInt16Vector2 => match cap.capability {
                        InputCapability::GamepadAxisLeftStick
                        | InputCapability::GamepadAxisRightStick => {
                            let gauge = AxisGauge::new(cap.capability, &label);
                            self.ui_axes.push(gauge);
                        }
                        // Assume touch for everything else
                        _ => {
                            let gauge = TouchGauge::new(cap.capability, &label);
                            self.ui_touch.push(gauge);
                        }
                    },
                    ValueType::UInt32Vector2 => (),
                    ValueType::UInt64Vector2 => (),
                    ValueType::Int8Vector2 => (),
                    ValueType::Int16Vector2 => (),
                    ValueType::Int32Vector2 => (),
                    ValueType::Int64Vector2 => (),
                    ValueType::UInt8Vector3 => (),
                    ValueType::UInt16Vector3 => (),
                    ValueType::UInt32Vector3 => (),
                    ValueType::UInt64Vector3 => (),
                    ValueType::Int8Vector3 => (),
                    ValueType::Int16Vector3 => {
                        let gauge = GyroGauge::new(cap.capability, &label);
                        self.ui_gyro.push(gauge);
                    }
                    ValueType::Int32Vector3 => (),
                    ValueType::Int64Vector3 => (),
                    ValueType::Touch => {
                        let gauge = TouchGauge::new(cap.capability, &label);
                        self.ui_touch.push(gauge);
                    }
                }
            }

            // Sort each widget by their capability
            self.ui_buttons.sort_by_key(|widget| widget.sort_value());
            self.ui_triggers.sort_by_key(|widget| widget.sort_value());
            self.ui_axes.sort_by_key(|widget| widget.sort_value());
            self.ui_gyro.sort_by_key(|widget| widget.sort_value());
            self.ui_touch.sort_by_key(|widget| widget.sort_value());
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
            let label = capability_label(cap.capability);

            match value {
                Value::None => (),
                Value::Bool(value) => {
                    let mut button = ButtonGauge::new(cap.capability, &label);
                    button.set_value(value.value);
                    self.ui_buttons.push(button);
                }
                Value::UInt8(value) => {
                    let mut trigger = TriggerGauge::new(cap.capability, &label);
                    trigger.set_value(value.value as f64 / u8::MAX as f64);
                    self.ui_triggers.push(trigger);
                }
                Value::UInt16(value) => {
                    let mut trigger = TriggerGauge::new(cap.capability, &label);
                    trigger.set_value(value.value as f64 / u16::MAX as f64);
                    self.ui_triggers.push(trigger);
                }
                Value::UInt16Vector2(value) => match cap.capability {
                    InputCapability::GamepadAxisLeftStick
                    | InputCapability::GamepadAxisRightStick => {
                        let mut gauge = AxisGauge::new(cap.capability, &label);
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
                        let mut gauge = TouchGauge::new(cap.capability, &label);
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
                    let mut gauge = GyroGauge::new(cap.capability, &label);
                    //let x = value.x / i16::MAX;
                    //let y = value.y / i16::MAX;
                    //let z = value.z / i16::MAX;
                    //gauge.set_value(x as f64, y as f64, z as f64);
                    gauge.set_value(value.x as f64, value.y as f64, value.z as f64);
                    self.ui_gyro.push(gauge);
                }
                Value::Touch(value) => {
                    let mut gauge = TouchGauge::new(cap.capability, &label);
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

        // Sort each widget by their capability
        self.ui_buttons.sort_by_key(|widget| widget.sort_value());
        self.ui_triggers.sort_by_key(|widget| widget.sort_value());
        self.ui_axes.sort_by_key(|widget| widget.sort_value());
        self.ui_gyro.sort_by_key(|widget| widget.sort_value());
        self.ui_touch.sort_by_key(|widget| widget.sort_value());

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
            .constraints(vec![Constraint::Percentage(28), Constraint::Percentage(72)])
            .split(area);

        // Top layout
        let top_layout = outer_layout[0];
        self.render_buttons(top_layout, buf);

        // Bottom layout
        // Split vertically: top row (axes/triggers/touch) and gyro at full width
        let bottom_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(outer_layout[1]);

        // Top row: axes, triggers, and touch share the same height
        let top_row_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(bottom_layout[0]);
        self.render_axes(top_row_layout[0], buf);
        self.render_triggers(top_row_layout[1], buf);
        self.render_touch(top_row_layout[2], buf);

        // Gyro at full width
        self.render_gyro(bottom_layout[1], buf);
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

/// Calculates the number of rows/columns that can fit the given number of
/// elements in a grid of the given aspect ratio. The aspect ratio should be
/// two floats that describe the aspect ratio; i.e. (4.0, 3.0) would be a 4:3
/// aspect ratio. Returns the width and height of the grid: (w, h).
fn calculate_rows_columns(element_count: usize, width_ratio: f64, height_ratio: f64) -> (u16, u16) {
    if element_count == 0 {
        return (1, 1);
    }

    // Pick a row count that keeps the grid close to the target aspect ratio,
    // then use just enough columns to fit every element with minimal waste.
    let target_ratio = width_ratio / height_ratio;
    let rows = ((element_count as f64 / target_ratio).sqrt().round() as usize).max(1);
    let columns = element_count.div_ceil(rows);

    (columns as u16, rows as u16)
}
