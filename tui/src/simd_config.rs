use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config;
use crate::consts;
use crate::libconfig::{self, Value};
use crate::paths;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimdConfig {
    pub sims: Vec<SimdSim>,
    pub extra: Vec<(String, Value)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimdSim {
    pub settings: Vec<(String, Value)>,
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

    pub fn field_display(&self, key: &str) -> String {
        if key == consts::SIMD_FIELD_TELEMETRY {
            return self.telemetry_source().to_string();
        }
        let Some(value) = libconfig::group_get(&self.settings, key) else {
            return String::new();
        };
        match value {
            Value::String(s) => s.clone(),
            Value::Int(n) => n.to_string(),
            Value::Float(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => consts::SIMD_VALUE_COMPLEX.to_string(),
        }
    }

    pub fn telemetry_source(&self) -> &'static str {
        if let Some(canonical) =
            canonical_telemetry_source(self.get_str(consts::SIMD_FIELD_TELEMETRY))
        {
            return canonical;
        }
        match libconfig::group_get(&self.settings, consts::SIMD_FIELD_USEUDP)
            .and_then(Value::as_bool)
        {
            Some(true) => consts::SIMD_TELEMETRY_UDP,
            Some(false) => consts::SIMD_TELEMETRY_SHM,
            None => consts::SIMD_TELEMETRY_AUTO,
        }
    }

    pub fn set_telemetry_source(&mut self, source: &'static str) {
        libconfig::group_set(
            &mut self.settings,
            consts::SIMD_FIELD_TELEMETRY,
            Value::String(source.to_string()),
        );
        libconfig::group_remove(&mut self.settings, consts::SIMD_FIELD_USEUDP);
    }

    pub fn cycle_telemetry_source(&mut self, delta: i32) -> &'static str {
        let next = cycle_telemetry_source(self.telemetry_source(), delta);
        self.set_telemetry_source(next);
        next
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
    pub fn index_for_game_id(&self, game_id: u64) -> Option<usize> {
        if game_id == consts::SETTINGS_GAME_IDLE {
            return None;
        }
        self.sims
            .iter()
            .position(|sim| sim.game_id() == Some(game_id))
    }

    pub fn name_for_game_id(&self, game_id: u64) -> Option<&str> {
        let index = self.index_for_game_id(game_id)?;
        let name = self.sims.get(index)?.name();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
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
            let list = value
                .as_list()
                .ok_or_else(|| anyhow!("sims must be a list"))?;
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

pub fn table_columns(config: &SimdConfig) -> Vec<String> {
    let mut columns: Vec<String> = consts::SIMD_TABLE_COLUMNS
        .iter()
        .map(|key| (*key).to_string())
        .collect();
    for sim in &config.sims {
        for (key, _) in &sim.settings {
            if columns.iter().any(|column| column == key) {
                continue;
            }
            columns.push(key.clone());
        }
    }
    columns
}

pub fn column_min_width(key: &str) -> u16 {
    match key {
        consts::SIMD_FIELD_NAME => consts::SIMD_COL_NAME_MIN,
        consts::SIMD_FIELD_GAMEID => consts::SIMD_COL_GAMEID,
        consts::SIMD_FIELD_LAUNCHEXE | consts::SIMD_FIELD_LIVEEXE => consts::SIMD_COL_EXE_MIN,
        consts::SIMD_FIELD_BRIDGEDELAY => consts::SIMD_COL_BRIDGE,
        consts::SIMD_FIELD_SIMAPI => consts::SIMD_COL_SIMAPI,
        consts::SIMD_FIELD_TELEMETRY => consts::SIMD_COL_TELEMETRY,
        _ => consts::SIMD_COL_EXTRA_MIN,
    }
}

pub fn column_content_width(config: &SimdConfig, key: &str) -> u16 {
    let mut width = column_min_width(key).max(text_cols(key));
    for sim in &config.sims {
        width = width.max(text_cols(&sim.field_display(key)));
    }
    width
}

pub fn column_widths(config: &SimdConfig, columns: &[String]) -> Vec<u16> {
    columns
        .iter()
        .map(|key| column_content_width(config, key))
        .collect()
}

pub fn column_window(widths: &[u16], selected: usize, budget: u16) -> (usize, usize) {
    if widths.is_empty() || budget == 0 {
        return (0, 0);
    }
    let selected = selected.min(widths.len() - 1);
    let mut start = selected;
    let mut end = selected + 1;
    let mut used = widths[selected].min(budget);
    while end < widths.len() {
        let extra = consts::SIMD_COL_SPACING + widths[end];
        if used.saturating_add(extra) > budget {
            break;
        }
        used = used.saturating_add(extra);
        end += 1;
    }
    while start > 0 {
        let extra = consts::SIMD_COL_SPACING + widths[start - 1];
        if used.saturating_add(extra) > budget {
            break;
        }
        used = used.saturating_add(extra);
        start -= 1;
    }
    (start, end)
}

fn text_cols(text: &str) -> u16 {
    u16::try_from(text.chars().count()).unwrap_or(u16::MAX)
}

fn canonical_telemetry_source(name: &str) -> Option<&'static str> {
    consts::SIMD_TELEMETRY_SOURCES
        .iter()
        .copied()
        .find(|source| *source == name)
}

fn cycle_telemetry_source(current: &str, delta: i32) -> &'static str {
    let index = consts::SIMD_TELEMETRY_SOURCES
        .iter()
        .position(|source| *source == current)
        .unwrap_or(0);
    let len = consts::SIMD_TELEMETRY_SOURCE_COUNT as i32;
    let next = (index as i32 + delta).rem_euclid(len) as usize;
    consts::SIMD_TELEMETRY_SOURCES[next]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_columns_keep_schema_then_extras() {
        let config = parse(
            r#"
sims = (
  { name = "Demo"; gameid = 1; custom = "kept"; }
);
"#,
        )
        .unwrap();
        let columns = table_columns(&config);
        assert_eq!(
            &columns[..consts::SIMD_FIELD_COUNT],
            &consts::SIMD_TABLE_COLUMNS
        );
        assert!(columns.iter().any(|column| column == "custom"));
        assert_eq!(
            config.sims[0].field_display(consts::SIMD_FIELD_NAME),
            "Demo"
        );
        assert_eq!(config.sims[0].field_display("custom"), "kept");
    }

    #[test]
    fn column_window_keeps_selected_visible() {
        let widths: Vec<u16> = consts::SIMD_TABLE_COLUMNS
            .iter()
            .map(|key| column_min_width(key))
            .collect();
        let last = widths.len() - 1;
        let tight = widths[last];
        let (start, end) = column_window(&widths, last, tight);
        assert_eq!(end, widths.len());
        assert!(start <= last);
        assert!(end > last);
        let (all_start, all_end) = column_window(&widths, 0, u16::MAX);
        assert_eq!((all_start, all_end), (0, widths.len()));
    }

    #[test]
    fn column_window_hides_overflow_at_content_width() {
        let config = parse(
            r#"
sims = (
  { name = "AssettoCorsaCompetizione"; launchexe = "AC2-Win64-Shipping.exe"; }
);
"#,
        )
        .unwrap();
        let columns = table_columns(&config);
        let widths = column_widths(&config, &columns);
        let name_width = column_content_width(&config, consts::SIMD_FIELD_NAME);
        assert!(name_width >= text_cols("AssettoCorsaCompetizione"));
        let (start, end) = column_window(&widths, 0, name_width);
        assert_eq!((start, end), (0, 1));
    }

    #[test]
    fn telemetry_defaults_to_auto_and_migrates_useudp() {
        let auto = parse(r#"sims = ( { name = "A"; gameid = 1; } );"#).unwrap();
        assert_eq!(auto.sims[0].telemetry_source(), consts::SIMD_TELEMETRY_AUTO);
        assert_eq!(
            auto.sims[0].field_display(consts::SIMD_FIELD_TELEMETRY),
            consts::SIMD_TELEMETRY_AUTO
        );
        let shm = parse(r#"sims = ( { name = "A"; gameid = 1; useudp = false; } );"#).unwrap();
        assert_eq!(shm.sims[0].telemetry_source(), consts::SIMD_TELEMETRY_SHM);
        let udp = parse(r#"sims = ( { name = "A"; gameid = 1; useudp = true; } );"#).unwrap();
        assert_eq!(udp.sims[0].telemetry_source(), consts::SIMD_TELEMETRY_UDP);
        let named = parse(r#"sims = ( { name = "A"; gameid = 1; telemetry = "udp"; } );"#).unwrap();
        assert_eq!(named.sims[0].telemetry_source(), consts::SIMD_TELEMETRY_UDP);
    }

    #[test]
    fn telemetry_cycle_writes_source_and_drops_useudp() {
        let mut config =
            parse(r#"sims = ( { name = "A"; gameid = 1; useudp = false; } );"#).unwrap();
        assert_eq!(
            config.sims[0].cycle_telemetry_source(1),
            consts::SIMD_TELEMETRY_UDP
        );
        assert_eq!(
            config.sims[0].get_str(consts::SIMD_FIELD_TELEMETRY),
            consts::SIMD_TELEMETRY_UDP
        );
        assert!(
            libconfig::group_get(&config.sims[0].settings, consts::SIMD_FIELD_USEUDP).is_none()
        );
        assert_eq!(
            config.sims[0].cycle_telemetry_source(1),
            consts::SIMD_TELEMETRY_AUTO
        );
    }

    #[test]
    fn index_for_game_id_skips_idle_and_empty() {
        let config = parse(
            r#"
sims = (
  { name = "One"; gameid = 11; },
  { name = "Two"; gameid = 22; }
);
"#,
        )
        .unwrap();
        assert_eq!(config.index_for_game_id(consts::SETTINGS_GAME_IDLE), None);
        assert_eq!(config.index_for_game_id(22), Some(1));
        assert_eq!(config.name_for_game_id(11), Some("One"));
        assert_eq!(config.name_for_game_id(99), None);
    }
}
