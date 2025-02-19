use ratatui::{
    prelude::*,
    widgets::{
        canvas::{Canvas, Circle},
        Block, Widget,
    },
};

#[derive(Debug, Default)]
pub struct TouchGauge {
    text: String,
    x: f64,
    y: f64,
    touching: bool,
}

impl TouchGauge {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            x: 0.0,
            y: 0.0,
            touching: false,
        }
    }

    pub fn set_value(&mut self, x: f64, y: f64, touching: bool) {
        self.x = x;
        self.y = y;
        self.touching = touching;
    }
}

impl Widget for &TouchGauge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let canvas = Canvas::default()
            .block(Block::bordered().title(self.text.as_str()))
            .marker(ratatui::symbols::Marker::Braille)
            .x_bounds([0.0, 100.0])
            .y_bounds([0.0, 100.0])
            .paint(|ctx| {
                let x = self.x;
                let y = (1.0 - self.y).abs();

                // Draw the current position
                if self.touching {
                    for radius in 0..10 {
                        let cursor = Circle {
                            x: x * 100.0,
                            y: y * 100.0,
                            radius: radius as f64,
                            color: Color::LightRed,
                        };
                        ctx.draw(&cursor);
                    }
                }

                // Draw the coordinates
                ctx.print(
                    50.0,
                    50.0,
                    format!("({}, {})", (x * 100.0).round(), (y * 100.0).round()),
                );
            });
        canvas.render(area, buf);
    }
}
