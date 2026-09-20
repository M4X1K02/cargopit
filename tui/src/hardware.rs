use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::consts;
use crate::schema::DeviceClass;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareChoice {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Default)]
pub struct Discovery {
    pub serial_ports: Vec<HardwareChoice>,
    pub hid_devices: Vec<HardwareChoice>,
    pub rumble_paths: Vec<HardwareChoice>,
    pub pulse_sinks: Vec<HardwareChoice>,
}

impl Discovery {
    pub fn live() -> Self {
        Self {
            serial_ports: list_serial_ports(),
            hid_devices: list_hid_devices(),
            rumble_paths: list_rumble_paths(),
            pulse_sinks: list_pulse_sinks(),
        }
    }

    pub fn choices_for(&self, class: DeviceClass, want_path: bool) -> &[HardwareChoice] {
        match class {
            DeviceClass::Serial => &self.serial_ports,
            DeviceClass::Sound => &self.pulse_sinks,
            DeviceClass::Usb if want_path => &self.rumble_paths,
            DeviceClass::Usb => &self.hid_devices,
        }
    }

    pub fn presence(&self, class: DeviceClass, devid: &str, devpath: &str) -> &'static str {
        match class {
            DeviceClass::Serial => presence_in(&self.serial_ports, devpath),
            DeviceClass::Sound => presence_in(&self.pulse_sinks, devid),
            DeviceClass::Usb => {
                let hid = if devid.is_empty() {
                    consts::PRESENCE_UNKNOWN
                } else {
                    presence_in(&self.hid_devices, devid)
                };
                if !devpath.is_empty() {
                    return presence_in(&self.rumble_paths, devpath);
                }
                hid
            }
        }
    }
}

fn presence_in(choices: &[HardwareChoice], value: &str) -> &'static str {
    if value.is_empty() {
        return consts::PRESENCE_UNKNOWN;
    }
    if Path::new(value).exists() {
        return consts::PRESENCE_CONNECTED;
    }
    if choices.iter().any(|choice| choice.value == value) {
        consts::PRESENCE_CONNECTED
    } else {
        consts::PRESENCE_MISSING
    }
}

fn list_serial_ports() -> Vec<HardwareChoice> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(consts::DEV_DIR) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let matches_prefix = consts::SERIAL_DEV_PREFIXES
                .iter()
                .any(|prefix| name.starts_with(prefix));
            let matches_udev = name.starts_with("simdev");
            if !matches_prefix && !matches_udev {
                continue;
            }
            let path = entry.path();
            out.push(HardwareChoice {
                label: path.display().to_string(),
                value: path.to_string_lossy().into_owned(),
            });
        }
    }
    out.sort_by(|a, b| a.value.cmp(&b.value));
    out
}

fn list_hid_devices() -> Vec<HardwareChoice> {
    let mut out = Vec::new();
    collect_hidraw(&mut out);
    collect_usb_ids(&mut out);
    out.sort_by(|a, b| a.value.cmp(&b.value));
    out.dedup_by(|a, b| a.value == b.value);
    out
}

fn collect_hidraw(out: &mut Vec<HardwareChoice>) {
    let Ok(entries) = fs::read_dir(consts::SYSFS_HIDRAW) else {
        return;
    };
    for entry in entries.flatten() {
        let uevent = entry.path().join("device").join("uevent");
        let Ok(text) = fs::read_to_string(uevent) else {
            continue;
        };
        if let Some(id) = hid_id_from_uevent(&text) {
            let node = format!("/dev/{}", entry.file_name().to_string_lossy());
            out.push(HardwareChoice {
                label: format!("{id} ({node})"),
                value: id,
            });
        }
    }
}

fn hid_id_from_uevent(text: &str) -> Option<String> {
    for line in text.lines() {
        let Some(value) = line.strip_prefix("HID_ID=") else {
            continue;
        };
        let parts: Vec<&str> = value.split(':').collect();
        if parts.len() < 3 {
            continue;
        }
        let vid = parts[1].trim_start_matches('0');
        let pid = parts[2].trim_start_matches('0');
        let vid = if vid.is_empty() { "0" } else { vid };
        let pid = if pid.is_empty() { "0" } else { pid };
        return Some(format!("{vid}:{pid}").to_ascii_uppercase());
    }
    None
}

fn collect_usb_ids(out: &mut Vec<HardwareChoice>) {
    let Ok(entries) = fs::read_dir(consts::SYSFS_HID_DEVICES) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let vid_path = path.join("idVendor");
        let pid_path = path.join("idProduct");
        let Ok(vid) = fs::read_to_string(vid_path) else {
            continue;
        };
        let Ok(pid) = fs::read_to_string(pid_path) else {
            continue;
        };
        let id = format!("{}:{}", vid.trim(), pid.trim()).to_ascii_uppercase();
        let product = fs::read_to_string(path.join("product"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let label = if product.is_empty() {
            id.clone()
        } else {
            format!("{id} ({product})")
        };
        out.push(HardwareChoice { value: id, label });
    }
}

fn list_rumble_paths() -> Vec<HardwareChoice> {
    glob_paths(consts::CSL_RUMBLE_GLOB)
        .into_iter()
        .map(|path| HardwareChoice {
            label: path.display().to_string(),
            value: path.to_string_lossy().into_owned(),
        })
        .collect()
}

fn glob_paths(pattern: &str) -> Vec<PathBuf> {
    let Some((dir, file_pat)) = pattern.rsplit_once('/') else {
        return Vec::new();
    };
    expand_glob_dir(Path::new(dir), file_pat)
}

fn expand_glob_dir(dir_pattern: &Path, file_pat: &str) -> Vec<PathBuf> {
    let dir_str = dir_pattern.to_string_lossy();
    if !dir_str.contains('*') {
        return match_dir(dir_pattern, file_pat);
    }
    let mut out = Vec::new();
    let parts: Vec<&str> = dir_str.split('/').filter(|p| !p.is_empty()).collect();
    expand_parts(PathBuf::from("/"), &parts, 0, file_pat, &mut out);
    out
}

fn expand_parts(
    current: PathBuf,
    parts: &[&str],
    index: usize,
    file_pat: &str,
    out: &mut Vec<PathBuf>,
) {
    if index == parts.len() {
        out.extend(match_dir(&current, file_pat));
        return;
    }
    let part = parts[index];
    if !part.contains('*') {
        expand_parts(current.join(part), parts, index + 1, file_pat, out);
        return;
    }
    let Ok(entries) = fs::read_dir(&current) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if glob_match(part, &name) {
            expand_parts(entry.path(), parts, index + 1, file_pat, out);
        }
    }
}

fn match_dir(dir: &Path, file_pat: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if glob_match(file_pat, &name) {
            out.push(entry.path());
        }
    }
    out
}

fn glob_match(pattern: &str, name: &str) -> bool {
    if let Some((head, tail)) = pattern.split_once('*') {
        return name.starts_with(head)
            && name.ends_with(tail)
            && name.len() >= head.len() + tail.len();
    }
    pattern == name
}

fn list_pulse_sinks() -> Vec<HardwareChoice> {
    let output = Command::new(consts::PACTL_BIN)
        .args(consts::PACTL_LIST_SINKS)
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_pactl_sinks(&String::from_utf8_lossy(&output.stdout))
}

pub fn parse_pactl_sinks(text: &str) -> Vec<HardwareChoice> {
    let mut out = Vec::new();
    let mut name = String::new();
    let mut description = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Sink #") {
            push_sink(&mut out, &name, &description);
            name.clear();
            description.clear();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("Name:") {
            name = value.trim().to_string();
        }
        if let Some(value) = trimmed.strip_prefix("Description:") {
            description = value.trim().to_string();
        }
    }
    push_sink(&mut out, &name, &description);
    out
}

fn push_sink(out: &mut Vec<HardwareChoice>, name: &str, description: &str) {
    if name.is_empty() {
        return;
    }
    let label = if description.is_empty() {
        name.to_string()
    } else {
        format!("{description} ({name})")
    };
    out.push(HardwareChoice {
        value: name.to_string(),
        label,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pactl_descriptions_are_preferred() {
        let text = "\
Sink #0
\tName: alsa_output.pci.analog
\tDescription: Built-in Audio
Sink #1
\tName: virtual.sink
\tDescription: Shaker
";
        let sinks = parse_pactl_sinks(text);
        assert_eq!(sinks.len(), 2);
        assert_eq!(sinks[0].value, "alsa_output.pci.analog");
        assert!(sinks[0].label.contains("Built-in Audio"));
    }

    #[test]
    fn usb_choices_are_not_serial_ports() {
        let discovery = Discovery {
            serial_ports: vec![HardwareChoice {
                value: "/dev/ttyUSB0".into(),
                label: "/dev/ttyUSB0".into(),
            }],
            hid_devices: vec![HardwareChoice {
                value: "0EB7:183B".into(),
                label: "Fanatec".into(),
            }],
            rumble_paths: Vec::new(),
            pulse_sinks: Vec::new(),
        };
        let usb = discovery.choices_for(DeviceClass::Usb, false);
        assert_eq!(usb[0].value, "0EB7:183B");
        let serial = discovery.choices_for(DeviceClass::Serial, false);
        assert_eq!(serial[0].value, "/dev/ttyUSB0");
    }
}
