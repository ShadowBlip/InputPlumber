pub const DS5_EDGE_NAME: &str = "Sony Interactive Entertainment DualSense Edge Wireless Controller";
pub const DS5_EDGE_VERSION: u16 = 256;
pub const DS5_EDGE_VID: u16 = 0x054C;
pub const DS5_EDGE_PID: u16 = 0x0DF2;

pub const DS5_NAME: &str = "Sony Interactive Entertainment DualSense Wireless Controller";
pub const DS5_VERSION: u16 = 0x8111;
pub const DS5_VID: u16 = 0x054C;
pub const DS5_PID: u16 = 0x0ce6;

pub const FEATURE_REPORT_PAIRING_IFO: u8 = 0x09;
pub const FEATURE_REPORT_PAIRING_INFO_SIZE: u8 = 20;
pub const FEATURE_REPORT_FIRMWARE_INFO: u8 = 0x20;
pub const FEATURE_REPORT_FIRMWARE_INFO_SIZE: u8 = 64;
pub const FEATURE_REPORT_CALIBRATION: u8 = 0x05;
pub const FEATURE_REPORT_CALIBRATION_SIZE: u8 = 41;

pub const INPUT_REPORT_USB: u8 = 0x01;
pub const INPUT_REPORT_USB_SIZE: u8 = 64;
pub const INPUT_REPORT_BT: u8 = 0x31;
pub const INPUT_REPORT_BT_SIZE: u8 = 78;
pub const OUTPUT_REPORT_USB: u8 = 0x02;
pub const OUTPUT_REPORT_USB_SIZE: u8 = 63;
pub const OUTPUT_REPORT_BT: u8 = 0x31;
pub const OUTPUT_REPORT_BT_SIZE: u8 = 78;

pub const OUTPUT_VALID_FLAG0_HAPTICS_SELECT: u8 = 0x02;
pub const OUTPUT_VALID_FLAG2_COMPATIBLE_VIBRATION2: u8 = 0x04;
pub const OUTPUT_VALID_FLAG0_COMPATIBLE_VIBRATION: u8 = 0x01;
pub const OUTPUT_VALID_FLAG1_LIGHTBAR_CONTROL_ENABLE: u8 = 0x04;

//#define DS5_SPEC_DELTA_TIME         4096.0f