use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::consts;
use crate::libconfig::{self, Value};
use crate::schema::{self, DeviceClass};

#[derive(Debug, Clone, PartialEq)]
pub struct CargopitConfig {
    pub profiles: Vec<SimProfile>,
    pub extra: Vec<(String, Value)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimProfile {
    pub sim: String,
    pub car: String,
    pub api: Option<String>,
    pub devices: Vec<DeviceEntry>,
    pub extra: Vec<(String, Value)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceEntry {
    pub settings: Vec<(String, Value)>,
}

impl Default for CargopitConfig {
    fn default() -> Self {
        Self {
            profiles: vec![SimProfile::default()],
            extra: Vec::new(),
        }
    }
}

impl Default for SimProfile {
    fn default() -> Self {
        Self {
            sim: consts::DEFAULT_SIM.to_string(),
            car: consts::DEFAULT_CAR.to_string(),
            api: None,
            devices: Vec::new(),
            extra: Vec::new(),
        }
    }
}

impl DeviceEntry {
    pub fn new() -> Self {
        Self {
            settings: Vec::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        libconfig::group_get(&self.settings, key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Value::as_i64)
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }

    pub fn set(&mut self, key: &str, value: Value) {
        libconfig::group_set(&mut self.settings, key, value);
    }

    pub fn set_str(&mut self, key: &str, value: impl Into<String>) {
        self.set(key, Value::String(value.into()));
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.set(key, Value::Bool(value));
    }

    pub fn set_int(&mut self, key: &str, value: i64) {
        self.set(key, Value::Int(value));
    }

    pub fn set_float(&mut self, key: &str, value: f64) {
        self.set(key, Value::Float(value));
    }

    pub fn remove(&mut self, key: &str) {
        libconfig::group_remove(&mut self.settings, key);
    }

    pub fn class(&self) -> DeviceClass {
        match self.get_str(consts::KEY_DEVICE) {
            Some(consts::CLASS_USB) => DeviceClass::Usb,
            Some(consts::CLASS_SERIAL) => DeviceClass::Serial,
            _ => DeviceClass::Sound,
        }
    }

    pub fn type_name(&self) -> &str {
        self.get_str(consts::KEY_TYPE).unwrap_or(consts::TYPE_HAPTIC)
    }

    pub fn enabled(&self) -> bool {
        self.get_bool(consts::KEY_ENABLED).unwrap_or(consts::DEFAULT_ENABLED)
    }

    pub fn identity(&self) -> String {
        self.get_str(consts::KEY_DEVID)
            .or_else(|| self.get_str(consts::KEY_DEVPATH))
            .unwrap_or("")
            .to_string()
    }

    pub fn summary(&self) -> String {
        let class = self.get_str(consts::KEY_DEVICE).unwrap_or("?");
        let kind = self.get_str(consts::KEY_TYPE).unwrap_or("-");
        let extra = self
            .get_str(consts::KEY_SUBTYPE)
            .or_else(|| self.get_str(consts::KEY_EFFECT))
            .unwrap_or("");
        if extra.is_empty() {
            format!("{class} - {kind}")
        } else {
            format!("{class} - {kind} - {extra}")
        }
    }

    pub fn keep_keys_for_class(&mut self, class: DeviceClass, type_name: &str) {
        let allowed = schema::allowed_keys(class, type_name);
        self.settings
            .retain(|(key, _)| allowed.iter().any(|allowed_key| *allowed_key == key.as_str()));
        self.set_str(consts::KEY_DEVICE, class.as_str());
        if class == DeviceClass::Sound && type_name == consts::TYPE_HAPTIC {
            self.remove(consts::KEY_TYPE);
        } else {
            self.set_str(consts::KEY_TYPE, type_name);
        }
    }
}

impl Default for DeviceEntry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_cargopit(src: &str) -> Result<CargopitConfig> {
    let root = libconfig::parse(src).map_err(|err| anyhow!("{err}"))?;
    from_value(&root)
}

pub fn from_value(root: &Value) -> Result<CargopitConfig> {
    let Some(group) = root.as_group() else {
        return Err(anyhow!("config root must be a group"));
    };
    let mut extra = Vec::new();
    let mut profiles = Vec::new();
    for (key, value) in group {
        if key == consts::KEY_CONFIGS {
            let list = value
                .as_list()
                .ok_or_else(|| anyhow!("configs must be a list"))?;
            for item in list {
                profiles.push(profile_from_value(item)?);
            }
            continue;
        }
        extra.push((key.clone(), value.clone()));
    }
    if profiles.is_empty() {
        profiles.push(SimProfile::default());
    }
    Ok(CargopitConfig { profiles, extra })
}

fn profile_from_value(value: &Value) -> Result<SimProfile> {
    let Some(group) = value.as_group() else {
        return Err(anyhow!("profile must be a group"));
    };
    let mut profile = SimProfile::default();
    let mut extra = Vec::new();
    for (key, item) in group {
        match key.as_str() {
            consts::KEY_SIM => {
                profile.sim = item.as_str().unwrap_or(consts::DEFAULT_SIM).to_string();
            }
            consts::KEY_CAR => {
                profile.car = item.as_str().unwrap_or(consts::DEFAULT_CAR).to_string();
            }
            consts::KEY_API => {
                profile.api = item.as_str().map(str::to_string);
            }
            consts::KEY_DEVICES => {
                let list = item
                    .as_list()
                    .ok_or_else(|| anyhow!("devices must be a list"))?;
                profile.devices = list
                    .iter()
                    .map(device_from_value)
                    .collect::<Result<Vec<_>>>()?;
            }
            _ => extra.push((key.clone(), item.clone())),
        }
    }
    profile.extra = extra;
    Ok(profile)
}

fn device_from_value(value: &Value) -> Result<DeviceEntry> {
    let Some(group) = value.as_group() else {
        return Err(anyhow!("device must be a group"));
    };
    Ok(DeviceEntry {
        settings: group.to_vec(),
    })
}

pub fn to_value(config: &CargopitConfig) -> Value {
    let mut root = config.extra.clone();
    let profiles = config
        .profiles
        .iter()
        .map(profile_to_value)
        .collect::<Vec<_>>();
    libconfig::group_set(&mut root, consts::KEY_CONFIGS, Value::List(profiles));
    Value::Group(root)
}

fn profile_to_value(profile: &SimProfile) -> Value {
    let mut items = profile.extra.clone();
    libconfig::group_set(
        &mut items,
        consts::KEY_SIM,
        Value::String(profile.sim.clone()),
    );
    libconfig::group_set(
        &mut items,
        consts::KEY_CAR,
        Value::String(profile.car.clone()),
    );
    if let Some(api) = &profile.api {
        libconfig::group_set(&mut items, consts::KEY_API, Value::String(api.clone()));
    } else {
        libconfig::group_remove(&mut items, consts::KEY_API);
    }
    let devices = profile
        .devices
        .iter()
        .map(|device| Value::Group(device.settings.clone()))
        .collect();
    libconfig::group_set(&mut items, consts::KEY_DEVICES, Value::List(devices));
    Value::Group(items)
}

pub fn render(config: &CargopitConfig) -> String {
    libconfig::render(&to_value(config))
}

pub fn load_file(path: &Path) -> Result<CargopitConfig> {
    if !path.exists() {
        return Ok(CargopitConfig::default());
    }
    let src = fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    parse_cargopit(&src)
}

pub fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}{}",
        path.extension().and_then(|e| e.to_str()).unwrap_or("cfg"),
        consts::ATOMIC_SAVE_SUFFIX
    ));
    {
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("create {}", tmp.display()))?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path).with_context(|| format!("rename {}", tmp.display()))?;
    Ok(())
}

pub fn save_file(path: &Path, config: &CargopitConfig) -> Result<()> {
    let rendered = render(config);
    libconfig::parse(&rendered).map_err(|err| anyhow!("would-be file failed parse: {err}"))?;
    atomic_write(path, &rendered)
}

pub fn read_raw(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_config() {
        let path = crate::paths::source_root().join("conf").join(consts::CONFIG_FILE_NAME);
        let src = std::fs::read_to_string(&path).expect("sample config");
        let config = parse_cargopit(&src).expect("parse sample");
        assert!(!config.profiles.is_empty());
        assert!(!config.profiles[0].devices.is_empty());
        let usb = config.profiles[0]
            .devices
            .iter()
            .find(|d| d.class() == DeviceClass::Usb)
            .expect("usb device");
        assert_eq!(usb.get_str(consts::KEY_TYPE), Some(consts::TYPE_TACHOMETER));
    }

    #[test]
    fn round_trip_catalog_keys() {
        let mut device = DeviceEntry::new();
        device.set_str(consts::KEY_DEVICE, consts::CLASS_SERIAL);
        device.set_str(consts::KEY_TYPE, consts::TYPE_SIMLEDS);
        device.set_str(consts::KEY_DEVPATH, "/dev/simdev0");
        device.set_int(consts::KEY_BAUD, 115200);
        device.set_float(consts::KEY_AMPFACTOR, 1.0);
        device.set_int(consts::KEY_NUMLEDS, 6);
        device.set_int(consts::KEY_STARTLED, 2);
        device.set_int(consts::KEY_ENDLED, 5);
        device.set_str(consts::KEY_CONFIG, "basic_rpms.lua");
        device.set_bool(consts::KEY_ENABLED, true);
        let mut config = CargopitConfig::default();
        config.profiles[0].devices.push(device);
        let rendered = render(&config);
        let parsed = parse_cargopit(&rendered).unwrap();
        let again = &parsed.profiles[0].devices[0];
        assert_eq!(again.get_i64(consts::KEY_NUMLEDS), Some(6));
        assert_eq!(again.get_f64(consts::KEY_AMPFACTOR), Some(1.0));
        assert_eq!(again.get_str(consts::KEY_DEVPATH), Some("/dev/simdev0"));
    }
}
