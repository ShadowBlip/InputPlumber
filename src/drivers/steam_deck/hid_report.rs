//! Source: https://gitlab.com/open-sd/opensd/-/blob/main/src/opensdd/drivers/gamepad/hid_reports.hpp
//! Source: https://github.com/torvalds/linux/blob/master/drivers/hid/hid-steam.c
#![allow(warnings)]
use packed_struct::prelude::*;

// Input report axis ranges
pub const STICK_X_MIN: f64 = -32767.0;
pub const STICK_X_MAX: f64 = 32767.0;
pub const STICK_Y_MIN: f64 = 32767.0; // Hardware uses an inverted y axis
pub const STICK_Y_MAX: f64 = -32767.0;
pub const STICK_FORCE_MAX: f64 = 112.0; // Weird number
pub const PAD_X_MIN: f64 = -32767.0;
pub const PAD_X_MAX: f64 = 32767.0;
pub const PAD_Y_MIN: f64 = 32767.0;
pub const PAD_Y_MAX: f64 = -32767.0;
pub const PAD_FORCE_MAX: f64 = 32767.0;
pub const TRIGG_MIN: f64 = 0.0;
pub const TRIGG_MAX: f64 = 32767.0;

// Precalculated axis multipliers
pub const STICK_X_AXIS_MULT: f64 = 1.0 / STICK_X_MAX;
pub const STICK_Y_AXIS_MULT: f64 = 1.0 / STICK_Y_MAX;
pub const STICK_FORCE_MULT: f64 = 1.0 / STICK_FORCE_MAX;
pub const PAD_X_AXIS_MULT: f64 = 1.0 / PAD_X_MAX;
pub const PAD_Y_AXIS_MULT: f64 = 1.0 / PAD_Y_MAX;
pub const PAD_X_SENS_MULT: f64 = 1.0 / 128.0;
pub const PAD_Y_SENS_MULT: f64 = 1.0 / 128.0;
pub const PAD_FORCE_MULT: f64 = 1.0 / PAD_FORCE_MAX;
pub const TRIGG_AXIS_MULT: f64 = 1.0 / TRIGG_MAX;

// Lengh of time for the thread to sleep before keyboard emulation
// has to be disabled again with a CLEAR_MAPPINGS report.
pub const LIZARD_SLEEP_SEC: f64 = 2.0;

/// Different reports types
pub enum ReportType {
    InputData = 0x09,
    SetMappings = 0x80,
    ClearMappings = 0x81,
    GetMappings = 0x82,
    GetAttrib = 0x83,
    GetAttribLabel = 0x84,
    DefaultMappings = 0x85,
    FactoryReset = 0x86,
    WriteRegister = 0x87,
    ClearRegister = 0x88,
    ReadRegister = 0x89,
    GetRegisterLabel = 0x8a,
    GetRegisterMax = 0x8b,
    GetRegisterDefault = 0x8c,
    SetMode = 0x8d,
    DefaultMouse = 0x8e,
    TriggerHapticPulse = 0x8f,
    RequestCommStatus = 0xb4,
    GetSerial = 0xae,
    TriggerHapticCommand = 0xea,
    TriggerRumbleCommand = 0xeb,
}

impl TryFrom<u8> for ReportType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x09 => Ok(Self::InputData),
            0x80 => Ok(Self::SetMappings),
            0x81 => Ok(Self::ClearMappings),
            0x82 => Ok(Self::GetMappings),
            0x83 => Ok(Self::GetAttrib),
            0x84 => Ok(Self::GetAttribLabel),
            0x85 => Ok(Self::DefaultMappings),
            0x86 => Ok(Self::FactoryReset),
            0x87 => Ok(Self::WriteRegister),
            0x88 => Ok(Self::ClearRegister),
            0x89 => Ok(Self::ReadRegister),
            0x8a => Ok(Self::GetRegisterLabel),
            0x8b => Ok(Self::GetRegisterMax),
            0x8c => Ok(Self::GetRegisterDefault),
            0x8d => Ok(Self::SetMode),
            0x8e => Ok(Self::DefaultMouse),
            0x8f => Ok(Self::TriggerHapticPulse),
            0xb4 => Ok(Self::RequestCommStatus),
            0xae => Ok(Self::GetSerial),
            0xea => Ok(Self::TriggerHapticCommand),
            0xeb => Ok(Self::TriggerRumbleCommand),
            _ => Err("Invalid report type"),
        }
    }
}

/// Register settings
pub enum Register {
    MouseSensitivity = 0x00,
    MouseAcceleration = 0x01,
    TrackballRotationAngle = 0x02,
    HapticIntensityUnused = 0x03,
    LeftGamepadStickEnabled = 0x04,
    RightGamepadStickEnabled = 0x05,
    UsbDebugMode = 0x06,
    LPadMode = 0x07,
    RPadMode = 0x08,
    MousePointerEnabled = 0x09,

    DPadDeadzone,
    MinimumMomentumVel,
    MomentumDecayAmmount,
    TrackpadRelativeModeTicksPerPixel,
    HapticIncrement,
    DPadAngleSin,
    DPadAngleCos,
    MomentumVerticalDivisor,
    MomentumMaximumVelocity,
    TrackpadZOn,

    TrackpadZOff,
    SensitivyScaleAmmount,
    LeftTrackpadSecondaryMode,
    RightTrackpadSecondaryMode,
    SmoothAbsoluteMouse,
    SteamButtonPowerOffTime,
    Unused1,
    TrackpadOuterRadius,
    TrackpadZOnLeft,
    TrackpadZOffLeft,

    TrackpadOuterSpinVel,
    TrackpadOuterSpinRadius,
    TrackpadOuterSpinHorizontalOnly,
    TrackpadRelativeModeDeadzone,
    TrackpadRelativeModeMaxVel,
    TrackpadRelativeModeInvertY,
    TrackpadDoubleTapBeepEnabled,
    TrackpadDoubleTapBeepPeriod,
    TrackpadDoubleTapBeepCount,
    TrackpadOuterRadiusReleaseOnTransition,

    RadialModeAngle,
    HapticIntensityMouseMode,
    LeftDPadRequiresClick,
    RightDPadRequiresClick,
    LedBaselineBrightness,
    LedUserBrightness,
    EnableRawJoystick,
    EnableFastScan,
    ImuMode,
    WirelessPacketVersion,

    SleepInactivityTimeout,
    TrackpadNoiseThreshold,
    LeftTrackpadClickPressure,
    RightTrackpadClickPressure,
    LeftBumperClickPressure,
    RightBumperClickPressure,
    LeftGripClickPressure,
    RightGripClickPressure,
    LeftGrip2ClickPressure,
    RightGrip2ClickPressure,

    PressureMode,
    ControllerTestMode,
    TriggerMode,
    TrackpadZThreshold,
    FrameRate,
    TrackpadFiltCtrl,
    TrackpadClip,
    DebugOutputSelect,
    TriggerThresholdPercent,
    TrackpadFrequencyHopping,

    HapticsEnabled,
    SteamWatchdogEnable,
    TimpTouchThresholdOn,
    TimpTouchThresholdOff,
    FreqHopping,
    TestControl,
    HapticMasterGainDb,
    ThumbTouchThresh,
    DevicePowerStatus,
    HapticIntensity,

    StabilizerEnabled,
    TimpModeMte,
}

/// Trackpad modes
pub enum TrackpadMode {
    AbsoluteMouse = 0x00,
    RelativeMouse = 0x01,
    DPadFourWayDiscrete = 0x02,
    DPadFourWayOverlap = 0x03,
    DPadEightWay = 0x04,
    RadialMode = 0x05,
    AbsoluteDPad = 0x06,
    None = 0x07,
    GestureKeyboard = 0x08,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct PackedInputDataReport {
    // byte 0-3
    #[packed_field(bytes = "0")]
    pub major_ver: u8, // Major version? Always 0x01
    #[packed_field(bytes = "1")]
    pub minor_ver: u8, // Minor version? Always 0x00
    #[packed_field(bytes = "2")]
    pub report_type: u8, // Report type? Always 0x09
    #[packed_field(bytes = "3")]
    pub report_size: u8, // Actual data length of report in bytes.  Always 64 for input reports.

    // byte 4-7
    #[packed_field(bytes = "4..=7", endian = "lsb")]
    pub frame: Integer<u32, packed_bits::Bits<32>>, // Input frame counter?

    // byte 8
    #[packed_field(bits = "64")]
    pub a: bool, // Button cluster
    #[packed_field(bits = "65")]
    pub x: bool,
    #[packed_field(bits = "66")]
    pub b: bool,
    #[packed_field(bits = "67")]
    pub y: bool,
    #[packed_field(bits = "68")]
    pub l1: bool, // Shoulder buttons
    #[packed_field(bits = "69")]
    pub r1: bool,
    #[packed_field(bits = "70")]
    pub l2: bool,
    #[packed_field(bits = "71")]
    pub r2: bool, // Binary sensor for analog triggers

    // byte 9
    #[packed_field(bits = "72")]
    pub l5: bool, // L5 & R5 on the back of the deck
    #[packed_field(bits = "73")]
    pub menu: bool, // Hamburger (☰) button located above right stick
    #[packed_field(bits = "74")]
    pub steam: bool, // STEAM button below left trackpad
    #[packed_field(bits = "75")]
    pub options: bool, // Overlapping square ⧉  button located above left stick
    #[packed_field(bits = "76")]
    pub down: bool,
    #[packed_field(bits = "77")]
    pub left: bool,
    #[packed_field(bits = "78")]
    pub right: bool,
    #[packed_field(bits = "79")]
    pub up: bool, // Directional Pad buttons

    // byte 10
    #[packed_field(bits = "80")]
    pub _unk4: bool,
    #[packed_field(bits = "81")]
    pub l3: bool, // Z-axis button on the left stick
    #[packed_field(bits = "82")]
    pub _unk3: bool,
    #[packed_field(bits = "83")]
    pub r_pad_touch: bool, // Binary "touch" sensor for trackpads
    #[packed_field(bits = "84")]
    pub l_pad_touch: bool,
    #[packed_field(bits = "85")]
    pub r_pad_press: bool, // Binary "press" sensor for trackpads
    #[packed_field(bits = "86")]
    pub l_pad_press: bool,
    #[packed_field(bits = "87")]
    pub r5: bool,

    // byte 11
    #[packed_field(bits = "88")]
    pub _unk11: bool,
    #[packed_field(bits = "89")]
    pub _unk10: bool,
    #[packed_field(bits = "90")]
    pub _unk9: bool,
    #[packed_field(bits = "91")]
    pub _unk8: bool,
    #[packed_field(bits = "92")]
    pub _unk7: bool,
    #[packed_field(bits = "93")]
    pub r3: bool, // Z-axis button on the right stick
    #[packed_field(bits = "94")]
    pub _unk6: bool,
    #[packed_field(bits = "95")]
    pub _unk5: bool,

    // byte 12
    #[packed_field(bits = "96")]
    pub _unk19: bool,
    #[packed_field(bits = "97")]
    pub _unk18: bool,
    #[packed_field(bits = "98")]
    pub _unk17: bool,
    #[packed_field(bits = "99")]
    pub _unk16: bool,
    #[packed_field(bits = "100")]
    pub _unk15: bool,
    #[packed_field(bits = "101")]
    pub _unk14: bool,
    #[packed_field(bits = "102")]
    pub _unk13: bool,
    #[packed_field(bits = "103")]
    pub _unk12: bool,

    // byte 13
    #[packed_field(bits = "104")]
    pub r_stick_touch: bool, // Binary touch sensors on the stick controls
    #[packed_field(bits = "105")]
    pub l_stick_touch: bool,
    #[packed_field(bits = "106")]
    pub _unk23: bool,
    #[packed_field(bits = "107")]
    pub _unk22: bool,
    #[packed_field(bits = "108")]
    pub _unk21: bool,
    #[packed_field(bits = "109")]
    pub r4: bool,
    #[packed_field(bits = "110")]
    pub l4: bool, // L4 & R4 on the back of the deck
    #[packed_field(bits = "111")]
    pub _unk20: bool,

    // byte 14
    #[packed_field(bits = "112")]
    pub _unk30: bool,
    #[packed_field(bits = "113")]
    pub _unk29: bool,
    #[packed_field(bits = "114")]
    pub _unk28: bool,
    #[packed_field(bits = "115")]
    pub _unk27: bool,
    #[packed_field(bits = "116")]
    pub _unk26: bool,
    #[packed_field(bits = "117")]
    pub quick_access: bool, // Quick Access (...) button below right trackpad
    #[packed_field(bits = "118")]
    pub _unk25: bool,
    #[packed_field(bits = "119")]
    pub _unk24: bool,

    // byte 15
    #[packed_field(bytes = "15")]
    pub _unk31: u8, // Not sure, maybe padding or just unused

    // byte 16-23
    #[packed_field(bytes = "16..=17", endian = "lsb")]
    pub l_pad_x: Integer<i16, packed_bits::Bits<16>>, // Trackpad touch coordinates
    #[packed_field(bytes = "18..=19", endian = "lsb")]
    pub l_pad_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "20..=21", endian = "lsb")]
    pub r_pad_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "22..=23", endian = "lsb")]
    pub r_pad_y: Integer<i16, packed_bits::Bits<16>>,

    // byte 24-29
    #[packed_field(bytes = "24..=25", endian = "lsb")]
    pub accel_x: Integer<i16, packed_bits::Bits<16>>, // Accelerometers
    #[packed_field(bytes = "26..=27", endian = "lsb")]
    pub accel_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "28..=29", endian = "lsb")]
    pub accel_z: Integer<i16, packed_bits::Bits<16>>,

    // byte 30-35
    #[packed_field(bytes = "30..=31", endian = "lsb")]
    pub pitch: Integer<i16, packed_bits::Bits<16>>, // Gyro
    #[packed_field(bytes = "32..=33", endian = "lsb")]
    pub yaw: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "34..=35", endian = "lsb")]
    pub roll: Integer<i16, packed_bits::Bits<16>>,

    // byte 36-43
    #[packed_field(bytes = "36..=37", endian = "lsb")]
    pub _magn_0: Integer<i16, packed_bits::Bits<16>>, // Magnetometer
    #[packed_field(bytes = "38..=39", endian = "lsb")]
    pub _magn_1: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "40..=41", endian = "lsb")]
    pub _magn_2: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "42..=43", endian = "lsb")]
    pub _magn_3: Integer<i16, packed_bits::Bits<16>>,

    // byte 44-47
    #[packed_field(bytes = "44..=45", endian = "lsb")]
    pub l_trigg: Integer<u16, packed_bits::Bits<16>>, // Pressure sensors for L2 & R2 triggers
    #[packed_field(bytes = "46..=47", endian = "lsb")]
    pub r_trigg: Integer<u16, packed_bits::Bits<16>>,

    // byte 48-55
    #[packed_field(bytes = "48..=49", endian = "lsb")]
    pub l_stick_x: Integer<i16, packed_bits::Bits<16>>, // Analogue thumbsticks
    #[packed_field(bytes = "50..=51", endian = "lsb")]
    pub l_stick_y: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "52..=53", endian = "lsb")]
    pub r_stick_x: Integer<i16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "54..=55", endian = "lsb")]
    pub r_stick_y: Integer<i16, packed_bits::Bits<16>>,

    // byte 56-59
    #[packed_field(bytes = "56..=57", endian = "lsb")]
    pub l_pad_force: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "58..=59", endian = "lsb")]
    pub r_pad_force: Integer<u16, packed_bits::Bits<16>>,

    // byte 60-63
    #[packed_field(bytes = "60..=61", endian = "lsb")]
    pub l_stick_force: Integer<u16, packed_bits::Bits<16>>, // Thumbstick capacitive sensors
    #[packed_field(bytes = "62..=63", endian = "lsb")]
    pub r_stick_force: Integer<u16, packed_bits::Bits<16>>,
    // 64 Bytes total
}

impl PackedInputDataReport {
    /// Return a new empty input data report
    pub fn new() -> Self {
        PackedInputDataReport {
            major_ver: 0x01,
            minor_ver: 0x00,
            report_type: 0x09,
            report_size: 64,
            frame: Integer::from_primitive(0),
            a: false,
            x: false,
            b: false,
            y: false,
            l1: false,
            r1: false,
            l2: false,
            r2: false,
            l5: false,
            menu: false,
            steam: false,
            options: false,
            down: false,
            left: false,
            right: false,
            up: false,
            _unk4: false,
            l3: false,
            _unk3: false,
            r_pad_touch: false,
            l_pad_touch: false,
            r_pad_press: false,
            l_pad_press: false,
            r5: false,
            _unk11: false,
            _unk10: false,
            _unk9: false,
            _unk8: false,
            _unk7: false,
            r3: false,
            _unk6: false,
            _unk5: false,
            _unk19: false,
            _unk18: false,
            _unk17: false,
            _unk16: false,
            _unk15: false,
            _unk14: false,
            _unk13: false,
            _unk12: false,
            r_stick_touch: false,
            l_stick_touch: false,
            _unk23: false,
            _unk22: false,
            _unk21: false,
            r4: false,
            l4: false,
            _unk20: false,
            _unk30: false,
            _unk29: false,
            _unk28: false,
            _unk27: false,
            _unk26: false,
            quick_access: false,
            _unk25: false,
            _unk24: false,
            _unk31: 0,
            l_pad_x: Integer::from_primitive(0),
            l_pad_y: Integer::from_primitive(0),
            r_pad_x: Integer::from_primitive(0),
            r_pad_y: Integer::from_primitive(0),
            accel_x: Integer::from_primitive(0),
            accel_y: Integer::from_primitive(0),
            accel_z: Integer::from_primitive(0),
            pitch: Integer::from_primitive(0),
            yaw: Integer::from_primitive(0),
            roll: Integer::from_primitive(0),
            _magn_0: Integer::from_primitive(0),
            _magn_1: Integer::from_primitive(0),
            _magn_2: Integer::from_primitive(0),
            _magn_3: Integer::from_primitive(0),
            l_trigg: Integer::from_primitive(0),
            r_trigg: Integer::from_primitive(0),
            l_stick_x: Integer::from_primitive(0),
            l_stick_y: Integer::from_primitive(0),
            r_stick_x: Integer::from_primitive(0),
            r_stick_y: Integer::from_primitive(0),
            l_pad_force: Integer::from_primitive(0),
            r_pad_force: Integer::from_primitive(0),
            l_stick_force: Integer::from_primitive(0),
            r_stick_force: Integer::from_primitive(0),
        }
    }
}

impl Default for PackedInputDataReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum PadSide {
    Left = 0,
    Right = 1,
    Both = 2,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum Intensity {
    Default = 0,
    Short = 1,
    Medium = 2,
    Long = 3,
    Insane = 4,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug)]
pub enum CommandType {
    Off = 0,
    Tick = 1,
    Click = 2,
}

/*
 * Send a haptic pulse to the trackpads
 * Duration and interval are measured in microseconds, count is the number
 * of pulses to send for duration time with interval microseconds between them
 * and gain is measured in decibels, ranging from -24 to +6
 */
#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0")]
pub struct PackedHapticPulseReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
    #[packed_field(bytes = "1")]
    pub report_size: u8,
    #[packed_field(bytes = "2", ty = "enum")]
    pub side: PadSide,
    #[packed_field(bytes = "3..=4", endian = "lsb")]
    pub amplitude: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "5..=6", endian = "lsb")]
    pub period: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub count: Integer<u16, packed_bits::Bits<16>>,
}

impl PackedHapticPulseReport {
    pub fn new() -> Self {
        Self {
            report_id: ReportType::TriggerHapticPulse as u8,
            report_size: 9,
            side: PadSide::Both,
            amplitude: Integer::from_primitive(0),
            period: Integer::from_primitive(0),
            count: Integer::from_primitive(0),
        }
    }
}

impl Default for PackedHapticPulseReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct PackedRumbleReport {
    #[packed_field(bytes = "0")]
    pub cmd_id: u8,
    #[packed_field(bytes = "1")]
    pub report_size: u8,
    #[packed_field(bytes = "2")]
    pub unk_2: u8,
    #[packed_field(bytes = "3", endian = "lsb")]
    pub event_type: u8,
    #[packed_field(bytes = "4", endian = "lsb")]
    pub intensity: u8,
    #[packed_field(bytes = "5..=6", endian = "lsb")]
    pub left_speed: Integer<u16, packed_bits::Bits<16>>,
    #[packed_field(bytes = "7..=8", endian = "lsb")]
    pub right_speed: Integer<u16, packed_bits::Bits<16>>,
}

impl PackedRumbleReport {
    pub fn new() -> Self {
        Self {
            cmd_id: ReportType::TriggerRumbleCommand as u8,
            report_size: 9,
            unk_2: 0,
            event_type: 0,
            intensity: 0,
            left_speed: Integer::from_primitive(0),
            right_speed: Integer::from_primitive(0),
        }
    }
}

impl Default for PackedRumbleReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct PackedHapticReport {
    #[packed_field(bytes = "0")]
    pub cmd_id: u8,
    #[packed_field(bytes = "1")]
    pub report_size: u8,
    #[packed_field(bytes = "2", ty = "enum")]
    pub side: PadSide,
    #[packed_field(bytes = "3", ty = "enum")]
    pub cmd_type: CommandType,
    #[packed_field(bytes = "4", ty = "enum")]
    pub intensity: Intensity,
    #[packed_field(bytes = "5")]
    pub gain: i8,
    #[packed_field(bytes = "6")]
    pub unk_6: u8,
    #[packed_field(bytes = "7")]
    pub unk_7: u8,
    #[packed_field(bytes = "8")]
    pub unk_8: u8,
    #[packed_field(bytes = "12")]
    pub unk_12: u8,
}

impl PackedHapticReport {
    pub fn new() -> Self {
        Self {
            cmd_id: ReportType::TriggerHapticCommand as u8,
            report_size: 13,
            side: PadSide::Left,
            cmd_type: CommandType::Off,
            intensity: Intensity::Default,
            gain: 0,
            unk_6: 95,
            unk_7: 204,
            unk_8: 3,
            unk_12: 16,
        }
    }
}

impl Default for PackedHapticReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "64")]
pub struct PackedMappingsReport {
    #[packed_field(bytes = "0")]
    pub report_id: u8,
}
