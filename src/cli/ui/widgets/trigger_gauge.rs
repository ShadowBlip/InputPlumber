use ratatui::{
    prelude::*,
    style::Style,
    symbols::border,
    widgets::{Block, Gauge, Widget},
};

#[derive(Debug, Default)]
pub struct TriggerGauge {
    text: String,
    value: f64,
}

impl TriggerGauge {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            value: 0.0,
        }
    }

    pub fn set_value(&mut self, value: f64) {
        if value > 1.0 {
            return;
        }
        self.value = value;
    }
}

impl Widget for &TriggerGauge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create a block
        let block = Block::bordered()
            .title(self.text.as_str())
            .border_set(border::ROUNDED)
            .border_style(Style::new());
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Set the color based on the value
        let color = {
            if self.value < 0.2 {
                Color::Indexed(53)
            } else if self.value < 0.4 {
                Color::Indexed(54)
            } else if self.value < 0.6 {
                Color::Indexed(55)
            } else if self.value < 0.8 {
                Color::Indexed(56)
            } else {
                Color::Indexed(57)
            }
        };

        // Create the gauge
        let gauge = Gauge::default().gauge_style(color).ratio(self.value);
        gauge.render(inside_block, buf);
    }
}
