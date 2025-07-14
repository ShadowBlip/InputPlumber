use crate::{
    dbus::interface::Unregisterable,
    udev::device::{AttributeGetter, AttributeSetter, UdevDevice},
};
use zbus::fdo;
use zbus_macros::interface;

/// DBusInterface exposing information about a HIDRaw device
pub struct SourceIioImuInterface {
    device: UdevDevice,
}

impl SourceIioImuInterface {
    pub fn new(device: UdevDevice) -> SourceIioImuInterface {
        SourceIioImuInterface { device }
    }
}

#[interface(name = "org.shadowblip.Input.Source.IIOIMUDevice")]
impl SourceIioImuInterface {
    /// Returns the human readable name of the device (e.g. XBox 360 Pad)
    #[zbus(property)]
    fn id(&self) -> fdo::Result<String> {
        Ok(self.device.sysname())
    }

    /// Returns the human readable name of the device (e.g. XBox 360 Pad)
    #[zbus(property)]
    fn name(&self) -> fdo::Result<String> {
        Ok(self.device.name())
    }

    #[zbus(property)]
    async fn accel_sample_rate(&self) -> fdo::Result<f64> {
        let Ok(dev) = self.device.get_device() else {
            return Ok(0.0);
        };
        Ok(dev
            .get_attribute_from_tree("in_accel_sampling_frequency")
            .parse()
            .unwrap_or_default())
    }

    #[zbus(property)]
    async fn accel_sample_rates_avail(&self) -> fdo::Result<Vec<f64>> {
        let Ok(dev) = self.device.get_device() else {
            return Ok(vec![0.0]);
        };
        let v = dev.get_attribute_from_tree("in_accel_sampling_frequency_available");

        let mut all_scales = Vec::new();
        for val in v.split_whitespace() {
            // convert the string into f64
            all_scales.push(val.parse::<f64>().unwrap_or_default());
        }
        Ok(all_scales)
    }

    #[zbus(property)]
    async fn angvel_sample_rate(&self) -> fdo::Result<f64> {
        let Ok(dev) = self.device.get_device() else {
            return Ok(0.0);
        };
        Ok(dev
            .get_attribute_from_tree("in_anglvel_sampling_frequency")
            .parse()
            .unwrap_or_default())
    }

    #[zbus(property)]
    async fn angvel_sample_rates_avail(&self) -> fdo::Result<Vec<f64>> {
        let Ok(dev) = self.device.get_device() else {
            return Ok(vec![0.0]);
        };
        let v = dev.get_attribute_from_tree("in_anglvel_sampling_frequency_available");

        let mut all_scales = Vec::new();
        for val in v.split_whitespace() {
            // convert the string into f64
            all_scales.push(val.parse::<f64>().unwrap_or_default());
        }
        Ok(all_scales)
    }

    #[zbus(property)]
    async fn set_accel_sample_rate(&self, sample_rate: f64) -> zbus::Result<()> {
        let Ok(mut dev) = self.device.get_device() else {
            return Ok(());
        };
        match dev.set_attribute_on_tree(
            "in_accel_sampling_frequency",
            sample_rate.to_string().as_str(),
        ) {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn set_angvel_sample_rate(&self, sample_rate: f64) -> zbus::Result<()> {
        let Ok(mut dev) = self.device.get_device() else {
            return Ok(());
        };
        match dev.set_attribute_on_tree(
            "in_anglvel_sampling_frequency",
            sample_rate.to_string().as_str(),
        ) {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn accel_scale(&self) -> fdo::Result<f64> {
        let Ok(dev) = self.device.get_device() else {
            return Ok(0.0);
        };
        Ok(dev
            .get_attribute_from_tree("in_accel_scale")
            .parse()
            .unwrap_or_default())
    }

    #[zbus(property)]
    async fn accel_scales_avail(&self) -> fdo::Result<Vec<f64>> {
        let Ok(dev) = self.device.get_device() else {
            return Ok(vec![0.0]);
        };
        let v = dev.get_attribute_from_tree("in_accel_scale_available");

        let mut all_scales = Vec::new();
        for val in v.split_whitespace() {
            // convert the string into f64
            all_scales.push(val.parse::<f64>().unwrap_or_default());
        }
        Ok(all_scales)
    }

    #[zbus(property)]
    async fn angvel_scale(&self) -> fdo::Result<f64> {
        let Ok(dev) = self.device.get_device() else {
            return Ok(0.0);
        };
        Ok(dev
            .get_attribute_from_tree("in_anglvel_scale")
            .parse()
            .unwrap_or_default())
    }

    #[zbus(property)]
    async fn angvel_scales_avail(&self) -> fdo::Result<Vec<f64>> {
        let Ok(dev) = self.device.get_device() else {
            return Ok(vec![0.0]);
        };
        let v = dev.get_attribute_from_tree("in_anglvel_scale_available");

        let mut all_scales = Vec::new();
        for val in v.split_whitespace() {
            // convert the string into f64
            all_scales.push(val.parse::<f64>().unwrap_or_default());
        }
        Ok(all_scales)
    }

    #[zbus(property)]
    async fn set_accel_scale(&self, scale: f64) -> zbus::Result<()> {
        let Ok(mut dev) = self.device.get_device() else {
            return Ok(());
        };
        match dev.set_attribute_on_tree("in_accel_scale", scale.to_string().as_str()) {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }

    #[zbus(property)]
    async fn set_angvel_scale(&self, scale: f64) -> zbus::Result<()> {
        let Ok(mut dev) = self.device.get_device() else {
            return Ok(());
        };
        match dev.set_attribute_on_tree("in_anglvel_scale", scale.to_string().as_str()) {
            Ok(result) => Ok(result),
            Err(e) => Err(zbus::Error::Failure(e.to_string())),
        }
    }
}

impl Unregisterable for SourceIioImuInterface {}
