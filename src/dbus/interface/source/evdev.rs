use std::{collections::HashMap, error::Error};

use evdev::EventType;
use zbus::{fdo, Connection};
use zbus_macros::interface;

use crate::{
    input::source::evdev::{get_capabilities, get_dbus_path},
    udev::device::UdevDevice,
};

/// The [SourceEventDeviceInterface] provides a DBus interface that can be exposed for managing
/// a [Manager]. It works by sending command messages to a channel that the
/// [Manager] is listening on.
pub struct SourceEventDeviceInterface {
    device: UdevDevice,
    capabilities: HashMap<EventType, Vec<u16>>,
}

impl SourceEventDeviceInterface {
    pub fn new(device: UdevDevice) -> SourceEventDeviceInterface {
        let handler = device.devnode();
        let capabilities = get_capabilities(handler.as_str()).unwrap_or_else(|e| {
            log::warn!("Failed to get capabilities for source evdev device '{handler}': {e:?}");
            HashMap::new()
        });
        SourceEventDeviceInterface {
            device,
            capabilities,
        }
    }

    /// Creates a new instance of the source evdev interface on DBus. Returns
    /// a structure with information about the source device.
    pub async fn listen_on_dbus(
        conn: Connection,
        sys_name: String,
        device: UdevDevice,
    ) -> Result<(), Box<dyn Error>> {
        log::debug!("Starting to listen on dbus interface for {sys_name}");
        let path = get_dbus_path(sys_name.clone());
        log::debug!("Got dbus path {path}");
        let iface = SourceEventDeviceInterface::new(device);
        log::debug!("Created interface for {sys_name}");
        tokio::task::spawn(async move {
            log::debug!("Starting dbus interface: {path}");
            let result = conn.object_server().at(path.clone(), iface).await;
            if let Err(e) = result {
                log::debug!("Failed to start dbus interface {path}: {e:?}");
            } else {
                log::debug!("Started dbus interface: {path}");
            }
        });
        Ok(())
    }

    /// Returns all the event codes for the given event type that this evdev device
    /// supports.
    pub fn supported_events(&self, event_type: &EventType) -> Vec<u16> {
        self.capabilities
            .get(event_type)
            .map(|caps| caps.to_owned())
            .unwrap_or_default()
    }
}

#[interface(name = "org.shadowblip.Input.Source.EventDevice")]
impl SourceEventDeviceInterface {
    /// Returns the detected device class of the device (e.g. "joystick", "touchscreen", etc.)
    #[zbus(property)]
    pub fn device_class(&self) -> fdo::Result<String> {
        let properties = self.device.get_properties();
        if properties.contains_key("ID_INPUT_KEYBOARD") {
            return Ok("keyboard".to_string());
        }
        if properties.contains_key("ID_INPUT_MOUSE") {
            return Ok("mouse".to_string());
        }
        if properties.contains_key("ID_INPUT_JOYSTICK") {
            return Ok("joystick".to_string());
        }
        if properties.contains_key("ID_INPUT_TABLET") {
            return Ok("tablet".to_string());
        }
        if properties.contains_key("ID_INPUT_TOUCHPAD") {
            return Ok("touchpad".to_string());
        }
        if properties.contains_key("ID_INPUT_TOUCHSCREEN") {
            return Ok("touchscreen".to_string());
        }
        if properties.contains_key("ID_INPUT_SWITCH") {
            return Ok("switch".to_string());
        }
        if properties.contains_key("ID_INPUT_ACCELEROMETER") {
            return Ok("imu".to_string());
        }
        if properties.contains_key("ID_INPUT_POINTINGSTICK") {
            return Ok("pointer".to_string());
        }
        Ok("other".to_string())
    }

    /// Returns the full device node path to the device (e.g. /dev/input/event3)
    #[zbus(property)]
    pub fn device_path(&self) -> fdo::Result<String> {
        Ok(self.device.devnode())
    }

    /// Returns the bus type of the device
    #[zbus(property)]
    async fn id_bustype(&self) -> fdo::Result<String> {
        Ok(format!("{}", self.device.id_bustype()))
    }

    /// Returns the product id of the device
    #[zbus(property)]
    async fn id_product(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.device.id_product()))
    }

    /// Returns the vendor id of the device
    #[zbus(property)]
    async fn id_vendor(&self) -> fdo::Result<String> {
        Ok(format!("{:04x}", self.device.id_vendor()))
    }

    /// Returns the version id of the device
    #[zbus(property)]
    async fn id_version(&self) -> fdo::Result<String> {
        Ok(format!("{}", self.device.id_version()))
    }

    /// Returns the human readable name of the device (e.g. XBox 360 Pad)
    #[zbus(property)]
    async fn name(&self) -> fdo::Result<String> {
        Ok(self.device.name())
    }

    /// Returns the phys_path of the device (e.g usb-0000:07:00.3-2/input0)
    #[zbus(property)]
    async fn phys_path(&self) -> fdo::Result<String> {
        Ok(self.device.phys())
    }

    /// Returns the full sysfs path of the device (e.g. /sys/devices/pci0000:00)
    #[zbus(property)]
    async fn sysfs_path(&self) -> fdo::Result<String> {
        Ok(self.device.devpath())
    }

    /// Returns the uniq of the device
    #[zbus(property)]
    async fn unique_id(&self) -> fdo::Result<String> {
        Ok(self.device.uniq())
    }

    /// Returns the set of supported keys reported by the device.
    ///
    /// For keyboards, this is the set of all possible keycodes the keyboard may emit. Controllers,
    /// mice, and other peripherals may also report buttons as keys.
    #[zbus(property)]
    async fn supported_keys(&self) -> fdo::Result<Vec<u16>> {
        Ok(self.supported_events(&EventType::KEY))
    }

    /// Returns the set of supported "relative axes" reported by the device.
    ///
    /// Standard mice will generally report `REL_X` and `REL_Y` along with wheel if supported.
    #[zbus(property)]
    async fn supported_relative_axes(&self) -> fdo::Result<Vec<u16>> {
        Ok(self.supported_events(&EventType::RELATIVE))
    }

    /// Returns the set of supported "absolute axes" reported by the device.
    ///
    /// These are most typically supported by joysticks and touchpads.
    #[zbus(property)]
    async fn supported_absolute_axes(&self) -> fdo::Result<Vec<u16>> {
        Ok(self.supported_events(&EventType::ABSOLUTE))
    }

    /// Returns the set of supported switches reported by the device.
    ///
    /// These are typically used for things like software switches on laptop lids (which the
    /// system reacts to by suspending or locking), or virtual switches to indicate whether a
    /// headphone jack is plugged in (used to disable external speakers).
    #[zbus(property)]
    async fn supported_switches(&self) -> fdo::Result<Vec<u16>> {
        Ok(self.supported_events(&EventType::SWITCH))
    }

    /// Returns a set of supported LEDs on the device.
    ///
    /// Most commonly these are state indicator lights for things like Scroll Lock, but they
    /// can also be found in cameras and other devices.
    #[zbus(property)]
    async fn supported_leds(&self) -> fdo::Result<Vec<u16>> {
        Ok(self.supported_events(&EventType::LED))
    }

    /// Returns the set of supported simple sounds supported by a device.
    ///
    /// You can use these to make really annoying beep sounds come from an internal self-test
    /// speaker, for instance.
    #[zbus(property)]
    async fn supported_sounds(&self) -> fdo::Result<Vec<u16>> {
        Ok(self.supported_events(&EventType::SOUND))
    }

    /// Returns the set of supported force feedback effects supported by a device.
    #[zbus(property)]
    async fn supported_ff(&self) -> fdo::Result<Vec<u16>> {
        Ok(self.supported_events(&EventType::FORCEFEEDBACK))
    }
}
