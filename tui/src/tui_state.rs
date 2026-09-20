use std::collections::BTreeMap;
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
    pub play_flags_by_game: BTreeMap<String, PlayFlags>,
    #[serde(default)]
    pub settings_game_id: u64,
    #[serde(default)]
    pub profile_index: usize,
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

    pub fn store_flags_for(&mut self, game_id: u64) {
        self.play_flags_by_game
            .insert(flags_key(game_id), self.play_flags.clone());
    }

    pub fn flags_for(&self, game_id: u64) -> PlayFlags {
        if let Some(flags) = self.play_flags_by_game.get(&flags_key(game_id)) {
            return flags.clone();
        }
        if game_id != consts::SETTINGS_GAME_IDLE {
            if let Some(flags) = self
                .play_flags_by_game
                .get(&flags_key(consts::SETTINGS_GAME_IDLE))
            {
                return flags.clone();
            }
        }
        PlayFlags::default()
    }
}

fn flags_key(game_id: u64) -> String {
    game_id.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_for_falls_back_to_idle_slot() {
        let mut state = TuiState::default();
        state.play_flags.verbosity = 2;
        state.store_flags_for(consts::SETTINGS_GAME_IDLE);
        state.play_flags.verbosity = 1;
        state.store_flags_for(11);
        assert_eq!(state.flags_for(11).verbosity, 1);
        assert_eq!(state.flags_for(22).verbosity, 2);
        assert_eq!(state.flags_for(consts::SETTINGS_GAME_IDLE).verbosity, 2);
    }

    #[test]
    fn load_keeps_legacy_play_flags() {
        let json = r#"{"play_flags":{"verbosity":2,"disable_audio":false,"udp":false}}"#;
        let state: TuiState = serde_json::from_str(json).unwrap();
        assert_eq!(state.play_flags.verbosity, 2);
        assert!(state.play_flags_by_game.is_empty());
        assert_eq!(state.settings_game_id, consts::SETTINGS_GAME_IDLE);
    }
}
