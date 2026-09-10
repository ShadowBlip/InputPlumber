use std::{
    collections::{HashMap, HashSet},
    error::Error,
    path::PathBuf,
};

use glob_match::glob_match;
use tokio::fs;

use crate::config::CompositeDeviceConfig;

const AUTOSTART_HWDB_FILE: &str = "./rootfs/usr/lib/udev/hwdb.d/60-inputplumber-autostart.hwdb";
const DEVICE_CONFIG_DIR: &str = "./rootfs/usr/share/inputplumber/devices";

const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const PURPLE: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";
const ENDCOLOR: &str = "\x1b[0m";

/// Test for validating that there is a device entry in the autostart hwdb file
/// for every device with `auto_manage: true`.
#[tokio::test]
async fn check_autostart_rules() -> Result<(), Box<dyn Error>> {
    // Find all config files
    let mut configs = HashMap::new();
    let mut config_dir = fs::read_dir(DEVICE_CONFIG_DIR).await?;
    while let Some(entry) = config_dir.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }

        // Load the config file
        let path = entry.path();
        let Ok(config) = CompositeDeviceConfig::from_yaml_file(path.display().to_string()) else {
            continue;
        };

        // Only consider configs with auto_manage enabled
        let Some(options) = config.options.as_ref() else {
            continue;
        };
        if !options.auto_manage.unwrap_or_default() {
            continue;
        }

        configs.insert(path, config);
    }

    // Check each config
    let mut failures = Vec::new();
    let mut failed_configs = HashSet::new();
    for (path, config) in configs {
        println!("Checking config {CYAN}{path:?}{ENDCOLOR}");

        // Validate each DMI match
        for entry in config.matches {
            let Some(dmi) = entry.dmi_data else {
                continue;
            };

            // Build the glob pattern to match in the file
            let mut patterns = Vec::new();
            if let Some(vendor) = dmi.sys_vendor.as_ref() {
                let vendor = vendor.replace(" ", ""); // Remove spaces
                let pattern_part = format!("svn{vendor}:");
                patterns.push(pattern_part);
            }
            if let Some(product) = dmi.product_name.as_ref() {
                let product = product.replace(" ", ""); // Remove spaces
                let pattern_part = format!("pn{product}:");
                patterns.push(pattern_part);
            }
            if let Some(board_vendor) = dmi.board_vendor.as_ref() {
                let board_vendor = board_vendor.replace(" ", ""); // Remove spaces
                let pattern_part = format!("rvn{board_vendor}:");
                patterns.push(pattern_part);
            }
            if let Some(board_name) = dmi.board_name.as_ref() {
                let board_name = board_name.replace(" ", ""); // Remove spaces
                let pattern_part = format!("rn{board_name}:");
                patterns.push(pattern_part);
            }

            let pattern = patterns.join("*");
            let pattern = format!("dmi:*{pattern}*");
            println!(
                "  Checking for autostart rule with glob pattern: {YELLOW}{pattern}{ENDCOLOR}"
            );

            // Check to see if the pattern matches any lines in the hwdb file
            let mut has_hwdb_entry = false;
            let hwdb_file = fs::read_to_string(PathBuf::from(AUTOSTART_HWDB_FILE)).await?;
            for line in hwdb_file.lines() {
                //println!("Line: {line}");
                if glob_match(pattern.as_str(), line) {
                    //println!("  Line matches pattern: {pattern}");
                    has_hwdb_entry = true;
                }
            }

            if has_hwdb_entry {
                continue;
            }

            println!(
                "    {RED}Failed to find pattern {YELLOW}'{pattern}'{RED} in hwdb config{ENDCOLOR}"
            );
            failures.push(format!("Unable to find pattern '{pattern}' generated from config {path:?} in hwdb file: {AUTOSTART_HWDB_FILE}"));
            failed_configs.insert(path.clone());
        }
    }

    // Print the results
    println!();

    if failures.is_empty() {
        println!("Total errors: 0");
        println!();
        println!("Success!");
        return Ok(());
    }

    println!("Errors:");
    for failure in failures.iter() {
        let msg = format!("  {RED}* {failure}{ENDCOLOR}");
        println!("{msg}");
    }
    println!("Total errors: {}", failures.len());
    println!();

    println!("Configs with failures:");
    let mut failed_configs: Vec<PathBuf> = failed_configs.into_iter().collect();
    failed_configs.sort();
    for config in failed_configs {
        println!("  {config:?}");
    }

    println!();
    println!("{PURPLE}ERROR: The above device configurations have `auto_manage: true`, but do not have a matching entry in the `inputplumber-autostart.hwdb` file. Please add an entry to the hwdb file so the inputplumber service will start when the device is detected.{ENDCOLOR}");
    println!();
    println!("Failed!");

    assert_eq!(failures.len(), 0);

    Ok(())
}

/// Quick validation test for the new Mayflash GameCube Controller Adapter
/// capability map and device profile added to fix legacy joystick-range
/// evdev button codes (BTN_TRIGGER/0x120 through BTN_DEAD/0x12f) not being
/// recognized as gamepad buttons.
#[test]
fn check_mayflash_gamecube_adapter_configs_parse() {
    use crate::config::capability_map::CapabilityMapConfig;

    let map = CapabilityMapConfig::from_yaml_file(
        "./rootfs/usr/share/inputplumber/capability_maps/mayflash_gamecube_adapter.yaml",
    )
    .expect("capability map should parse");
    assert_eq!(map.id(), "mayflash_gamecube_adapter");

    let device_config = CompositeDeviceConfig::from_yaml_file(
        "./rootfs/usr/share/inputplumber/devices/55-mayflash_gamecube_adapter.yaml".to_string(),
    )
    .expect("device profile should parse");
    assert_eq!(device_config.name, "Mayflash GameCube Controller Adapter");
    assert_eq!(
        device_config.capability_map_id.as_deref(),
        Some("mayflash_gamecube_adapter"),
        "capability_map_id must be set at the CompositeDeviceConfig level to actually apply"
    );
    assert_eq!(device_config.source_devices.len(), 1);
    let evdev = device_config.source_devices[0]
        .evdev
        .as_ref()
        .expect("source device should have an evdev matcher");
    assert_eq!(evdev.vendor_id.as_deref(), Some("0079"));
    assert_eq!(evdev.product_id.as_deref(), Some("1844"));
    // Setting capability_map_id only at the CompositeDeviceConfig level (as
    // asserted above) is NOT sufficient to actually translate events - only
    // the SourceDevice-level field wires up the EventTranslator that
    // performs real-time evdev KEY -> Capability translation
    // (src/input/source/evdev.rs EventDevice::new). Confirmed live: without
    // this field set here, axis/D-pad events passed through fine but every
    // face-button press produced zero output events.
    assert_eq!(
        device_config.source_devices[0].capability_map_id.as_deref(),
        Some("mayflash_gamecube_adapter"),
        "capability_map_id must ALSO be set on the source_devices entry itself \
         (not just at the CompositeDeviceConfig level) or button translation \
         silently does nothing at runtime"
    );
}
