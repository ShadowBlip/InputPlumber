use ratatui::{
    prelude::*,
    style::Style,
    symbols::border,
    widgets::{Block, Gauge, Widget},
};

#[derive(Debug, Default)]
pub struct ButtonGauge {
    text: String,
    value: bool,
}

impl ButtonGauge {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            value: false,
        }
    }

    pub fn set_value(&mut self, value: bool) {
        self.value = value;
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
