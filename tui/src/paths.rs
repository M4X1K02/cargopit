use crate::consts::*;
use std::env;
use std::path::PathBuf;

pub fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn xdg_dir(var: &str, fallback: &str) -> PathBuf {
    env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(fallback))
}

pub fn config_path() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", ".config")
        .join(PROGRAM_NAME)
        .join(CONFIG_FILE_NAME)
}

pub fn log_dir() -> PathBuf {
    xdg_dir("XDG_CACHE_HOME", ".cache").join(PROGRAM_NAME)
}

pub fn data_dir() -> PathBuf {
    env::var_os("MONOCOQUE_INSTALL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| xdg_dir("XDG_DATA_HOME", ".local/share").join(PROGRAM_NAME))
}

pub fn local_bin_dir() -> PathBuf {
    home_dir().join(".local/bin")
}
