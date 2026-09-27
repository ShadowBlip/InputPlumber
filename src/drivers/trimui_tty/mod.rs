pub mod driver;
pub mod event;
pub mod serial_report;

pub const LEFT_UART_PATH: &str = "/2501400.uart/";
pub const RIGHT_UART_PATH: &str = "/2501c00.uart/";
pub const BAUD: u32 = 19_200;
pub const TTY_TIMEOUT: u64 = 10;

/// UART address in the syspath so the `ttyAS0` console (or anything else)
/// can never be claimed, even if `ttyAS*` numbering shifts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrimuiSide {
    Left,
    Right,
}

/// Resolve the pad half from the tty syspath.
pub fn side_from_syspath(syspath: &str) -> Option<TrimuiSide> {
    if syspath.contains(LEFT_UART_PATH) {
        Some(TrimuiSide::Left)
    } else if syspath.contains(RIGHT_UART_PATH) {
        Some(TrimuiSide::Right)
    } else {
        None
    }
}
