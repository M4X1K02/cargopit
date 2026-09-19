use crate::consts::*;
use crate::libconfig::{self, Value};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct DeviceEntry {
    pub fields: Vec<(String, Value)>,
}

impl DeviceEntry {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    pub fn string(&self, key: &str) -> Option<String> {
        match self.get(key)? {
            Value::String(text) => Some(text.clone()),
            Value::Int(number) => Some(number.to_string()),
            Value::Float(number) => Some(number.to_string()),
            Value::Bool(flag) => Some(flag.to_string()),
            _ => None,
        }
    }

    pub fn string_or(&self, key: &str, fallback: &str) -> String {
        self.string(key).unwrap_or_else(|| fallback.to_string())
    }

    pub fn int_or(&self, key: &str, fallback: i64) -> i64 {
        match self.get(key) {
            Some(Value::Int(number)) => *number,
            Some(Value::Float(number)) => *number as i64,
            Some(Value::String(text)) => text.parse().unwrap_or(fallback),
            _ => fallback,
        }
    }

    pub fn float_or(&self, key: &str, fallback: f64) -> f64 {
        match self.get(key) {
            Some(Value::Float(number)) => *number,
            Some(Value::Int(number)) => *number as f64,
            Some(Value::String(text)) => text.parse().unwrap_or(fallback),
            _ => fallback,
        }
    }

    pub fn bool_or(&self, key: &str, fallback: bool) -> bool {
        match self.get(key) {
            Some(Value::Bool(flag)) => *flag,
            Some(Value::Int(number)) => *number != 0,
            Some(Value::String(text)) => text.eq_ignore_ascii_case("true"),
            _ => fallback,
        }
    }

    pub fn set(&mut self, key: &str, value: Value) {
        if let Some(entry) = self.fields.iter_mut().find(|(name, _)| name == key) {
            entry.1 = value;
            return;
        }
        self.fields.push((key.to_string(), value));
    }

    pub fn set_str(&mut self, key: &str, value: &str) {
        if value.is_empty() {
            self.fields.retain(|(name, _)| name != key);
            return;
        }
        self.set(key, Value::String(value.to_string()));
    }

    pub fn class(&self) -> String {
        self.string_or(DEVICE_CLASS_KEY, CLASS_SOUND)
    }

    pub fn summary(&self) -> String {
        let class = self.class();
        if class.eq_ignore_ascii_case(CLASS_SOUND) {
            format!(
                "{} - {} - {}",
                class,
                self.string_or(EFFECT_KEY, ""),
                self.string_or(TYRE_KEY, "")
            )
        } else {
            format!(
                "{} - {} - {}",
                class,
                self.string_or(DEVICE_TYPE_KEY, ""),
                self.string_or(DEVICE_SUBTYPE_KEY, "")
            )
        }
    }

    pub fn identity(&self) -> String {
        self.string(DEVID_KEY)
            .or_else(|| self.string(DEVPATH_KEY))
            .unwrap_or_default()
    }

    pub fn has_haptic_fields(&self) -> bool {
        let class = self.class();
        if class.eq_ignore_ascii_case(CLASS_SOUND) {
            return true;
        }
        let type_name = self.string_or(DEVICE_TYPE_KEY, "");
        type_name.eq_ignore_ascii_case(TYPE_HAPTIC)
            || type_name.eq_ignore_ascii_case(TYPE_SOUND_HAPTIC)
            || type_name.eq_ignore_ascii_case("UsbHaptic")
            || type_name.eq_ignore_ascii_case("SerialHaptic")
            || type_name.eq_ignore_ascii_case(TYPE_WHEEL)
            || type_name.eq_ignore_ascii_case("UsbWheel")
    }

    pub fn has_led_fields(&self) -> bool {
        let type_name = self.string_or(DEVICE_TYPE_KEY, "");
        type_name.eq_ignore_ascii_case(TYPE_SIM_LED) || type_name.eq_ignore_ascii_case("SimLed")
    }
}

#[derive(Clone, Debug, Default)]
pub struct SimConfig {
    pub sim: String,
    pub car: String,
    pub api: String,
    pub extra: Vec<(String, Value)>,
    pub devices: Vec<DeviceEntry>,
}

impl SimConfig {
    pub fn label(&self) -> String {
        let api = if self.api.is_empty() {
            DEFAULT_VALUE
        } else {
            &self.api
        };
        format!("{} / {} / {}", self.sim, self.car, api)
    }

    pub fn is_default_entry(&self) -> bool {
        matches_any(&self.sim) && matches_any(&self.car) && matches_any(&self.api)
    }
}

#[derive(Clone, Debug, Default)]
pub struct MonocoqueFile {
    pub configs: Vec<SimConfig>,
}

impl MonocoqueFile {
    pub fn empty() -> Self {
        Self {
            configs: vec![SimConfig {
                sim: DEFAULT_SIM.to_string(),
                car: DEFAULT_CAR.to_string(),
                api: DEFAULT_API.to_string(),
                extra: Vec::new(),
                devices: Vec::new(),
            }],
        }
    }

    pub fn default_config_index(&self) -> usize {
        self.configs
            .iter()
            .position(SimConfig::is_default_entry)
            .unwrap_or(0)
    }
}

fn matches_any(value: &str) -> bool {
    value.is_empty()
        || value.eq_ignore_ascii_case(DEFAULT_VALUE)
        || value.eq_ignore_ascii_case(ALL_VALUE)
}

pub fn load_from_path(path: &Path) -> Result<MonocoqueFile> {
    if !path.exists() {
        return Ok(MonocoqueFile::empty());
    }
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_monocoque(&text)
}

pub fn parse_monocoque(text: &str) -> Result<MonocoqueFile> {
    let root = libconfig::parse(text)?;
    let configs_value = root
        .iter()
        .find(|(name, _)| name == CONFIGS_KEY)
        .map(|(_, value)| value)
        .context("config file has no configs array")?;
    let items = configs_value
        .as_array()
        .context("configs is not an array")?;
    let mut file = MonocoqueFile {
        configs: Vec::new(),
    };
    for item in items {
        file.configs.push(sim_config_from_group(item)?);
    }
    if file.configs.is_empty() {
        return Ok(MonocoqueFile::empty());
    }
    Ok(file)
}

fn sim_config_from_group(value: &Value) -> Result<SimConfig> {
    let entries = value.as_group().context("config entry is not a group")?;
    let mut sim = SimConfig::default();
    sim.sim = DEFAULT_SIM.to_string();
    sim.car = DEFAULT_CAR.to_string();
    for (name, child) in entries {
        match name.as_str() {
            SIM_KEY => sim.sim = scalar_to_string(child),
            CAR_KEY => sim.car = scalar_to_string(child),
            API_KEY => sim.api = scalar_to_string(child),
            DEVICES_KEY => {
                let devices = child.as_array().context("devices is not an array")?;
                for device in devices {
                    let fields = device
                        .as_group()
                        .context("device entry is not a group")?
                        .to_vec();
                    sim.devices.push(DeviceEntry { fields });
                }
            }
            _ => sim.extra.push((name.clone(), child.clone())),
        }
    }
    Ok(sim)
}

fn scalar_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Int(number) => number.to_string(),
        Value::Float(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        _ => String::new(),
    }
}

pub fn save_to_path(path: &Path, file: &MonocoqueFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let rendered = render_monocoque(file);
    fs::write(path, rendered).with_context(|| format!("failed to write {}", path.display()))
}

pub fn render_monocoque(file: &MonocoqueFile) -> String {
    let mut configs = Vec::new();
    for sim in &file.configs {
        let mut group: Vec<(String, Value)> = vec![
            (SIM_KEY.to_string(), Value::String(sim.sim.clone())),
            (CAR_KEY.to_string(), Value::String(sim.car.clone())),
        ];
        if !sim.api.is_empty() {
            group.push((API_KEY.to_string(), Value::String(sim.api.clone())));
        }
        group.extend(sim.extra.iter().cloned());
        let devices = sim
            .devices
            .iter()
            .map(|device| Value::Group(device.fields.clone()))
            .collect();
        group.push((DEVICES_KEY.to_string(), Value::Array(devices)));
        configs.push(Value::Group(group));
    }
    libconfig::render(&[(CONFIGS_KEY.to_string(), Value::Array(configs))])
}

pub fn new_device(class: &str) -> DeviceEntry {
    let mut device = DeviceEntry::default();
    device.set_str(DEVICE_CLASS_KEY, class);
    device.set(ENABLED_KEY, Value::Bool(true));
    device.set(FPS_KEY, Value::Int(DEFAULT_FPS as i64));
    if class.eq_ignore_ascii_case(CLASS_SOUND) {
        device.set_str(DEVICE_TYPE_KEY, TYPE_HAPTIC);
        device.set_str(EFFECT_KEY, EFFECT_NAMES[0]);
        device.set_str(MODULATION_KEY, MODULATION_NAMES[1]);
        device.set(VOLUME_KEY, Value::Int(DEFAULT_VOLUME as i64));
        device.set(CHANNELS_KEY, Value::Int(DEFAULT_CHANNELS as i64));
        device.set(PAN_KEY, Value::Int(DEFAULT_PAN as i64));
        device.set(NOISE_KEY, Value::Int(DEFAULT_NOISE as i64));
        device.set(FREQUENCY_KEY, Value::Int(DEFAULT_FREQUENCY as i64));
        device.set(AMPLITUDE_KEY, Value::Int(DEFAULT_AMPLITUDE as i64));
        device.set(FREQUENCY_MAX_KEY, Value::Int(DEFAULT_FREQUENCY_MAX as i64));
        device.set(AMPLITUDE_MAX_KEY, Value::Int(DEFAULT_AMPLITUDE as i64));
        device.set(THRESHOLD_KEY, Value::Float(DEFAULT_THRESHOLD));
        device.set(DURATION_KEY, Value::Float(DEFAULT_DURATION));
    } else if class.eq_ignore_ascii_case(CLASS_SERIAL) {
        device.set_str(DEVICE_TYPE_KEY, TYPE_SHIFT_LIGHTS);
        device.set(BAUD_KEY, Value::Int(DEFAULT_BAUD as i64));
        device.set(FANPOWER_KEY, Value::Float(DEFAULT_FANPOWER));
        device.set(AMPFACTOR_KEY, Value::Float(DEFAULT_AMPFACTOR));
        device.set(NUMLEDS_KEY, Value::Int(DEFAULT_NUMLEDS as i64));
        device.set(STARTLED_KEY, Value::Int(DEFAULT_STARTLED as i64));
        device.set(ENDLED_KEY, Value::Int(DEFAULT_ENDLED as i64));
    } else {
        device.set_str(DEVICE_TYPE_KEY, TYPE_TACHOMETER);
        device.set_str(DEVICE_SUBTYPE_KEY, USB_TACH_SUBTYPES[0]);
        device.set(GRANULARITY_KEY, Value::Int(DEFAULT_GRANULARITY as i64));
    }
    device
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example_config() {
        let src = include_str!("../../conf/monocoque.config");
        let file = parse_monocoque(src).expect("example config should parse");
        assert!(!file.configs.is_empty());
        assert!(file.configs[0].devices.len() > 1);
        assert!(file.configs[0]
            .devices
            .iter()
            .any(|device| device.class().eq_ignore_ascii_case(CLASS_SOUND)));
    }

    #[test]
    fn roundtrip_minimal() {
        let src = r#"
            configs = (
                {
                    sim = "default";
                    car = "default";
                    devices = (
                        {
                            device = "Sound";
                            effect = "Engine";
                            devid = "sink0";
                            volume = 70;
                        }
                    );
                }
            );
        "#;
        let file = parse_monocoque(src).unwrap();
        assert_eq!(file.configs[0].devices.len(), 1);
        assert_eq!(
            file.configs[0].devices[0].string_or(EFFECT_KEY, ""),
            "Engine"
        );
        let rendered = render_monocoque(&file);
        let again = parse_monocoque(&rendered).unwrap();
        assert_eq!(
            again.configs[0].devices[0].string_or(DEVID_KEY, ""),
            "sink0"
        );
    }
}
