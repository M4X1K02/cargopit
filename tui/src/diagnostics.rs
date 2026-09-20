use std::path::Path;
use std::process::Command;

use crate::consts;
use crate::hardware::Discovery;
use crate::paths;
use crate::process;
use crate::schema::DeviceClass;

#[derive(Debug, Clone)]
pub struct Diagnostics {
    pub groups: String,
    pub in_input: bool,
    pub in_dialout: bool,
    pub in_uucp: bool,
    pub udev_present: bool,
    pub cargopit_bin: Option<String>,
    pub simd_bin: Option<String>,
    pub simapi_exists: bool,
    pub simapi_live: bool,
    pub connected: usize,
    pub missing: usize,
}

pub fn collect(discovery: &Discovery, devices: &[(DeviceClass, String, String)]) -> Diagnostics {
    let groups = user_groups();
    let connected = devices
        .iter()
        .filter(|(class, devid, devpath)| {
            discovery.presence(*class, devid, devpath) == consts::PRESENCE_CONNECTED
        })
        .count();
    let missing = devices
        .iter()
        .filter(|(class, devid, devpath)| {
            discovery.presence(*class, devid, devpath) == consts::PRESENCE_MISSING
        })
        .count();
    Diagnostics {
        in_input: groups.split_whitespace().any(|g| g == consts::UDEV_GROUP_INPUT),
        in_dialout: groups
            .split_whitespace()
            .any(|g| g == consts::UDEV_GROUP_DIALOUT),
        in_uucp: groups.split_whitespace().any(|g| g == consts::UDEV_GROUP_UUCP),
        groups,
        udev_present: Path::new(consts::UDEV_RULES_PATH).exists(),
        cargopit_bin: process::find_binary(consts::BINARY_CARGOPIT)
            .map(|p| p.display().to_string()),
        simd_bin: process::find_binary(consts::BINARY_SIMD).map(|p| p.display().to_string()),
        simapi_exists: false,
        simapi_live: false,
        connected,
        missing,
    }
}

fn user_groups() -> String {
    Command::new(consts::ID_BIN)
        .arg(consts::ID_GROUPS)
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub fn lua_scripts() -> Vec<String> {
    let mut names = Vec::new();
    let user_dir = paths::config_home().join(consts::CONFIG_DIR_NAME);
    push_lua_from_dir(&user_dir, &mut names);
    for dir in paths::bundled_conf_dirs() {
        push_lua_from_dir(&dir, &mut names);
    }
    names.sort();
    names.dedup();
    names
}

fn push_lua_from_dir(dir: &Path, names: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("lua") {
            names.push(path.display().to_string());
        }
    }
}

pub fn copy_lua_template(name: &str) -> anyhow::Result<std::path::PathBuf> {
    let src = paths::find_bundled_file(name)
        .ok_or_else(|| anyhow::anyhow!("bundled Lua {name} not found"))?;
    let dest_dir = paths::config_home().join(consts::CONFIG_DIR_NAME);
    std::fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(name);
    std::fs::copy(&src, &dest)?;
    Ok(dest)
}
