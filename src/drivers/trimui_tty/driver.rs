//! Serial transport for TrimUI Smart Pro S MCUs: read 19-byte frames off
//! the UART and decode them into [`Event`]s. Calibration and capability
//! translation live in the source-device adapter, not here.

use std::{error::Error, io::Read, time::Duration};

use serialport::{DataBits, Parity, StopBits, TTYPort};

use crate::drivers::trimui_tty::{
    event::Event, serial_report::FrameParser, TrimuiSide, BAUD, TTY_TIMEOUT,
};

pub struct Driver {
    port: TTYPort,
    parser: FrameParser,
    side: TrimuiSide,
    previous_buttons: u32,
}

impl Driver {
    pub fn new(devnode: &str, side: TrimuiSide) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let port = serialport::new(devnode, BAUD)
            .data_bits(DataBits::Eight)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .timeout(Duration::from_millis(TTY_TIMEOUT));
        let port = TTYPort::open(&port)?;
        log::info!("Started TrimUI TTY driver on {devnode} ({side:?}).");
        Ok(Self {
            port,
            parser: FrameParser::new(),
            side,
            previous_buttons: 0,
        })
    }

    /// Poll the UART and decode input events.
    pub fn poll(&mut self) -> Result<Vec<Event>, Box<dyn Error + Send + Sync>> {
        let mut buf = [0u8; 128];
        let count = match self.port.read(&mut buf) {
            Ok(count) => count,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => return Ok(Vec::new()),
            Err(e) => return Err(format!("TrimUI UART read failed: {e}").into()),
        };
        let mut events = Vec::new();
        for frame in self.parser.push(&buf[..count]) {
            let buttons = frame.buttons();
            let changed = buttons ^ self.previous_buttons;
            self.previous_buttons = buttons;
            if changed != 0 {
                events.push(Event::Buttons { buttons, changed });
            }
            let (x, y) = match self.side {
                TrimuiSide::Left => frame.left_axes(),
                TrimuiSide::Right => frame.right_axes(),
            };
            events.push(Event::Stick { x, y });
        }
        Ok(events)
    }
}
