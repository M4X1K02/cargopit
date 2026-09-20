use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::consts;
use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayFlags {
    #[serde(default)]
    pub verbosity: u8,
    #[serde(default)]
    pub disable_audio: bool,
    #[serde(default)]
    pub udp: bool,
    #[serde(default)]
    pub fps: Option<i64>,
    #[serde(default)]
    pub log_file: Option<String>,
}

impl Default for PlayFlags {
    fn default() -> Self {
        Self {
            verbosity: consts::DEFAULT_VERBOSITY,
            disable_audio: false,
            udp: false,
            fps: None,
            log_file: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TuiState {
    #[serde(default)]
    pub play_flags: PlayFlags,
    #[serde(default)]
    pub config_path: Option<String>,
}

impl TuiState {
    pub fn config_path(&self) -> std::path::PathBuf {
        self.config_path
            .as_ref()
            .map(|p| paths::expand_tilde(p))
            .unwrap_or_else(paths::default_config_path)
    }
}

pub fn load() -> TuiState {
    load_from(&paths::tui_state_path()).unwrap_or_default()
}

pub fn load_from(path: &Path) -> Result<TuiState> {
    if !path.exists() {
        return Ok(TuiState::default());
    }
    let src = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&src)?)
}

pub fn save(state: &TuiState) -> Result<()> {
    let path = paths::tui_state_path();
    let json = serde_json::to_string_pretty(state)?;
    config::atomic_write(&path, &json)
}
