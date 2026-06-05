// HID Report Descriptor for 8BitDo Ultimate 2 Wireless (DInput mode emulation).
//
// This descriptor is crafted to satisfy Linux kernel HID parsing requirements
// while matching the 34-byte input report format used by the real device.
// Steam identifies the device by VID/PID and parses hidraw data directly
// using fixed byte offsets (from SDL_hidapi_8bitdo.c), so descriptor field
// semantics do not need to be pixel-perfect.
//
// Report layout (payload bytes, excluding the 1-byte report ID prefix):
//   Report ID 0x04 - Input  (33 bytes payload = 34 bytes total)
//   Report ID 0x05 - Output ( 4 bytes payload =  5 bytes total, rumble command)

pub const REPORT_DESCRIPTOR: [u8; 40] = [
    0x05, 0x01,        // Usage Page (Generic Desktop)
    0x09, 0x05,        // Usage (Game Pad)
    0xa1, 0x01,        // Collection (Application)

    // Input report: ID=0x04, 33 bytes via Vendor Defined usage
    0x85, 0x04,        //   Report ID (4)
    0x06, 0x00, 0xff,  //   Usage Page (Vendor Defined 0xFF00)
    0x09, 0x20,        //   Usage (Vendor Usage 0x20)
    0x15, 0x00,        //   Logical Minimum (0)
    0x26, 0xff, 0x00,  //   Logical Maximum (255)
    0x75, 0x08,        //   Report Size (8 bits)
    0x95, 0x21,        //   Report Count (33)  → 33 bytes
    0x81, 0x02,        //   Input (Data, Variable, Absolute)

    // Output report: ID=0x05, 4 bytes (rumble: [low_freq, high_freq, 0x00, 0x00])
    0x85, 0x05,        //   Report ID (5)
    0x09, 0x21,        //   Usage (Vendor Usage 0x21)
    0x15, 0x00,        //   Logical Minimum (0)
    0x26, 0xff, 0x00,  //   Logical Maximum (255)
    0x75, 0x08,        //   Report Size (8 bits)
    0x95, 0x04,        //   Report Count (4)   → 4 bytes
    0x91, 0x02,        //   Output (Data, Variable, Absolute)

    0xc0,              // End Collection
];
