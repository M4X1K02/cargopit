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
    pub simapi_nonzero: bool,
    pub connected: usize,
    pub missing: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SlowDiagnostics {
    pub groups: String,
    pub in_input: bool,
    pub in_dialout: bool,
    pub in_uucp: bool,
    pub udev_present: bool,
    pub cargopit_bin: Option<String>,
    pub simd_bin: Option<String>,
    pub simapi_exists: bool,
    pub simapi_nonzero: bool,
}

impl SlowDiagnostics {
    pub fn live() -> Self {
        let groups = user_groups();
        Self {
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
            simapi_exists: process::simapi_present(),
            simapi_nonzero: process::simapi_nonzero(),
        }
    }
}

pub fn collect(discovery: &Discovery, devices: &[(DeviceClass, String, String)]) -> Diagnostics {
    assemble(&SlowDiagnostics::live(), discovery, devices)
}

pub fn assemble(
    slow: &SlowDiagnostics,
    discovery: &Discovery,
    devices: &[(DeviceClass, String, String)],
) -> Diagnostics {
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
        groups: slow.groups.clone(),
        in_input: slow.in_input,
        in_dialout: slow.in_dialout,
        in_uucp: slow.in_uucp,
        udev_present: slow.udev_present,
        cargopit_bin: slow.cargopit_bin.clone(),
        simd_bin: slow.simd_bin.clone(),
        simapi_exists: slow.simapi_exists,
        simapi_nonzero: slow.simapi_nonzero,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::HardwareChoice;

    #[test]
    fn assemble_counts_presence_from_discovery() {
        let slow = SlowDiagnostics::default();
        let discovery = Discovery {
            pulse_sinks: vec![HardwareChoice {
                value: "sink0".into(),
                label: "Sink".into(),
            }],
            ..Discovery::default()
        };
        let devices = vec![
            (DeviceClass::Sound, "sink0".into(), String::new()),
            (DeviceClass::Sound, "missing-sink".into(), String::new()),
        ];
        let diag = assemble(&slow, &discovery, &devices);
        assert_eq!(diag.connected, 1);
        assert_eq!(diag.missing, 1);
    }
}
