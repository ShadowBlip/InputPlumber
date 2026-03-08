//! Multicolor LED source device.
//!
//! Multicolor LEDs use the kernel `multi_intensity` sysfs interface with
//! separate color channels (e.g. `multi_index` contains `red green blue`).
//! They share the same driver implementation as RGB LEDs since both use
//! the same sysfs interface — the [`LedRgb`](super::rgb::LedRgb) driver
//! handles both encodings via the `multi_index_map`.

pub use super::rgb::LedRgb as LedMultiColor;
