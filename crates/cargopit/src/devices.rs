//! Configured devices for one play session. Hardware open stays in the C host.

use cargopit_config::config::{CargopitConfig, DeviceEntry};
use cargopit_config::keys::{self, CLASS_SERIAL, CLASS_SOUND, CLASS_USB};
use cargopit_devices::{tick_interval_ms, DeviceKind, SimDevice, DEFAULT_DEVICE_FPS};

use crate::scheduler;

const CONFIG_INDEX_FIRST: i32 = 0;

pub struct LoadedDevices {
    devices: Vec<SimDevice>,
    updates: Vec<u64>,
}

impl LoadedDevices {
    pub fn from_config(config: &CargopitConfig, requested_index: i32, disable_audio: bool) -> Self {
        let Some(index) = profile_index(config.profiles.len(), requested_index) else {
            return Self::empty();
        };
        let mut devices = Vec::new();
        for entry in &config.profiles[index].devices {
            if let Some(device) = device_from_entry(entry, devices.len() as i32, disable_audio) {
                devices.push(device);
            }
        }
        let updates = vec![0; devices.len()];
        Self { devices, updates }
    }

    pub fn empty() -> Self {
        Self {
            devices: Vec::new(),
            updates: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.devices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn device(&self, index: usize) -> Option<&SimDevice> {
        self.devices.get(index)
    }

    pub fn updates(&self, index: usize) -> u64 {
        self.updates.get(index).copied().unwrap_or(0)
    }

    pub fn tick(&mut self, index: usize) {
        if let Some(count) = self.updates.get_mut(index) {
            *count = count.wrapping_add(1);
        }
    }

    pub fn interval_ms(&self, index: usize) -> u64 {
        self.devices
            .get(index)
            .map(SimDevice::interval_ms)
            .unwrap_or(tick_interval_ms(DEFAULT_DEVICE_FPS))
    }
}

pub fn profile_index(profile_count: usize, requested: i32) -> Option<usize> {
    if profile_count == 0 {
        return None;
    }
    if requested < CONFIG_INDEX_FIRST {
        return Some(0);
    }
    if requested as usize >= profile_count {
        return None;
    }
    Some(requested as usize)
}

fn device_from_entry(entry: &DeviceEntry, id: i32, disable_audio: bool) -> Option<SimDevice> {
    if entry.get_bool(keys::KEY_ENABLED) == Some(false) {
        return None;
    }
    let kind = kind_from_type(entry.get_str(keys::KEY_TYPE).unwrap_or(""));
    if disable_audio && kind == DeviceKind::Sound {
        return None;
    }
    let fps = scheduler::clamp_fps(entry.get_i64(keys::KEY_FPS).unwrap_or(0) as i32);
    let mut device = SimDevice::new(id, fps, kind);
    device.set_initialized(true);
    device.set_config_file(entry.get_str(keys::KEY_CONFIG).map(str::to_string));
    Some(device)
}

fn kind_from_type(name: &str) -> DeviceKind {
    if name.eq_ignore_ascii_case(CLASS_USB) {
        return DeviceKind::Usb;
    }
    if name.eq_ignore_ascii_case(CLASS_SERIAL) {
        return DeviceKind::Serial;
    }
    if name.eq_ignore_ascii_case(CLASS_SOUND) {
        return DeviceKind::Sound;
    }
    DeviceKind::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargopit_config::config::{DeviceEntry, SimProfile};

    fn entry(kind: &str, fps: i64, enabled: bool) -> DeviceEntry {
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_TYPE, kind);
        device.set_int(keys::KEY_FPS, fps);
        device.set_bool(keys::KEY_ENABLED, enabled);
        device
    }

    #[test]
    fn profile_load_skips_disabled_and_muted_sound() {
        let config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![
                    entry("usb", 60, true),
                    entry("serial", 0, true),
                    entry("sound", 60, true),
                    entry("wheel", 144, false),
                ],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let loaded = LoadedDevices::from_config(&config, -1, true);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.device(0).unwrap().kind(), DeviceKind::Usb);
        assert_eq!(loaded.device(0).unwrap().interval_ms(), 16);
        assert_eq!(loaded.device(1).unwrap().kind(), DeviceKind::Serial);
        assert_eq!(loaded.device(1).unwrap().fps(), 1);
        assert!(profile_index(1, 4).is_none());
        loaded_tick_counts();
    }

    fn loaded_tick_counts() {
        let config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![entry("USB", 60, true)],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let mut loaded = LoadedDevices::from_config(&config, 0, false);
        loaded.tick(0);
        assert_eq!(loaded.updates(0), 1);
    }
}
