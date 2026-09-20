use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config;
use crate::consts;
use crate::libconfig::{self, Value};
use crate::paths;

#[derive(Debug, Clone, PartialEq)]
pub struct SimdConfig {
    pub sims: Vec<SimdSim>,
    pub extra: Vec<(String, Value)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimdSim {
    pub settings: Vec<(String, Value)>,
}

impl Default for SimdConfig {
    fn default() -> Self {
        Self {
            sims: Vec::new(),
            extra: Vec::new(),
        }
    }
}

impl SimdSim {
    pub fn get_str(&self, key: &str) -> &str {
        libconfig::group_get(&self.settings, key)
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    pub fn name(&self) -> &str {
        self.get_str(consts::SIMD_FIELD_NAME)
    }

    pub fn display_name(&self) -> String {
        let name = self.name();
        if name.is_empty() {
            consts::SIMD_UNNAMED.to_string()
        } else {
            name.to_string()
        }
    }

    pub fn game_id(&self) -> Option<u64> {
        let value = libconfig::group_get(&self.settings, consts::SIMD_FIELD_GAMEID)?;
        if let Some(n) = value.as_i64() {
            if n < 0 {
                return None;
            }
            return Some(n as u64);
        }
        value.as_str()?.parse().ok()
    }
}

impl SimdConfig {
    pub fn name_for_game_id(&self, game_id: u64) -> Option<&str> {
        if game_id == 0 {
            return None;
        }
        self.sims.iter().find_map(|sim| {
            if sim.game_id() == Some(game_id) {
                Some(sim.name())
            } else {
                None
            }
        })
    }
}

pub fn load(path: &Path) -> Result<SimdConfig> {
    if !path.exists() {
        return Ok(SimdConfig::default());
    }
    let src = fs::read_to_string(path)?;
    parse(&src)
}

pub fn parse(src: &str) -> Result<SimdConfig> {
    let root = libconfig::parse(src).map_err(|err| anyhow!("{err}"))?;
    let Some(group) = root.as_group() else {
        return Err(anyhow!("simd.config root must be a group"));
    };
    let mut extra = Vec::new();
    let mut sims = Vec::new();
    for (key, value) in group {
        if key == consts::KEY_SIMS {
            let list = value.as_list().ok_or_else(|| anyhow!("sims must be a list"))?;
            for item in list {
                let settings = item
                    .as_group()
                    .ok_or_else(|| anyhow!("sim entry must be a group"))?
                    .to_vec();
                sims.push(SimdSim { settings });
            }
            continue;
        }
        extra.push((key.clone(), value.clone()));
    }
    Ok(SimdConfig { sims, extra })
}

pub fn to_value(config: &SimdConfig) -> Value {
    let mut root = config.extra.clone();
    let sims = config
        .sims
        .iter()
        .map(|sim| Value::Group(sim.settings.clone()))
        .collect();
    libconfig::group_set(&mut root, consts::KEY_SIMS, Value::List(sims));
    Value::Group(root)
}

pub fn save(path: &Path, config: &SimdConfig) -> Result<()> {
    config::atomic_write(path, &libconfig::render(&to_value(config)))
}

pub fn stub_from_bundled() -> SimdConfig {
    if let Some(path) = find_example() {
        if let Ok(src) = fs::read_to_string(path) {
            if let Ok(parsed) = parse(&src) {
                return parsed;
            }
        }
    }
    SimdConfig::default()
}

fn find_example() -> Option<std::path::PathBuf> {
    let candidates = [
        paths::source_root()
            .join("src/cargopit/simulatorapi/simapi/simd/conf")
            .join(consts::SIMD_CONFIG_FILE_NAME),
        paths::simd_config_path(),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

pub fn field_help(key: &str) -> &'static str {
    consts::SIMD_FIELD_HELP
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, help)| *help)
        .unwrap_or("Unknown key (preserved on save)")
}
