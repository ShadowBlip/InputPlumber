pub const REPORT_DESCRIPTOR: [u8; 140] = [
    0x05, 0x01, // Usage Page (Generic Desktop)        0
    0x09, 0x05, // Usage (Game Pad)                    2
    0xa1, 0x01, // Collection (Application)            4
    0x85, 0x07, //  Report ID (7)                      6
    0xa1, 0x00, //  Collection (Physical)              8
    0x09, 0x30, //   Usage (X)                         10
    0x09, 0x31, //   Usage (Y)                         12
    0x09, 0x32, //   Usage (Z)                         14
    0x09, 0x35, //   Usage (Rz)                        16
    0x15, 0x00, //   Logical Minimum (0)               18
    0x26, 0xff, 0x00, //   Logical Maximum (255)       20
    0x75, 0x08, //   Report Size (8)                   23
    0x95, 0x04, //   Report Count (4)                  25
    0x81, 0x02, //   Input (Data,Var,Abs)              27
    0xc0, //  End Collection                           29
    0x09, 0x39, //  Usage (Hat switch)                 30
    0x15, 0x00, //  Logical Minimum (0)                32
    0x25, 0x07, //  Logical Maximum (7)                34
    0x35, 0x00, //  Physical Minimum (0)               36
    0x46, 0x3b, 0x01, //  Physical Maximum (315)       38
    0x65, 0x14, //  Unit (EnglishRotation: deg)        41
    0x75, 0x04, //  Report Size (4)                    43
    0x95, 0x01, //  Report Count (1)                   45
    0x81, 0x42, //  Input (Data,Var,Abs,Null)          47
    0x05, 0x09, //  Usage Page (Button)                49
    0x19, 0x01, //  Usage Minimum (1)                  51
    0x29, 0x14, //  Usage Maximum (20)                 53
    0x15, 0x00, //  Logical Minimum (0)                55
    0x25, 0x01, //  Logical Maximum (1)                57
    0x75, 0x01, //  Report Size (1)                    59
    0x95, 0x14, //  Report Count (20)                  61
    0x81, 0x02, //  Input (Data,Var,Abs)               63
    0x05, 0x02, //  Usage Page (Simulation Controls)   65
    0x15, 0x00, //  Logical Minimum (0)                67
    0x26, 0xff, 0x00, //  Logical Maximum (255)        69
    0x09, 0xc4, //  Usage (Accelerator)                72
    0x09, 0xc5, //  Usage (Brake)                      74
    0x95, 0x02, //  Report Count (2)                   76
    0x75, 0x08, //  Report Size (8)                    78
    0x81, 0x02, //  Input (Data,Var,Abs)               80
    0x06, 0x00, 0xff, //  Usage Page (Vendor Defined)  82
    0x09, 0x20, //  Usage (Vendor Usage 0x20)          85
    0x95, 0x26, //  Report Count (38)                  87
    0x81, 0x02, //  Input (Data,Var,Abs)               89
    0x85, 0x05, //  Report ID (5)                      91
    0x09, 0x21, //  Usage (Vendor Usage 0x21)          93
    0x95, 0x20, //  Report Count (32)                  95
    0x91, 0x02, //  Output (Data,Var,Abs)              97
    0x85, 0x12, //  Report ID (18)                     99
    0x09, 0x22, //  Usage (Vendor Usage 0x22)          101
    0x95, 0x3f, //  Report Count (63)                  103
    0x81, 0x02, //  Input (Data,Var,Abs)               105
    0x09, 0x23, //  Usage (Vendor Usage 0x23)          107
    0x91, 0x02, //  Output (Data,Var,Abs)              109
    0x85, 0x14, //  Report ID (20)                     111
    0x09, 0x26, //  Usage (Vendor Usage 0x26)          113
    0x95, 0x3f, //  Report Count (63)                  115
    0x81, 0x02, //  Input (Data,Var,Abs)               117
    0x09, 0x27, //  Usage (Vendor Usage 0x27)          119
    0x91, 0x02, //  Output (Data,Var,Abs)              121
    0x85, 0x10, //  Report ID (16)                     123
    0x09, 0x24, //  Usage (Vendor Usage 0x24)          125
    0x95, 0x3f, //  Report Count (63)                  127
    0x81, 0x02, //  Input (Data,Var,Abs)               129
    0x85, 0x0f, //  Report ID (15)                     131
    0x09, 0x28, //  Usage (Vendor Usage 0x28)          133
    0x95, 0x3f, //  Report Count (63)                  135
    0x91, 0x02, //  Output (Data,Var,Abs)              137
    0xc0, // End Collection                            139
];
