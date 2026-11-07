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

use crate::config::DeviceProfile;
use crate::input::capability::{Capability, Gamepad, GamepadButton};

const PROFILE_DIR: &str = "./rootfs/usr/share/inputplumber/profiles";

/// Every shipped profile must still parse, and profiles other than the layers
/// example must keep declaring no layers.
#[test]
fn shipped_profiles_parse_without_layers() {
    let entries = std::fs::read_dir(PROFILE_DIR).expect("profiles directory");
    let mut count = 0;
    for entry in entries {
        let path = entry.expect("profile entry").path();
        if path.extension().map(|ext| ext != "yaml").unwrap_or(true) {
            continue;
        }
        let profile = DeviceProfile::from_yaml_file(path.display().to_string())
            .unwrap_or_else(|e| panic!("Failed to parse profile {path:?}: {e}"));
        if path
            .file_name()
            .map(|name| name == "layers_example.yaml")
            .unwrap_or(false)
        {
            continue;
        }
        assert!(profile.layers.is_empty(), "Profile {path:?} has layers");
        count += 1;
    }
    assert!(count > 0, "No profiles found in {PROFILE_DIR}");
}

/// The shipped layers example must parse with the layers the docs describe.
#[test]
fn shipped_layers_example_parses() {
    let path = format!("{PROFILE_DIR}/layers_example.yaml");
    let profile = DeviceProfile::from_yaml_file(path).expect("parse layers example profile");

    let names: Vec<&str> = profile.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["mouse_mode", "shortcuts", "rapid"]);

    // The layer outputs need targets that a gamepad-only composite lacks.
    let targets = profile.target_devices.unwrap_or_default();
    for kind in ["gamepad", "keyboard", "mouse"] {
        assert!(
            targets.iter().any(|t| t == kind),
            "shipped example must declare a {kind} target device"
        );
    }

    // Both the shortcuts and rapid layers remap South.
    let south: Capability = Capability::Gamepad(Gamepad::Button(GamepadButton::South));
    let remaps_south = profile
        .layers
        .iter()
        .filter(|layer| {
            layer
                .mapping
                .iter()
                .any(|m| Into::<Capability>::into(m.source_event.clone()) == south)
        })
        .count();
    assert_eq!(remaps_south, 2);
}

/// A profile with `layers:` and `layer:` targets parses, and dumping it
/// round-trips the layers.
#[test]
fn profile_with_layers_parses_and_round_trips() {
    let yaml = r#"
version: 1
kind: DeviceProfile
name: Layers example
mapping:
  - name: Guide opens the guide layer
    source_event: { gamepad: { button: Guide } }
    target_events:
      - layer: { name: guide }
  - name: QuickAccess latches mouse mode
    source_event: { gamepad: { button: QuickAccess } }
    target_events:
      - layer: { name: mouse_mode, mode: toggle }
layers:
  - name: guide
    mapping:
      - name: Guide+South -> Esc
        source_event: { gamepad: { button: South } }
        target_events:
          - keyboard: KeyEsc
"#;

    let profile = DeviceProfile::from_yaml(yaml.to_string()).expect("parse profile with layers");
    assert_eq!(profile.layers.len(), 1);
    assert_eq!(profile.layers[0].name, "guide");
    assert_eq!(profile.layers[0].mapping.len(), 1);

    // Activation targets resolve to the layer capability with a resolved mode.
    let hold: Capability = profile.mapping[0].target_events[0].clone().into();
    assert_eq!(
        hold,
        Capability::Layer {
            name: "guide".to_string(),
            mode: crate::config::capability_map::LayerActivationMode::Hold
        }
    );
    let toggle: Capability = profile.mapping[1].target_events[0].clone().into();
    assert_eq!(toggle.to_capability_string(), "Layer:mouse_mode:toggle");

    // A profile without layers dumps without the key; one with layers round-trips.
    let dumped = serde_yaml::to_string(&profile).expect("serialize profile");
    assert!(dumped.contains("layers:"));
    let reparsed = DeviceProfile::from_yaml(dumped).expect("re-parse dumped profile");
    assert_eq!(reparsed.layers.len(), 1);
    assert_eq!(reparsed.layers[0].mapping[0].name, "Guide+South -> Esc");

    let source: Capability = profile.mapping[0].source_event.clone().into();
    assert_eq!(
        source,
        Capability::Gamepad(Gamepad::Button(GamepadButton::Guide))
    );
}

/// Profiles without layers must dump without a `layers:` key so existing
/// saved profiles stay byte-identical.
#[test]
fn profile_without_layers_dumps_without_layers_key() {
    let yaml = r#"
version: 1
kind: DeviceProfile
name: No layers
mapping:
  - name: South -> A
    source_event: { gamepad: { button: South } }
    target_events:
      - gamepad: { button: South }
"#;
    let profile = DeviceProfile::from_yaml(yaml.to_string()).expect("parse profile");
    let dumped = serde_yaml::to_string(&profile).expect("serialize profile");
    assert!(!dumped.contains("layers:"));
}
