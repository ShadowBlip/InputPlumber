//! Based on the pattern developed by the hhd project:
//! https://github.com/hhd-dev/hhd/blob/master/src/hhd/controller/lib/hide.py

#[cfg(test)]
pub mod device_test;

pub mod device;

use std::{error::Error, fs, path::Path};

use tokio::process::Command;
use udev::Enumerator;

use self::device::Device;

const RULE_PRIORITY: &str = "59";
const RULES_PREFIX: &str = "/run/udev/rules.d";

/// Hide all removable input devices from regular users.
pub async fn block_joysticks() -> Result<(), Box<dyn Error>> {
    // Find the chmod command to use for hiding
    let chmod_cmd = if Path::new("/bin/chmod").exists() {
        "/bin/chmod"
    } else {
        "/usr/bin/chmod"
    };

    let rule = format!(
        r#"# Hide all evdev devices that are not InputPlumber virtual devices
ACTION=="add|change", SUBSYSTEM=="input", KERNEL=="js[0-9]*|event[0-9]*", ENV{{ID_INPUT_JOYSTICK}}=="1", ENV{{INPUTPLUMBER_VIRT}}!="1", MODE:="0000", GROUP:="root", RUN:="{chmod_cmd} 000 %p"

# Hide all Horipad Steam Controller hidraw devices
ACTION=="add|change", SUBSYSTEM=="hidraw", KERNEL=="hidraw[0-9]*", ATTR{{idVendor}}=="0F0D", ATTR{{idProduct}}=="0196", ENV{{INPUTPLUMBER_VIRT}}!="1", MODE:="0000", GROUP:="root", RUN:="{chmod_cmd} 000 %p"
ACTION=="add|change", SUBSYSTEM=="hidraw", KERNEL=="hidraw[0-9]*", ATTR{{idVendor}}=="0F0D", ATTR{{idProduct}}=="01AB", ENV{{INPUTPLUMBER_VIRT}}!="1", MODE:="0000", GROUP:="root", RUN:="{chmod_cmd} 000 %p"

# Hide all PlayStation hidraw devices
ACTION=="add|change", SUBSYSTEMS=="hid", DRIVERS=="playstation", GOTO="playstation_start"
GOTO="playstation_end"
LABEL="playstation_start"
ACTION=="add|change", SUBSYSTEM=="hidraw", KERNEL=="hidraw[0-9]*", ENV{{INPUTPLUMBER_VIRT}}!="1", MODE:="0000", GROUP:="root", RUN:="{chmod_cmd} 000 %p"
LABEL="playstation_end"
"#
    );

    // Write the udev rule
    fs::create_dir_all(RULES_PREFIX)?;
    let rule_path = format!("{RULES_PREFIX}/51-inputplumber-hide-joysticks.rules");
    fs::write(rule_path, rule)?;

    reload_all().await?;

    Ok(())
}

/// Unhide all removable input devices from regular users.
pub async fn unblock_joysticks() -> Result<(), Box<dyn Error>> {
    let rule_path = format!("{RULES_PREFIX}/51-inputplumber-hide-joysticks.rules");
    fs::remove_file(rule_path)?;
    reload_all().await?;

    Ok(())
}

/// Hide the given input device from regular users.
pub async fn hide_device(path: &str) -> Result<(), Box<dyn Error>> {
    // Get the device to hide
    let device = get_device(path.to_string()).await?;
    let name = device.name.clone();
    let Some(parent) = device.get_parent() else {
        return Err("Unable to determine parent for device".into());
    };
    let subsystem = device.subsystem.clone();
    let Some(match_rule) = device.get_match_rule() else {
        return Err("Unable to create match rule for device".into());
    };

    // Find the chmod command to use for hiding
    let chmod_cmd = if Path::new("/bin/chmod").exists() {
        "/bin/chmod"
    } else {
        "/usr/bin/chmod"
    };

    // Create a udev rule to hide the device
    let rule = format!(
        r#"# Hides devices stemming from {name}
# Managed by InputPlumber, this file will be autoremoved during configuration changes.
{match_rule}, GOTO="inputplumber_valid"
GOTO="inputplumber_end"
LABEL="inputplumber_valid"
ACTION=="add|change", KERNEL=="hidraw[0-9]*|js[0-9]*|event[0-9]*", SUBSYSTEM=="{subsystem}", MODE:="0000", GROUP:="root", RUN:="{chmod_cmd} 000 {path}", SYMLINK+="inputplumber/%k"
LABEL="inputplumber_end"
"#
    );

    // Write the udev rule
    fs::create_dir_all(RULES_PREFIX)?;
    let rule_path = format!("{RULES_PREFIX}/{RULE_PRIORITY}-inputplumber-hide-{name}.rules");
    fs::write(rule_path, rule)?;

    // Reload udev
    reload_children(parent).await?;

    Ok(())
}

/// Unhide the given device
pub async fn unhide_device(path: String) -> Result<(), Box<dyn Error>> {
    // Get the device to unhide
    let device = get_device(path.clone()).await?;
    let name = device.name.clone();
    let Some(parent) = device.get_parent() else {
        return Err("Unable to determine parent for device".into());
    };
    let rule_path = format!("{RULES_PREFIX}/{RULE_PRIORITY}-inputplumber-hide-{name}.rules");
    fs::remove_file(rule_path)?;

    // Reload udev
    reload_children(parent).await?;

    Ok(())
}

/// Unhide all devices hidden by InputPlumber
pub async fn unhide_all() -> Result<(), Box<dyn Error>> {
    let entries = fs::read_dir(RULES_PREFIX)?;
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.starts_with(format!("{RULE_PRIORITY}-inputplumber-hide").as_str()) {
            continue;
        }
        let path = entry.path().to_string_lossy().to_string();
        fs::remove_file(path)?;
    }

    // Reload udev rules
    reload_all().await?;

    Ok(())
}

/// Trigger udev to evaluate rules on the children of the given parent device path
async fn reload_children(parent: String) -> Result<(), Box<dyn Error>> {
    let _ = Command::new("udevadm")
        .args(["control", "--reload-rules"])
        .output()
        .await?;

    for action in ["remove", "add"] {
        let _ = Command::new("udevadm")
            .args(["trigger", "--action", action, "-b", parent.as_str()])
            .output()
            .await?;
    }

    Ok(())
}

/// Trigger udev to evaluate rules on the children of the given parent device path
async fn reload_all() -> Result<(), Box<dyn Error>> {
    let _ = Command::new("udevadm")
        .args(["control", "--reload-rules"])
        .output()
        .await?;

    let _ = Command::new("udevadm").arg("trigger").output().await?;

    Ok(())
}

/// Returns device information for the given device path using udevadm.
pub async fn get_device(path: String) -> Result<Device, Box<dyn Error>> {
    let mut device = Device::default();
    let output = Command::new("udevadm")
        .args(["info", path.as_str()])
        .output()
        .await?;
    let output = String::from_utf8(output.stdout)?;

    for line in output.split('\n') {
        if line.starts_with("P: ") {
            let line = line.replace("P: ", "");
            device.path = line;
            continue;
        }
        if line.starts_with("M: ") {
            let line = line.replace("M: ", "");
            device.name = line;
            continue;
        }
        if line.starts_with("R: ") {
            let line = line.replace("R: ", "");
            let number = line.parse().unwrap_or_default();
            device.number = number;
            continue;
        }
        if line.starts_with("U: ") {
            let line = line.replace("U: ", "");
            device.subsystem = line;
            continue;
        }
        if line.starts_with("T: ") {
            let line = line.replace("T: ", "");
            device.device_type = line;
            continue;
        }
        if line.starts_with("D: ") {
            let line = line.replace("D: ", "");
            device.node = line;
            continue;
        }
        if line.starts_with("I: ") {
            let line = line.replace("I: ", "");
            device.network_index = line;
            continue;
        }
        if line.starts_with("N: ") {
            let line = line.replace("N: ", "");
            device.node_name = line;
            continue;
        }
        if line.starts_with("L: ") {
            let line = line.replace("L: ", "");
            let priority = line.parse().unwrap_or_default();
            device.symlink_priority = priority;
            continue;
        }
        if line.starts_with("S: ") {
            let line = line.replace("S: ", "");
            device.symlink.push(line);
            continue;
        }
        if line.starts_with("Q: ") {
            let line = line.replace("Q: ", "");
            let seq = line.parse().unwrap_or_default();
            device.sequence_num = seq;
            continue;
        }
        if line.starts_with("V: ") {
            let line = line.replace("V: ", "");
            device.driver = line;
            continue;
        }
        if line.starts_with("E: ") {
            let line = line.replace("E: ", "");
            let mut parts = line.splitn(2, '=');
            if parts.clone().count() != 2 {
                continue;
            }
            let key = parts.next().unwrap();
            let value = parts.last().unwrap();
            device.properties.insert(key.to_string(), value.to_string());
            continue;
        }
    }

    Ok(device)
}

/// Returns a list of devices in the given subsystem that have a devnode property.
pub fn discover_devices(subsystem: &str) -> Result<Vec<udev::Device>, Box<dyn Error>> {
    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem(subsystem)?;

    log::debug!("Started udev {subsystem} enumerator.");

    let mut node_devices = Vec::new();
    let devices = enumerator.scan_devices()?;
    for device in devices {
        let Some(_) = device.devnode() else {
            log::trace!("No devnode found for device: {:?}", device);
            continue;
        };

        let name = device.sysname();
        log::debug!("udev {subsystem} enumerator found device: {:?}", name);

        node_devices.push(device);
    }

    Ok(node_devices)
}
