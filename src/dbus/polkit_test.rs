use std::{collections::HashSet, error::Error, path::PathBuf};

use tokio::fs;

const POLKIT_POLICY_PATH: &str =
    "./rootfs/usr/share/polkit-1/actions/org.shadowblip.InputPlumber.policy";
const DBUS_IFACE_SRC_DIR: &str = "./src/dbus/interface";

const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const PURPLE: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";
const ENDCOLOR: &str = "\x1b[0m";

#[derive(Debug)]
struct PolKitUsage {
    path: PathBuf,
    line: usize,
    action: String,
}

#[tokio::test]
async fn check_polkit_policies() -> Result<(), Box<dyn Error>> {
    let defined_actions = get_polkit_actions().await?;
    let usages_in_source = get_polkit_usage_in_source().await?;

    let mut failures = Vec::new();
    let mut failed_refs = Vec::new();
    for usage in usages_in_source {
        let (path, line, action) = (
            usage.path.to_string_lossy().to_string(),
            usage.line,
            usage.action,
        );
        println!(
            "Checking polkit usage in {CYAN}'{path}:{line}'{ENDCOLOR}: {YELLOW}{action}{ENDCOLOR}"
        );
        if defined_actions.contains(&action) {
            continue;
        }
        failures.push(format!("Unable to find polkit policy '{action}' in policy file '{POLKIT_POLICY_PATH}' for polkit usage in '{path}:{line}'."));
        failed_refs.push(format!("{path}:{line}"));
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

    println!("References with failures:");
    let mut failed_refs: Vec<String> = failed_refs.into_iter().collect();
    failed_refs.sort();
    for config in failed_refs {
        println!("  {config:?}");
    }

    println!();
    println!("{PURPLE}ERROR: The above references call `check_polkit()`, but do not have a matching entry in the `{POLKIT_POLICY_PATH}` file. Please add an entry to the policy file so authorization can be enforced.{ENDCOLOR}");
    println!();
    println!("Failed!");

    assert_eq!(failures.len(), 0);

    Ok(())
}

async fn get_polkit_actions() -> Result<HashSet<String>, Box<dyn Error>> {
    let mut actions = HashSet::new();
    let policy_data = fs::read_to_string(POLKIT_POLICY_PATH).await?;
    let lines: Vec<&str> = policy_data.split('\n').collect();
    for line in lines {
        if !line.contains("action id=") {
            continue;
        }
        let parts: Vec<&str> = line.split('"').collect();
        if parts.len() < 2 {
            continue;
        }
        let action = parts.get(1).unwrap().to_string();
        actions.insert(action);
    }

    Ok(actions)
}

async fn get_polkit_usage_in_source() -> Result<Vec<PolKitUsage>, Box<dyn Error>> {
    let mut usages = Vec::new();

    let root_dir = fs::read_dir(DBUS_IFACE_SRC_DIR).await?;
    let mut dirs_to_visit = vec![root_dir];

    while !dirs_to_visit.is_empty() {
        let Some(mut dir) = dirs_to_visit.pop() else {
            continue;
        };
        while let Some(entry) = dir.next_entry().await? {
            let entry_type = entry.file_type().await?;
            if entry_type.is_dir() {
                dirs_to_visit.push(fs::read_dir(entry.path()).await?);
                continue;
            }

            let mut file_usages = get_polkit_usages_in_file(entry.path()).await?;
            if file_usages.is_empty() {
                continue;
            }
            usages.append(&mut file_usages);
        }
    }

    Ok(usages)
}

async fn get_polkit_usages_in_file(path: PathBuf) -> Result<Vec<PolKitUsage>, Box<dyn Error>> {
    let mut usages = Vec::new();
    let source = fs::read_to_string(&path).await?;
    let lines: Vec<&str> = source.split('\n').collect();
    for (line_no, line) in lines.iter().enumerate() {
        if !line.contains("check_polkit(") {
            continue;
        }
        let parts: Vec<&str> = line.split('"').collect();
        if parts.len() < 2 {
            continue;
        }
        let action = parts.get(1).unwrap().to_string();
        let usage = PolKitUsage {
            path: path.clone(),
            line: line_no,
            action,
        };
        usages.push(usage);
    }

    Ok(usages)
}
