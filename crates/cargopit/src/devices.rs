//! Configured devices for one play session. A USB tachometer opens the RevBurner HID device.

use cargopit_config::config::{CargopitConfig, DeviceEntry};
use cargopit_config::keys::{self, CLASS_SERIAL, CLASS_SOUND, CLASS_USB};
use cargopit_config::names;
use cargopit_devices::transport::RealHid;
use cargopit_devices::{tick_interval_ms, DeviceKind, SimDevice, DEFAULT_DEVICE_FPS};

use crate::games;
use crate::log::Level;
use crate::scheduler;
use crate::tyres;

pub const REVBURNER_VENDOR_ID: u16 = 0x04d8;
pub const REVBURNER_PRODUCT_ID: u16 = 0x0102;

const CONFIG_INDEX_FIRST: i32 = 0;

pub struct LoadedDevices {
    devices: Vec<SimDevice>,
    updates: Vec<u64>,
    effects: Vec<Option<i32>>,
    live_hids: Vec<RealHid>,
}

pub struct InitNotice {
    pub level: Level,
    pub message: String,
}

pub struct ProfileLoad {
    pub devices: LoadedDevices,
    pub notices: Vec<InitNotice>,
}

impl LoadedDevices {
    pub fn from_config(config: &CargopitConfig, requested_index: i32, disable_audio: bool) -> Self {
        let Some(index) = profile_index(config.profiles.len(), requested_index) else {
            return Self::empty();
        };
        open_profile_at(config, index, disable_audio).devices
    }

    pub fn empty() -> Self {
        Self {
            devices: Vec::new(),
            updates: Vec::new(),
            effects: Vec::new(),
            live_hids: Vec::new(),
        }
    }

    pub fn hid_handles(&self) -> usize {
        self.live_hids.len()
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

    pub fn effect(&self, index: usize) -> Option<i32> {
        self.effects.get(index).copied().flatten()
    }

    pub fn needs_tyre_diameter(&self) -> bool {
        self.effects
            .iter()
            .flatten()
            .any(|effect| tyres::device_needs_tyre_diameter(*effect))
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

struct PreparedDevice {
    device: SimDevice,
    effect: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceSkip {
    Disabled,
    AudioDisabled,
}

pub fn device_skip(entry: &DeviceEntry, disable_audio: bool) -> Option<DeviceSkip> {
    if entry.get_bool(keys::KEY_ENABLED) == Some(false) {
        return Some(DeviceSkip::Disabled);
    }
    if disable_audio && entry_kind(entry) == DeviceKind::Sound {
        return Some(DeviceSkip::AudioDisabled);
    }
    None
}

pub fn open_profile_at(config: &CargopitConfig, index: usize, disable_audio: bool) -> ProfileLoad {
    open_profile_with(config, index, disable_audio, &mut HidAttempt::Live)
}

pub fn open_profile_probed<F>(
    config: &CargopitConfig,
    index: usize,
    disable_audio: bool,
    mut probe: F,
) -> ProfileLoad
where
    F: FnMut(u16, u16) -> bool,
{
    open_profile_with(
        config,
        index,
        disable_audio,
        &mut HidAttempt::Probe(&mut probe),
    )
}

fn open_profile_with(
    config: &CargopitConfig,
    index: usize,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> ProfileLoad {
    let Some(profile) = config.profiles.get(index) else {
        return ProfileLoad {
            devices: LoadedDevices::empty(),
            notices: Vec::new(),
        };
    };
    let mut devices = Vec::new();
    let mut effects = Vec::new();
    let mut live_hids = Vec::new();
    let mut notices = Vec::new();
    let mut initialized = 0i32;
    for (slot, entry) in profile.devices.iter().enumerate() {
        let slot = i32::try_from(slot).unwrap_or(i32::MAX);
        let considered = consider_entry(entry, slot, devices.len() as i32, disable_audio, attempt);
        notices.extend(considered.notices);
        let Some(prepared) = considered.prepared else {
            continue;
        };
        if let Some(hid) = considered.hid {
            live_hids.push(hid);
        }
        devices.push(prepared.device);
        effects.push(prepared.effect);
        initialized = initialized.saturating_add(1);
    }
    notices.push(InitNotice {
        level: Level::Info,
        message: games::initialized_devices_message(initialized),
    });
    let updates = vec![0; devices.len()];
    ProfileLoad {
        devices: LoadedDevices {
            devices,
            updates,
            effects,
            live_hids,
        },
        notices,
    }
}

enum HidAttempt<'a> {
    Live,
    Probe(&'a mut dyn FnMut(u16, u16) -> bool),
}

enum OpenedHid {
    Missing,
    Live(RealHid),
    Simulated,
}

struct Considered {
    notices: Vec<InitNotice>,
    prepared: Option<PreparedDevice>,
    hid: Option<RealHid>,
}

fn consider_entry(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return Considered {
            notices: vec![InitNotice {
                level: Level::Info,
                message: skip_message(skip, slot),
            }],
            prepared: None,
            hid: None,
        };
    }
    if usb_uses_revburner(entry) {
        return revburner_attempt(entry, id, attempt);
    }
    Considered {
        notices: vec![InitNotice {
            level: Level::Warn,
            message: games::could_not_initialize_message(class_label(entry_kind(entry))),
        }],
        prepared: None,
        hid: None,
    }
}

fn revburner_attempt(entry: &DeviceEntry, id: i32, attempt: &mut HidAttempt<'_>) -> Considered {
    let mut notices = vec![
        InitNotice {
            level: Level::Info,
            message: games::MSG_INIT_USB.to_string(),
        },
        InitNotice {
            level: Level::Info,
            message: games::MSG_INIT_TACH.to_string(),
        },
        InitNotice {
            level: Level::Info,
            message: games::MSG_INIT_REVBURNER.to_string(),
        },
    ];
    match open_revburner(attempt) {
        OpenedHid::Missing => {
            notices.push(InitNotice {
                level: Level::Error,
                message: games::MSG_REVBURNER_MISSING.to_string(),
            });
            notices.push(InitNotice {
                level: Level::Warn,
                message: games::usb_init_error_message(games::ERROR_UNKNOWN),
            });
            notices.push(InitNotice {
                level: Level::Warn,
                message: games::could_not_initialize_message(CLASS_USB),
            });
            Considered {
                notices,
                prepared: None,
                hid: None,
            }
        }
        OpenedHid::Live(hid) => Considered {
            notices,
            prepared: Some(build_device(entry, id)),
            hid: Some(hid),
        },
        OpenedHid::Simulated => Considered {
            notices,
            prepared: Some(build_device(entry, id)),
            hid: None,
        },
    }
}

fn open_revburner(attempt: &mut HidAttempt<'_>) -> OpenedHid {
    match attempt {
        HidAttempt::Live => match RealHid::open(REVBURNER_VENDOR_ID, REVBURNER_PRODUCT_ID) {
            Ok(hid) => OpenedHid::Live(hid),
            Err(_) => OpenedHid::Missing,
        },
        HidAttempt::Probe(probe) => {
            if probe(REVBURNER_VENDOR_ID, REVBURNER_PRODUCT_ID) {
                return OpenedHid::Simulated;
            }
            OpenedHid::Missing
        }
    }
}

fn usb_uses_revburner(entry: &DeviceEntry) -> bool {
    if entry_kind(entry) != DeviceKind::Usb {
        return false;
    }
    let subtype = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    !matches!(
        names::lookup(names::USB_TYPES, subtype),
        Some(names::SUBTYPE_USB_WHEEL | names::SUBTYPE_USB_HAPTIC)
    )
}

fn class_label(kind: DeviceKind) -> &'static str {
    match kind {
        DeviceKind::Usb => CLASS_USB,
        DeviceKind::Sound => CLASS_SOUND,
        DeviceKind::Serial => CLASS_SERIAL,
        DeviceKind::Unknown => games::DEVICE_NAME_MISSING,
    }
}

fn skip_message(skip: DeviceSkip, index: i32) -> String {
    match skip {
        DeviceSkip::Disabled => games::skipping_disabled_message(index),
        DeviceSkip::AudioDisabled => games::MSG_SKIP_AUDIO.to_string(),
    }
}

fn build_device(entry: &DeviceEntry, id: i32) -> PreparedDevice {
    let kind = entry_kind(entry);
    let fps = scheduler::clamp_fps(entry.get_i64(keys::KEY_FPS).unwrap_or(0) as i32);
    let mut device = SimDevice::new(id, fps, kind);
    device.set_initialized(true);
    device.set_config_file(entry.get_str(keys::KEY_CONFIG).map(str::to_string));
    PreparedDevice {
        device,
        effect: device_effect(entry),
    }
}

pub fn device_effect(entry: &DeviceEntry) -> Option<i32> {
    let name = entry.get_str(keys::KEY_EFFECT)?;
    names::lookup(names::EFFECTS, name)
}

pub fn entry_kind(entry: &DeviceEntry) -> DeviceKind {
    let class = entry
        .get_str(keys::KEY_DEVICE)
        .or_else(|| entry.get_str(keys::KEY_TYPE))
        .unwrap_or("");
    device_kind(class)
}

pub fn device_kind(name: &str) -> DeviceKind {
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
    use crate::games;
    use cargopit_config::config::{DeviceEntry, SimProfile};
    use cargopit_config::keys::CLASS_SERIAL;

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
        let loaded = open_profile_probed(&config, 0, true, |_vendor, _product| true);
        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(loaded.devices.device(0).unwrap().kind(), DeviceKind::Usb);
        assert_eq!(loaded.devices.device(0).unwrap().interval_ms(), 16);
        assert_eq!(loaded.devices.hid_handles(), 0);
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::initialized_devices_message(1)));
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::could_not_initialize_message(CLASS_SERIAL)));
        let missing = open_profile_probed(&config, 0, true, |_vendor, _product| false);
        assert!(missing.devices.is_empty());
        assert!(missing
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_REVBURNER_MISSING));
        assert!(missing
            .notices
            .iter()
            .any(|notice| { notice.message == games::initialized_devices_message(0) }));
        assert!(profile_index(1, 4).is_none());
        let disabled = entry("sound", 60, false);
        assert_eq!(device_skip(&disabled, true), Some(DeviceSkip::Disabled));
        let muted = entry("sound", 60, true);
        assert_eq!(device_skip(&muted, true), Some(DeviceSkip::AudioDisabled));
        assert_eq!(device_skip(&muted, false), None);
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
        let mut loaded = open_profile_probed(&config, 0, false, |_vendor, _product| true).devices;
        loaded.tick(0);
        assert_eq!(loaded.updates(0), 1);
    }

    #[test]
    fn slip_alias_marks_the_profile_for_tyre_diameter() {
        let mut slip = entry("usb", 60, true);
        slip.set_str(keys::KEY_EFFECT, "Slip");
        let mut engine = entry("serial", 60, true);
        engine.set_str(keys::KEY_EFFECT, "Engine");
        let config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![slip, engine],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let loaded = open_profile_probed(&config, 0, false, |_vendor, _product| true).devices;
        assert_eq!(loaded.effect(0), Some(names::EFFECT_TYRE_SLIP));
        assert!(loaded.needs_tyre_diameter());
    }
}
