// HID Report Descriptor for 8BitDo Ultimate 2 Wireless (DInput mode emulation).
//
// This is the real controller's own descriptor (captured via hid-recorder),
// used as-is. The previous hand-crafted descriptor represented the entire
// input payload as one opaque Vendor Defined blob with no standard Button/
// axis/Hat usages at all -- valid enough for hidraw, but the kernel's
// hid-input driver needs real usages to build an evdev/joystick device and
// for udev to tag it ID_INPUT_JOYSTICK=1, which is very likely what gates
// Steam's own device enumeration before it ever gets to SDL's fixed-byte-
// offset parsing. It also mismatched the report ID it declared for input
// (4) against the id our data actually uses (REPORT_ID_INPUT = 1).
//
// This descriptor's standard-usage byte layout (Report ID 1, then a 1-byte
// hat+padding nibble, 4 bytes of X/Y/Z/Rz, 2 bytes of Accelerator/Brake, 3
// bytes/24 bits of Button 1-24, then a 23-byte vendor-defined blob) lines up
// byte-for-byte with our own PackedInputDataReport layout (dpad, sticks,
// triggers, buttons, then accel/gyro/battery/timestamp/padding) -- 33 payload
// bytes + 1 report ID byte = 34 bytes total, matching REPORT_ID_INPUT's real
// data. The output report (Report ID 5, 4 bytes) matches PackedRumbleOutputReport
// unchanged.
//
// Report layout (payload bytes, excluding the 1-byte report ID prefix):
//   Report ID 0x01 - Input  (33 bytes payload = 34 bytes total)
//   Report ID 0x05 - Output ( 4 bytes payload =  5 bytes total, rumble command)

pub const REPORT_DESCRIPTOR: [u8; 113] = [
    0x05, 0x01, //  Usage Page (Generic Desktop)
    0x09, 0x05, //  Usage (Game Pad)
    0xa1, 0x01, //  Collection (Application)
    0x85, 0x01, //   Report ID (1)
    0x05, 0x01, //   Usage Page (Generic Desktop)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x07, //   Logical Maximum (7)
    0x46, 0x3b, 0x01, //   Physical Maximum (315)
    0x95, 0x01, //   Report Count (1)
    0x75, 0x04, //   Report Size (4)
    0x65, 0x14, //   Unit (EnglishRotation: deg)
    0x09, 0x39, //   Usage (Hat switch)
    0x81, 0x42, //   Input (Data,Var,Abs,Null)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x04, //   Report Count (4)
    0x81, 0x01, //   Input (Cnst,Arr,Abs)
    0x15, 0x00, //   Logical Minimum (0)
    0x26, 0xff, 0x00, //   Logical Maximum (255)
    0x09, 0x30, //   Usage (X)
    0x09, 0x31, //   Usage (Y)
    0x09, 0x32, //   Usage (Z)
    0x09, 0x35, //   Usage (Rz)
    0x95, 0x04, //   Report Count (4)
    0x75, 0x08, //   Report Size (8)
    0x81, 0x02, //   Input (Data,Var,Abs)
    0x05, 0x02, //   Usage Page (Simulation Controls)
    0x15, 0x00, //   Logical Minimum (0)
    0x26, 0xff, 0x00, //   Logical Maximum (255)
    0x09, 0xc4, //   Usage (Accelerator)
    0x09, 0xc5, //   Usage (Brake)
    0x95, 0x02, //   Report Count (2)
    0x75, 0x08, //   Report Size (8)
    0x81, 0x02, //   Input (Data,Var,Abs)
    0x05, 0x09, //   Usage Page (Button)
    0x19, 0x01, //   Usage Minimum (1)
    0x29, 0x18, //   Usage Maximum (24)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x18, //   Report Count (24)
    0x81, 0x02, //   Input (Data,Var,Abs)
    0x06, 0x00, 0xff, //   Usage Page (Vendor Defined Page 1)
    0x09, 0x20, //   Usage (Vendor Usage 0x20)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x17, //   Report Count (23)
    0x81, 0x02, //   Input (Data,Var,Abs)
    0x05, 0x0f, //   Usage Page (Vendor Usage Page 0x0f)
    0x09, 0x70, //   Usage (Vendor Usage 0x70)
    0x85, 0x05, //   Report ID (5)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x64, //   Logical Maximum (100)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x04, //   Report Count (4)
    0x91, 0x02, //   Output (Data,Var,Abs)
    0xc0, //  End Collection
];
