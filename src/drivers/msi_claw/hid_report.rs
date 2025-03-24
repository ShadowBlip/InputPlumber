use packed_struct::prelude::*;

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum ReportId {
    #[default]
    SendCommand = 15,
}

#[derive(PrimitiveEnum_u8, Clone, Copy, PartialEq, Debug, Default)]
pub enum Command {
    #[default]
    SwitchMode = 36,
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
    #[packed_field(bytes = "5", ty = "enum")]
    pub mode: GamepadMode,
    #[packed_field(bytes = "6")]
    pub unk_6: u8,
    #[packed_field(bytes = "7")]
    pub unk_7: u8,
}

impl Default for PackedCommandReport {
    fn default() -> Self {
        Self {
            report_id: ReportId::SendCommand,
            unk_1: Default::default(),
            unk_2: Default::default(),
            unk_3: 60,
            command: Command::SwitchMode,
            mode: GamepadMode::XInput,
            unk_6: Default::default(),
            unk_7: Default::default(),
        }
    }
}
