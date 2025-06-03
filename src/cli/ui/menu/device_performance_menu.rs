use std::{
    collections::{HashMap, VecDeque},
    error::Error,
    time::Duration,
};

use futures::StreamExt;
use ratatui::{
    crossterm::event::KeyCode,
    layout::{Constraint, Direction, Layout},
    prelude::*,
    symbols::border,
    widgets::{Axis, Block, Chart, Dataset, GraphType, List, ListDirection, ListItem, Widget},
};
use style::palette::tailwind::SLATE;
use tokio::sync::mpsc;
use zbus::{fdo::ObjectManagerProxy, Connection};

use crate::{
    cli::ui::InterfaceCommand,
    constants::{BUS_NAME, BUS_PREFIX},
    dbus::interface::performance::PerformanceInterfaceProxy,
    input::event::context::SerializedSpan,
};

use super::MenuWidget;

const NUM_SAMPLES: usize = 1000;
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;

/// Menu for testing an input device
#[derive(Debug)]
pub struct DevicePerformanceMenu {
    data: VecDeque<u64>,
    spans: VecDeque<Vec<(String, u64)>>,
    events: VecDeque<String>,
    conn: Connection,
    rx_disconnect: mpsc::Receiver<()>,
    rx_reports: mpsc::Receiver<(String, Vec<SerializedSpan>)>,
    device_path: String,
}

impl DevicePerformanceMenu {
    pub async fn new(conn: &Connection, dbus_path: &str) -> Result<Self, Box<dyn Error>> {
        // Get a reference to the device to debug
        let device = PerformanceInterfaceProxy::builder(conn)
            .path(dbus_path)?
            .build()
            .await?;

        // Enable metrics
        device.set_enabled(true).await?;

        // Create channels to listen for metrics
        let (tx_reports, rx_reports) = mpsc::channel(2048);
        let (tx_disconnect, rx_disconnect) = mpsc::channel(1);

        // Spawn a task to listen for metrics
        let conn_clone = conn.clone();
        let path_clone = dbus_path.to_string();
        tokio::task::spawn(async move {
            let _ =
                Self::listen_for_signals(&conn_clone, path_clone, tx_disconnect, tx_reports).await;
        });

        Ok(Self {
            data: VecDeque::with_capacity(NUM_SAMPLES),
            spans: VecDeque::with_capacity(NUM_SAMPLES),
            events: VecDeque::with_capacity(NUM_SAMPLES),
            conn: conn.clone(),
            device_path: dbus_path.to_string(),
            rx_disconnect,
            rx_reports,
        })
    }

    // Stop the test, restoring the device state
    fn stop(&self) {
        // Restore the state of the device
        let conn = self.conn.clone();
        let dbus_path = self.device_path.clone();
        tokio::task::spawn(async move {
            // Create a reference to the metrics interface
            let device = PerformanceInterfaceProxy::builder(&conn)
                .path(dbus_path)
                .unwrap()
                .build()
                .await
                .unwrap();

            // Disable metrics
            device.set_enabled(false).await.unwrap();
        });

        // Wait a beat
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

impl DevicePerformanceMenu {
    /// Listen for device signals over dbus
    async fn listen_for_signals(
        conn: &Connection,
        dbus_path: String,
        tx_disconnect: mpsc::Sender<()>,
        tx_reports: mpsc::Sender<(String, Vec<SerializedSpan>)>,
    ) -> Result<(), Box<dyn Error>> {
        // Get a reference to the device to debug
        let device = PerformanceInterfaceProxy::builder(conn)
            .path(dbus_path.as_str())?
            .build()
            .await?;

        // Listen for metrics
        let mut receive_report = device.receive_event_metrics().await?;
        tokio::task::spawn(async move {
            while let Some(signal) = receive_report.next().await {
                let Ok(args) = signal.args() else { break };
                let _ = tx_reports.send((args.capability, args.spans)).await;
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

    fn render_graph(&self, area: Rect, buf: &mut Buffer, data: &VecDeque<u64>) {
        // Create a block
        let block = Block::bordered()
            .title("Input Latency")
            .border_set(border::ROUNDED)
            .border_style(Style::new().red());
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Get the average from the data
        let avg = {
            let sum: u64 = data.iter().sum();
            if !data.is_empty() {
                sum / data.len() as u64
            } else {
                0
            }
        };
        let avg_latency = Duration::from_micros(avg);

        // Get the maximum value from the data
        let max = data.iter().max().cloned().unwrap_or(20000);
        let max_latency = Duration::from_micros(max);
        let mid = max / 2;
        let mid_latency = Duration::from_micros(mid);
        let min = data.iter().min().cloned().unwrap_or_default();
        let min_latency = Duration::from_micros(min);

        // Get the general unit to use in the legend
        let unit = if max > 1000 { "ms" } else { "µs" };

        // Convert the data into (x, y) values
        let data: Vec<(f64, f64)> = data
            .iter()
            .enumerate()
            .map(|(i, value)| (i as f64, *value as f64))
            .collect();

        // Create a dataset from the given data
        let dataset = Dataset::default()
            .name("Latency")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Bar)
            .data(data.as_slice());

        // Create the X axis and define its properties
        let latency_label =
            format!("Min: {min_latency:?}, Average: {avg_latency:?}, Max: {max_latency:?}");
        let x_axis = Axis::default()
            .title("Time".red())
            .style(Style::default().white())
            .bounds([0.0, NUM_SAMPLES as f64])
            .labels(["", latency_label.as_str(), ""]);

        // Create the Y axis and define its properties
        let min_label = format!("0{unit}");
        let mid_label = format!("{mid_latency:?}");
        let max_label = format!("{max_latency:?}");
        let y_axis = Axis::default()
            .title("Latency".red())
            .style(Style::default().white())
            .bounds([0.0, max as f64])
            .labels([min_label.as_str(), mid_label.as_str(), max_label.as_str()]);

        // Render the chart using the dataset
        let widget = Chart::new(vec![dataset]).x_axis(x_axis).y_axis(y_axis);
        widget.render(inside_block, buf);
    }

    fn render_events(&self, area: Rect, buf: &mut Buffer) {
        // Create a block
        let block = Block::bordered()
            .title("Events")
            .border_set(border::ROUNDED)
            .border_style(Style::new().red());
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .events
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let color = if i % 2 == 0 {
                    NORMAL_ROW_BG
                } else {
                    ALT_ROW_BG_COLOR
                };

                // Get the corresponding time information for the event
                let duration = self.data.get(i).cloned().unwrap_or_default();
                let duration = Duration::from_micros(duration);
                let text = format!("{item} {duration:?}");

                ListItem::from(text).bg(color)
            })
            .collect();

        // Create and render the list
        let widget = List::new(items).direction(ListDirection::BottomToTop);
        Widget::render(widget, inside_block, buf);
    }
}

impl MenuWidget for DevicePerformanceMenu {
    fn update(&mut self) -> Vec<InterfaceCommand> {
        if self.rx_reports.is_closed() || self.rx_disconnect.is_closed() {
            self.stop();
            return vec![InterfaceCommand::Quit];
        }

        // Check to see if the device has disconnected
        if !self.rx_disconnect.is_empty() {
            self.stop();
            return vec![InterfaceCommand::Quit];
        }

        // Check for any input metrics
        while !self.rx_reports.is_empty() {
            let Some((capability, spans)) = self.rx_reports.blocking_recv() else {
                self.stop();
                return vec![InterfaceCommand::Quit];
            };
            let root_span = spans.first().unwrap();
            let _parent_span_id = root_span.0.as_str();
            let _span_id = root_span.1.as_str();
            let duration = root_span.2;

            // Collect the name of the input event
            self.events.push_front(capability.clone());
            if self.events.len() > NUM_SAMPLES {
                self.events.pop_back();
            }

            // Collect the root span time from every capability
            self.data.push_front(duration);
            if self.data.len() > NUM_SAMPLES {
                self.data.pop_back();
            }

            // Collect the individual spans
            let spans: Vec<(String, u64)> = spans
                .into_iter()
                .map(|(_parent_id, id, duration)| (id, duration))
                .filter(|(id, _duration)| id.as_str() != "root")
                .collect();
            self.spans.push_front(spans);
            if self.spans.len() > NUM_SAMPLES {
                self.spans.pop_back();
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

impl Widget for &DevicePerformanceMenu {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create a block
        let block = Block::bordered()
            .title_bottom(Line::from("Press 'q' to quit").centered())
            .border_set(border::ROUNDED);
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Split the area into two parts vertically
        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inside_block);

        // Top layout
        let top_layout = outer_layout[0];
        self.render_graph(top_layout, buf, &self.data);

        // Bottom layout
        let bottom_layout = outer_layout[1];
        self.render_events(bottom_layout, buf);
    }
}
