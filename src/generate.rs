use std::fs::File;
use std::io::Write;

use inputplumber::config::capability_map::CapabilityMapConfigV2;
use schemars::schema_for;

fn main() {
    let composite_device_v2_schema = schema_for!(CapabilityMapConfigV2);
    let mut file = File::create("./rootfs/usr/share/inputplumber/schema/capability_map_v2.json")
        .expect("Failed to create schema file");
    write!(
        file,
        "{}",
        serde_json::to_string_pretty(&composite_device_v2_schema).unwrap()
    )
    .expect("Failed to write schema");
}
