use ratatui::{
    prelude::*,
    widgets::{
        canvas::{Canvas, Circle},
        Block, Widget,
    },
};

use crate::drivers::unified_gamepad::capability::InputCapability;

#[derive(Debug, Default)]
pub struct AxisGauge {
    capability: InputCapability,
    text: String,
    x: f64,
    y: f64,
}

impl AxisGauge {
    pub fn new(capability: InputCapability, text: &str) -> Self {
        Self {
            text: text.to_string(),
            capability,
            x: 0.0,
            y: 0.0,
        }
    }

    pub fn set_value(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
    }

    pub fn sort_value(&self) -> u32 {
        self.capability as u32
    }
}

impl Widget for &AxisGauge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let canvas = Canvas::default()
            .block(Block::bordered().title(self.text.as_str()))
            .marker(ratatui::symbols::Marker::Braille)
            .x_bounds([-100.0, 100.0])
            .y_bounds([-100.0, 100.0])
            .paint(|ctx| {
                // Draw the edges
                let circle = Circle {
                    radius: 100.0,
                    ..Default::default()
                };
                ctx.draw(&circle);

                // Draw the current position
                for radius in 0..10 {
                    let cursor = Circle {
                        x: self.x * 100.0,
                        y: self.y * 100.0,
                        radius: radius as f64,
                        color: Color::LightRed,
                    };
                    ctx.draw(&cursor);
                }

                // Draw the coordinates
                ctx.print(
                    0.0,
                    0.0,
                    format!(
                        "({}, {})",
                        (self.x * 100.0).round(),
                        (self.y * 100.0).round()
                    ),
                );
            });
        canvas.render(area, buf);
    }
}
