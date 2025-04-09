pub const REPORT_DESCRIPTOR: [u8; 125] = [
    0x05, 0x01,                    // Usage Page (Generic Desktop)        0
    0x09, 0x05,                    // Usage (Game Pad)                    2
    0xa1, 0x01,                    // Collection (Application)            4
    0x75, 0x08,                    //  Report Size (8)                    6
    0x95, 0x04,                    //  Report Count (4)                   8
    0x15, 0x00,                    //  Logical Minimum (0)                10
    0x26, 0xff, 0x00,              //  Logical Maximum (255)              12
    0x35, 0x00,                    //  Physical Minimum (0)               15
    0x46, 0xff, 0x00,              //  Physical Maximum (255)             17
    0x09, 0x30,                    //  Usage (X)                          20
    0x09, 0x31,                    //  Usage (Y)                          22
    0x09, 0x32,                    //  Usage (Z)                          24
    0x09, 0x35,                    //  Usage (Rz)                         26
    0x81, 0x02,                    //  Input (Data,Var,Abs)               28
    0x75, 0x04,                    //  Report Size (4)                    30
    0x95, 0x01,                    //  Report Count (1)                   32
    0x25, 0x07,                    //  Logical Maximum (7)                34
    0x46, 0x3b, 0x01,              //  Physical Maximum (315)             36
    0x65, 0x14,                    //  Unit (EnglishRotation: deg)        39
    0x09, 0x39,                    //  Usage (Hat switch)                 41
    0x81, 0x42,                    //  Input (Data,Var,Abs,Null)          43
    0x65, 0x00,                    //  Unit (None)                        45
    0x25, 0x01,                    //  Logical Maximum (1)                47
    0x45, 0x01,                    //  Physical Maximum (1)               49
    0x05, 0x0c,                    //  Usage Page (Consumer Devices)      51
    0x09, 0x69,                    //  Usage (Red Menu Button)            53
    0x75, 0x01,                    //  Report Size (1)                    55
    0x95, 0x01,                    //  Report Count (1)                   57
    0x81, 0x02,                    //  Input (Data,Var,Abs)               59
    0x05, 0x09,                    //  Usage Page (Button)                61
    0x09, 0x14,                    //  Usage (Vendor Usage 0x14)          63
    0x09, 0x13,                    //  Usage (Vendor Usage 0x13)          65
    0x09, 0x12,                    //  Usage (Vendor Usage 0x12)          67
    0x09, 0x11,                    //  Usage (Vendor Usage 0x11)          69
    0x09, 0x10,                    //  Usage (Vendor Usage 0x10)          71
    0x09, 0x0f,                    //  Usage (Vendor Usage 0x0f)          73
    0x09, 0x0e,                    //  Usage (Vendor Usage 0x0e)          75
    0x09, 0x0c,                    //  Usage (Vendor Usage 0x0c)          77
    0x09, 0x0b,                    //  Usage (Vendor Usage 0x0b)          79
    0x09, 0x0a,                    //  Usage (Vendor Usage 0x0a)          81
    0x09, 0x09,                    //  Usage (Vendor Usage 0x09)          83
    0x09, 0x08,                    //  Usage (Vendor Usage 0x08)          85
    0x09, 0x07,                    //  Usage (Vendor Usage 0x07)          87
    0x09, 0x06,                    //  Usage (Vendor Usage 0x06)          89
    0x09, 0x05,                    //  Usage (Vendor Usage 0x05)          91
    0x09, 0x04,                    //  Usage (Vendor Usage 0x04)          93
    0x09, 0x03,                    //  Usage (Vendor Usage 0x03)          95
    0x09, 0x02,                    //  Usage (Vendor Usage 0x02)          97
    0x09, 0x01,                    //  Usage (Vendor Usage 0x01)          99
    0x75, 0x01,                    //  Report Size (1)                    101
    0x95, 0x13,                    //  Report Count (19)                  103
    0x81, 0x02,                    //  Input (Data,Var,Abs)               105
    0x05, 0x02,                    //  Usage Page (Simulation Controls)   107
    0x15, 0x00,                    //  Logical Minimum (0)                109
    0x26, 0xff, 0x00,              //  Logical Maximum (255)              111
    0x09, 0xc5,                    //  Usage (Brake)                      114
    0x09, 0xc4,                    //  Usage (Accelerator)                116
    0x95, 0x02,                    //  Report Count (2)                   118
    0x75, 0x08,                    //  Report Size (8)                    120
    0x81, 0x02,                    //  Input (Data,Var,Abs)               122
    0xc0,                          // End Collection                      124
];
