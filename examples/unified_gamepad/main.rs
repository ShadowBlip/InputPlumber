use std::{fs, thread::sleep, time::Duration};

use inputplumber::drivers::unified_gamepad::driver::Driver;

const SEARCH_PATH: &str = "/sys/bus/hid/devices";

fn main() {
    // Look for a Unified Controller
    let paths = fs::read_dir(SEARCH_PATH).expect("Unable to read HID devices");

    println!("Searching for Unified Controller");
    let mut device_sys_path = None;
    for path in paths {
        let path = path.unwrap();
        let mut file_path = path.path().into_os_string();
        file_path.push("/uevent");
        println!("{file_path:?}");

        let data = fs::read_to_string(file_path).unwrap();
        println!("{data}");
        if data.contains("InputPlumber Unified Gamepad") {
            device_sys_path = Some(path);
            break;
        }
    }

    // Get the hidraw name of the device
    let device_sys_path = device_sys_path.expect("Unable to find Unified Controller");
    let mut hid_sys_path = device_sys_path.path().into_os_string();
    hid_sys_path.push("/hidraw");
    let paths = fs::read_dir(hid_sys_path).expect("Unable to read HID device info");
    let device_name = paths.into_iter().next().unwrap().unwrap().file_name();

    println!("Found Unified Controller device: {device_name:?}");
    println!("Starting Unified Controller driver!");

    let path = format!("/dev/{}", device_name.to_string_lossy());
    let mut driver = Driver::new(path).expect("Failed to create driver instance");

    loop {
        let events = driver.poll().expect("Failed to poll device");
        for event in events.into_iter() {
            println!("Event: {event:?}");
        }
        sleep(Duration::from_millis(1));
    }
}
