/// Container for system DMI data
#[derive(Debug, Clone, Default)]
pub struct DMIData {
    pub bios_date: String,
    pub bios_release: String,
    pub bios_vendor: String,
    pub bios_version: String,
    pub board_asset_tag: String,
    pub board_name: String,
    pub board_vendor: String,
    pub board_version: String,
    pub chassis_asset_tag: String,
    pub chassis_type: String,
    pub chassis_vendor: String,
    pub chassis_version: String,
    pub product_family: String,
    pub product_name: String,
    pub product_sku: String,
    pub product_version: String,
    pub sys_vendor: String,
}
