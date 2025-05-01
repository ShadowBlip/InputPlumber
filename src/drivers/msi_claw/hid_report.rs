use packed_struct::prelude::*;

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum ReportId {
    #[default]
    SendCommand = 15,
    CommandResponse = 16,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum Command {
    Ack = 6,
    #[default]
    SwitchMode = 36,
    ReadGamepadMode = 38,
    GamepadModeAck = 39,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum GamepadMode {
    Offline = 0,
    #[default]
    XInput = 1,
    DInput = 2,
    Msi = 3,
    Desktop = 4,
    Bios = 5,
    Testing = 6,
}

impl From<u8> for GamepadMode {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Offline,
            1 => Self::XInput,
            2 => Self::DInput,
            3 => Self::Msi,
            4 => Self::Desktop,
            5 => Self::Bios,
            6 => Self::Testing,
            _ => Self::Offline,
        }
    }
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum MkeysFunction {
    #[default]
    Macro = 0,
    Combination = 1,
}

#[derive(PackedStruct, Debug, Copy, Clone, PartialEq)]
#[packed_struct(bit_numbering = "msb0", size_bytes = "8")]
pub struct PackedCommandReport {
    #[packed_field(bytes = "0", ty = "enum")]
    pub report_id: ReportId,
    #[packed_field(bytes = "1")]
    pub unk_1: u8,
    #[packed_field(bytes = "2")]
    pub unk_2: u8,
    #[packed_field(bytes = "3")]
    pub unk_3: u8,
    #[packed_field(bytes = "4", ty = "enum")]
    pub command: Command,
    #[packed_field(bytes = "5")]
    pub arg1: u8,
    #[packed_field(bytes = "6")]
    pub arg2: u8,
    #[packed_field(bytes = "7")]
    pub arg3: u8,
}

impl Default for PackedCommandReport {
    fn default() -> Self {
        Self {
            report_id: ReportId::SendCommand,
            unk_1: Default::default(),
            unk_2: Default::default(),
            unk_3: 60,
            command: Command::SwitchMode,
            arg1: GamepadMode::XInput as u8,
            arg2: 1,
            arg3: Default::default(),
        }
    }
}

impl PackedCommandReport {
    /// Create an input report to switch gamepad mode
    pub fn switch_mode(mode: GamepadMode, mkeys: MkeysFunction) -> Self {
        Self {
            report_id: ReportId::SendCommand,
            unk_1: Default::default(),
            unk_2: Default::default(),
            unk_3: 60,
            command: Command::SwitchMode,
            arg1: mode as u8,
            arg2: mkeys as u8,
            arg3: Default::default(),
        }
    }

    /// Create an input report to query the gamepad mode
    pub fn read_mode() -> Self {
        Self {
            report_id: ReportId::SendCommand,
            unk_1: Default::default(),
            unk_2: Default::default(),
            unk_3: 60,
            command: Command::ReadGamepadMode,
            arg1: Default::default(),
            arg2: Default::default(),
            arg3: Default::default(),
        }
    }
}
