//! Based on the pattern developed by the hhd project:
//! https://github.com/hhd-dev/hhd/blob/master/src/hhd/controller/lib/hide.py

#[cfg(test)]
pub mod device_test;

pub mod device;

use std::os::linux::fs::MetadataExt;
use std::{error::Error, fs, path::Path};

use device::AttributeGetter;
use tokio::process::Command;
use udev::Enumerator;

use self::device::Device;

const RULES_PREFIX: &str = "/run/udev/rules.d";

/// Hide the given input device from regular users.
pub async fn hide_device(path: String) -> Result<(), Box<dyn Error>> {
    // Get the device to hide
    let device = get_device(path.clone()).await?;
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
KERNEL=="hidraw[0-9]*|js[0-9]*|event[0-9]*", SUBSYSTEM=="{subsystem}", MODE="000", GROUP="root", TAG-="uaccess", RUN+="{chmod_cmd} 000 {path}"
LABEL="inputplumber_end"
"#
    );

    // Write the udev rule
    fs::create_dir_all(RULES_PREFIX)?;
    let rule_path = format!("{RULES_PREFIX}/96-inputplumber-hide-{name}.rules");
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
    let rule_path = format!("{RULES_PREFIX}/96-inputplumber-hide-{name}.rules");
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
        if !filename.starts_with("96-inputplumber-hide") {
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

    let output = (match path.starts_with("/dev/") {
        true => {
            let metadata = fs::metadata(path).map_err(|e| Box::new(e))?;
            let devtype = match metadata.st_mode() & nix::libc::S_IFMT {
                nix::libc::S_IFCHR => Some(udev::DeviceType::Character),
                nix::libc::S_IFBLK => Some(udev::DeviceType::Block),
                _ => None,
            }
            .expect("Not a character or block special file");
            udev::Device::from_devnum(devtype, metadata.st_rdev())
        }
        false => udev::Device::from_syspath(Path::new(path.as_str())),
    })
    .map_err(|e| Box::new(e))?;

    device.path = String::from(output.syspath().to_string_lossy());
    device.name = String::from(output.name());

    if let Some(val) = output.devnum() {
        device.number = val
    };

    if let Some(val) = output.subsystem() {
        device.subsystem = String::from(val.to_string_lossy())
    };

    if let Some(val) = output.devtype() {
        device.device_type = String::from(val.to_string_lossy())
    };

    if let Some(val) = output.devnode() {
        device.node = String::from(val.to_string_lossy())
    };

    if let Some(val) = output.driver() {
        device.driver = String::from(val.to_string_lossy())
    };

    device.properties = output
        .properties()
        .into_iter()
        .map(|p| {
            (
                String::from(p.name().to_string_lossy()),
                String::from(p.value().to_string_lossy()),
            )
        })
        .collect();

    // TODO: L: device.symlink_priority
    // TODO: S: device.symlink
    // TODO: Q: device.sequence_num
    // TODO: I: Network interface index
    Ok(device)
}

/// Returns a list of devices in the given subsystem that have a devnode property.
pub fn discover_devices(subsystem: &str) -> Result<Vec<udev::Device>, Box<dyn Error>> {
    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem(subsystem)?;

    log::debug!("Started udev {subsystem} enumerator.");

    Ok(enumerator
        .scan_devices()?
        .into_iter()
        .map(|device| device)
        .collect())
}
