use crate::consts::*;
use std::fs;
use std::process::Command;

pub fn list_serial_ports() -> Vec<String> {
    let mut ports = Vec::new();
    let Ok(entries) = fs::read_dir(SERIAL_DEV_DIR) else {
        return ports;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if SERIAL_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            ports.push(format!("{SERIAL_DEV_DIR}/{name}"));
        }
    }
    ports.sort();
    ports
}

pub fn list_pulse_sinks() -> Vec<String> {
    let output = Command::new(PACTL_BIN).args(PACTL_LIST_ARGS).output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1).map(str::to_string))
        .collect()
}

pub fn device_choices_for_class(class: &str) -> Vec<String> {
    if class.eq_ignore_ascii_case(CLASS_SERIAL) || class.eq_ignore_ascii_case(CLASS_USB) {
        list_serial_ports()
    } else if class.eq_ignore_ascii_case(CLASS_SOUND) {
        list_pulse_sinks()
    } else {
        Vec::new()
    }
}
