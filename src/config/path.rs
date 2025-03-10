//! Module for searching for InputPlumber config files

use std::{
    fs::{self, DirEntry},
    path::PathBuf,
};

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

/// Returns a list of directories in load order to find device configurations.
/// E.g. ["/etc/inputplumber/devices.d", "/usr/share/inputplumber/devices"]
pub fn get_devices_paths() -> Vec<PathBuf> {
    let paths = vec![
        PathBuf::from("./rootfs/usr/share/inputplumber/devices"),
        PathBuf::from("/etc/inputplumber/devices.d"),
        get_base_path().join("devices"),
    ];

    paths
}

/// Returns a list of directories in load order to find capability map configs.
/// E.g. ["/etc/inputplumber/capability_maps.d", "/usr/share/inputplumber/capability_maps"]
pub fn get_capability_maps_paths() -> Vec<PathBuf> {
    let paths = vec![
        get_base_path().join("capability_maps"),
        PathBuf::from("/etc/inputplumber/capability_maps.d"),
        PathBuf::from("./rootfs/usr/share/inputplumber/capability_maps"),
    ];

    paths
}

/// Returns a list of file paths for the given directories sorted by filename across
/// all given directories. The filter argument is a closure that should return
/// `true` for any files that should be included in the final results.
pub fn get_multidir_sorted_files<F>(paths: &[PathBuf], filter: F) -> Vec<PathBuf>
where
    F: Fn(&DirEntry) -> bool,
{
    // Look for files in the given locations
    let mut file_entries: Vec<DirEntry> = paths
        .iter()
        .flat_map(|path| {
            log::trace!("Checking {path:?} for files");
            let files = match fs::read_dir(path) {
                Ok(files) => files,
                Err(e) => {
                    log::debug!("Unable to read directory: {path:?}: {e}");
                    return vec![];
                }
            };
            files
                .filter_map(|r| {
                    let Ok(entry) = r else { return None };
                    log::trace!("Got entry: {entry:?}");
                    if filter(&entry) {
                        Some(entry)
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();
    log::trace!("Got file entries: {file_entries:?}");

    // Sort the device configs by name
    file_entries.sort_by(|a, b| {
        let file_name_a = a.file_name();
        let file_name_b = b.file_name();
        if file_name_a != file_name_b {
            return file_name_a.cmp(&file_name_b);
        }

        // If the filenames match, use the path order
        let path_a = a.path();
        let Some(directory_a) = path_a.parent() else {
            return file_name_a.cmp(&file_name_b);
        };
        let path_b = b.path();
        let Some(directory_b) = path_b.parent() else {
            return file_name_a.cmp(&file_name_b);
        };

        let directory_a_priority = paths
            .iter()
            .position(|base_path| base_path.as_os_str() == directory_a.as_os_str())
            .unwrap_or(10);
        let directory_b_priority = paths
            .iter()
            .position(|base_path| base_path.as_os_str() == directory_b.as_os_str())
            .unwrap_or(10);

        directory_a_priority.cmp(&directory_b_priority)
    });
    log::trace!("Got sorted entries: {file_entries:?}");

    file_entries.into_iter().map(|entry| entry.path()).collect()
}
