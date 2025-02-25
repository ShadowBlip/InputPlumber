use ratatui::{
    prelude::*,
    style::Style,
    symbols::border,
    widgets::{Block, Gauge, LineGauge, Widget},
};

enum Axis {
    X,
    Y,
    Z,
}

#[derive(Debug, Default)]
pub struct GyroGauge {
    text: String,
    x: f64,
    y: f64,
    z: f64,
}

impl GyroGauge {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn set_value(&mut self, x: f64, y: f64, z: f64) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    fn render_axis(&self, axis: Axis, area: Rect, buf: &mut Buffer) {
        let (label, value, color) = match axis {
            Axis::X => ("X Axis", self.x, Color::Indexed(55)),
            Axis::Y => ("Y Axis", self.y, Color::Indexed(56)),
            Axis::Z => ("Z Axis", self.z, Color::Indexed(57)),
        };

        // Create a block
        let block = Block::bordered()
            .title(label)
            .border_set(border::ROUNDED)
            .border_style(Style::new());
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Split the area to have a negative and positive gauge
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Fill(1), Constraint::Fill(1)])
            .split(inside_block);

        // Create the gauge for the negative side
        let neg_value = if value.is_sign_negative() {
            (value.abs() / i16::MAX as f64).min(1.0)
        } else {
            0.0
        };
        let negative_area = layout[0];
        let gauge = Gauge::default()
            .ratio((1.0 - neg_value).abs())
            .gauge_style(color)
            .reversed()
            .label("");
        gauge.render(negative_area, buf);

        // Create the gauge for the positive side
        let pos_value = if value.is_sign_positive() {
            (value / i16::MAX as f64).min(1.0)
        } else {
            0.0
        };
        let positive_area = layout[1];
        let gauge = Gauge::default()
            .ratio(pos_value)
            .gauge_style(color)
            .label(format!("{}", value));
        gauge.render(positive_area, buf);
    }
}

impl Widget for &GyroGauge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create a block
        let block = Block::bordered()
            .title(self.text.as_str())
            .border_set(border::ROUNDED)
            .border_style(Style::new());
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Create rows for each axis
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Fill(1),
            ])
            .split(inside_block);
        self.render_axis(Axis::X, rows[0], buf);
        self.render_axis(Axis::Y, rows[1], buf);
        self.render_axis(Axis::Z, rows[2], buf);
    }
}
