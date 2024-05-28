use std::fs;

use procfs::{CpuInfo, Current};

use self::data::DMIData;

pub mod data;

/// Returns DMI data from the system
pub fn get_dmi_data() -> DMIData {
    let bios_date = get_dmi_property("bios_date");
    let bios_release = get_dmi_property("bios_release");
    let bios_vendor = get_dmi_property("bios_vendor");
    let bios_version = get_dmi_property("bios_version");
    let board_asset_tag = get_dmi_property("board_asset_tag");
    let board_name = get_dmi_property("board_name");
    let board_vendor = get_dmi_property("board_vendor");
    let board_version = get_dmi_property("board_version");
    let chassis_asset_tag = get_dmi_property("chassis_asset_tag");
    let chassis_type = get_dmi_property("chassis_type");
    let chassis_vendor = get_dmi_property("chassis_vendor");
    let chassis_version = get_dmi_property("chassis_version");
    let product_family = get_dmi_property("product_family");
    let product_name = get_dmi_property("product_name");
    let product_sku = get_dmi_property("product_sku");
    let product_version = get_dmi_property("product_version");
    let sys_vendor = get_dmi_property("sys_vendor");

    DMIData {
        bios_date,
        bios_release,
        bios_vendor,
        bios_version,
        board_asset_tag,
        board_name,
        board_vendor,
        board_version,
        chassis_asset_tag,
        chassis_type,
        chassis_vendor,
        chassis_version,
        product_family,
        product_name,
        product_sku,
        product_version,
        sys_vendor,
    }
}

/// Returns the CPU info from the system
pub fn get_cpu_info() -> Result<CpuInfo, procfs::ProcError> {
    CpuInfo::current()
}

/// Read the given DMI property
fn get_dmi_property(name: &str) -> String {
    let path = format!("/sys/devices/virtual/dmi/id/{name}");
    fs::read_to_string(path)
        .unwrap_or_default()
        .replace('\n', "")
}
