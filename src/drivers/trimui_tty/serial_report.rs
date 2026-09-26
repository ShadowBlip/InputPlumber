//! TrimUI Smart Pro S MCU frame protocol (ported from trimui-inputrs).
//!
//! Each MCU speaks 19200 8N1 with 19-byte `ff ... fe` frames: a
//! little-endian u32 button bitmap at bytes [2..6] and 12-bit stick axes.

pub const FRAME_LEN: usize = 19;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frame {
    pub raw: [u8; FRAME_LEN],
}

impl Frame {
    pub fn parse(raw: [u8; FRAME_LEN]) -> Option<Self> {
        (raw[0] == 0xff && raw[18] == 0xfe).then_some(Self { raw })
    }

    pub fn buttons(self) -> u32 {
        u32::from_le_bytes(self.raw[2..6].try_into().unwrap())
    }

    pub fn le16(self, offset: usize) -> u16 {
        u16::from_le_bytes(self.raw[offset..offset + 2].try_into().unwrap())
    }

    pub fn left_axes(self) -> (u16, u16) {
        (self.le16(6), self.le16(8))
    }

    pub fn right_axes(self) -> (u16, u16) {
        (self.le16(10), self.le16(12))
    }
}

/// Button bitmap bit assignments.
pub const A: u32 = 1 << 0;
pub const X: u32 = 1 << 1;
pub const SELECT: u32 = 1 << 2;
pub const START: u32 = 1 << 3;
pub const DPAD_UP: u32 = 1 << 4;
pub const DPAD_DOWN: u32 = 1 << 5;
pub const DPAD_LEFT: u32 = 1 << 6;
pub const DPAD_RIGHT: u32 = 1 << 7;
pub const B: u32 = 1 << 8;
pub const Y: u32 = 1 << 9;
pub const L1: u32 = 1 << 10;
pub const R1: u32 = 1 << 11;
pub const L2: u32 = 1 << 12;
pub const R2: u32 = 1 << 13;
pub const L3: u32 = 1 << 14;
pub const R3: u32 = 1 << 15;
pub const MODE: u32 = 1 << 16;

#[derive(Debug, Default)]
pub struct FrameParser {
    buffer: Vec<u8>,
}

impl FrameParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<Frame> {
        self.buffer.extend_from_slice(bytes);
        let mut frames = Vec::new();
        loop {
            let Some(start) = self.buffer.iter().position(|&b| b == 0xff) else {
                self.buffer.clear();
                break;
            };
            if start != 0 {
                self.buffer.drain(..start);
            }
            if self.buffer.len() < FRAME_LEN {
                break;
            }
            let mut raw = [0u8; FRAME_LEN];
            raw.copy_from_slice(&self.buffer[..FRAME_LEN]);
            if let Some(frame) = Frame::parse(raw) {
                frames.push(frame);
                self.buffer.drain(..FRAME_LEN);
            } else {
                self.buffer.drain(..1);
            }
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> [u8; FRAME_LEN] {
        [
            0xff, 0x42, 0x11, 0x44, 0x01, 0x80, 0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a, 0xf0, 0xde,
            0xff, 0xfe, 0x0f, 0x1f, 0xfe,
        ]
    }

    #[test]
    fn parser_handles_every_split() {
        for split in 0..=FRAME_LEN {
            let mut parser = FrameParser::new();
            let bytes = frame();
            let mut got = parser.push(&bytes[..split]);
            got.extend(parser.push(&bytes[split..]));
            assert_eq!(got.len(), 1);
            assert_eq!(got[0].raw, bytes);
        }
    }

    #[test]
    fn decoder_uses_le_buttons_and_side_axis_offsets() {
        let f = Frame { raw: frame() };
        assert_eq!(f.buttons(), 0x8001_4411);
        assert_eq!(f.left_axes(), (0x1234, 0x5678));
        assert_eq!(f.right_axes(), (0x9abc, 0xdef0));
    }
}
