//! Module for searching for InputPlumber config files

use std::path::PathBuf;

/// Base system fallback path to use if one cannot be found with XDG
const FALLBACK_BASE_PATH: &str = "/usr/share/inputplumber";

/// Returns the base path for configuration data
pub fn get_base_path() -> PathBuf {
    let Ok(base_dirs) = xdg::BaseDirectories::with_prefix("inputplumber") else {
        log::warn!("Unable to determine config base path. Using fallback path.");
        return PathBuf::from(FALLBACK_BASE_PATH);
    };

    // Get the data directories in preference order
    let data_dirs = base_dirs.get_data_dirs();
    for dir in data_dirs {
        if dir.exists() {
            return dir;
        }
    }

    log::warn!("Config base path not found. Using fallback path.");
    PathBuf::from(FALLBACK_BASE_PATH)
}

/// Returns the directory for input profiles (e.g. "/usr/share/inputplumber/profiles")
pub fn get_profiles_path() -> PathBuf {
    let rel_path = PathBuf::from("./rootfs/usr/share/inputplumber/profiles");
    if rel_path.exists() && rel_path.is_dir() {
        return rel_path;
    }
    let base_path = get_base_path();
    base_path.join("profiles")
}

/// Returns a list of directories in preference order to find device configurations.
/// E.g. ["/etc/inputplumber/devices.d", "/usr/share/inputplumber/devices"]
pub fn get_devices_paths() -> Vec<PathBuf> {
    let paths = vec![
        PathBuf::from("./rootfs/usr/share/inputplumber/devices"),
        PathBuf::from("/etc/inputplumber/devices.d"),
        get_base_path().join("devices"),
    ];

    paths
}

/// Returns a list of directories in preference order to find capability map configs.
/// E.g. ["/etc/inputplumber/capability_maps.d", "/usr/share/inputplumber/capability_maps"]
pub fn get_capability_maps_paths() -> Vec<PathBuf> {
    let paths = vec![
        get_base_path().join("capability_maps"),
        PathBuf::from("/etc/inputplumber/capability_maps.d"),
        PathBuf::from("./rootfs/usr/share/inputplumber/capability_maps"),
    ];

    paths
}
