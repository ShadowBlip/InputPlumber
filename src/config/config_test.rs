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
        let Ok(config) = CompositeDeviceConfig::from_yaml_path(&path) else {
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
