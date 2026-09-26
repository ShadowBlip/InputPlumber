pub mod driver;
pub mod event;
pub mod serial_report;

/// UART5 MMIO address fragment identifying the left MCU port. Matched
/// against the tty syspath (e.g. `/devices/platform/soc@3000000/2501400.uart/tty/ttyAS5`),
/// which is stable across `ttyAS*` renumbering, unlike the sysname.
pub const LEFT_UART_PATH: &str = "/2501400.uart/";
/// UART7 MMIO address fragment identifying the right MCU port.
pub const RIGHT_UART_PATH: &str = "/2501c00.uart/";
/// TrimUI MCU line discipline: 19200 8N1.
pub const BAUD: u32 = 19_200;
pub const TTY_TIMEOUT: u64 = 10;

/// Which half of the pad a driver instance reads. Matched on the parent
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
