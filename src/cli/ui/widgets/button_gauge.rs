use ratatui::{
    prelude::*,
    style::Style,
    symbols::border,
    widgets::{Block, Gauge, Widget},
};

use crate::drivers::unified_gamepad::capability::InputCapability;

#[derive(Debug, Default)]
pub struct ButtonGauge {
    text: String,
    capability: InputCapability,
    value: bool,
}

impl ButtonGauge {
    pub fn new(capability: InputCapability, text: &str) -> Self {
        Self {
            text: text.to_string(),
            capability,
            value: false,
        }
    }

    pub fn set_value(&mut self, value: bool) {
        self.value = value;
    }

    pub fn sort_value(&self) -> u32 {
        self.capability as u32
    }
}

impl Widget for &ButtonGauge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Choose the style based on the value
        let style = if self.value {
            Style::new().green()
        } else {
            Style::new().gray()
        };

        // Create a block
        let block = Block::bordered()
            .title(self.text.as_str())
            .border_set(border::ROUNDED)
            .border_style(style);
        let inside_block = block.inner(area);
        block.render(area, buf);

        // Create the gauge
        let color = if self.value {
            Color::Indexed(93)
        } else {
            Color::Indexed(105)
        };
        let gauge = Gauge::default().ratio(1.0).gauge_style(color).label("");
        gauge.render(inside_block, buf);
    }
}
