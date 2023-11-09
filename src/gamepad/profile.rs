use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct DeviceProfile {
    version: u32,
    kind: String,
    name: String,
}

impl DeviceProfile {
    fn from_yaml(content: String) -> Result<DeviceProfile, serde_yaml::Error> {
        let profile: DeviceProfile = serde_yaml::from_str(content.as_str())?;
        Ok(profile)
    }
}
