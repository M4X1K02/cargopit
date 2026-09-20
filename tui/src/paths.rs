use std::env;
use std::path::{Path, PathBuf};

use crate::consts;

pub fn home_dir() -> PathBuf {
    env::var_os(consts::ENV_HOME)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn xdg_dir(var: &str, fallback: &str) -> PathBuf {
    if let Some(value) = env::var_os(var) {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    home_dir().join(fallback)
}

pub fn config_home() -> PathBuf {
    xdg_dir(consts::ENV_XDG_CONFIG_HOME, ".config")
}

pub fn cache_home() -> PathBuf {
    xdg_dir(consts::ENV_XDG_CACHE_HOME, ".cache")
}

pub fn data_home() -> PathBuf {
    xdg_dir(consts::ENV_XDG_DATA_HOME, ".local/share")
}

pub fn state_home() -> PathBuf {
    xdg_dir(consts::ENV_XDG_STATE_HOME, ".local/state")
}

pub fn default_config_path() -> PathBuf {
    config_home()
        .join(consts::CONFIG_DIR_NAME)
        .join(consts::CONFIG_FILE_NAME)
}

pub fn simd_config_path() -> PathBuf {
    config_home()
        .join(consts::SIMD_CONFIG_DIR_NAME)
        .join(consts::SIMD_CONFIG_FILE_NAME)
}

pub fn diameters_path() -> PathBuf {
    config_home()
        .join(consts::CONFIG_DIR_NAME)
        .join(consts::DIAMETERS_FILE_NAME)
}

pub fn tui_state_path() -> PathBuf {
    state_home()
        .join(consts::CONFIG_DIR_NAME)
        .join(consts::TUI_STATE_FILE_NAME)
}

pub fn log_dir() -> PathBuf {
    cache_home().join(consts::CACHE_DIR_NAME)
}

pub fn source_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    if path == "~" {
        return home_dir();
    }
    PathBuf::from(path)
}

pub fn bundled_conf_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(source_root().join(consts::CONF_DIRNAME));
    dirs.push(
        data_home()
            .join(consts::CONFIG_DIR_NAME)
            .join(consts::CONF_DIRNAME),
    );
    dirs.push(
        PathBuf::from("/usr")
            .join(consts::SHARE_DIRNAME)
            .join(consts::CONFIG_DIR_NAME)
            .join(consts::CONF_DIRNAME),
    );
    dirs
}

pub fn find_bundled_file(name: &str) -> Option<PathBuf> {
    for dir in bundled_conf_dirs() {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn local_bin_dir() -> PathBuf {
    home_dir().join(consts::LOCAL_BIN_DIRNAME)
}

pub fn prepend_search_path() {
    let extra = [
        source_root().join(consts::BUILD_DIRNAME),
        local_bin_dir(),
    ];
    let current = env::var(consts::ENV_PATH).unwrap_or_default();
    let mut parts: Vec<String> = extra
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if !current.is_empty() {
        parts.push(current);
    }
    env::set_var(consts::ENV_PATH, parts.join(":"));
}
