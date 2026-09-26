//! Sound-device init logs, Pulse node names, and the playback voice.

use std::sync::{Arc, Mutex};

use cargopit_config::config::DeviceEntry;
use cargopit_config::keys;
use cargopit_config::names;

use cargopit_devices::haptic::{HapticSettings, TyreId, VibrationEffect};
use cargopit_devices::sound::{ShakerVoice, SharedShaker};
use cargopit_devices::transport::ShakerRequest;

use crate::games;
use crate::log::Level;

const CHANNEL_MIN: i64 = 1;
const CHANNEL_MAX: i64 = 8;
const VOLUME_MIN: i64 = 0;
const VOLUME_UNITY: i64 = 100;
pub(crate) const SOUND_PAN_ALL: i64 = -1;
pub(crate) const SOUND_AMPLITUDE_UNITY: i64 = 100;
pub(crate) const SOUND_MOTOR_DEFAULT: i64 = 1;
const FREQUENCY_DEFAULT: i64 = 0;
const FREQUENCY_MAX_DEFAULT: i64 = 0;
const THRESHOLD_DEFAULT: f64 = 0.0;
const NOISE_DEFAULT: i64 = 0;
const GEAR_DURATION_DEFAULT_S: f64 = 0.125;
const DURATION_UNSET: f64 = 0.0;
const NODE_EFFECT_FALLBACK: &str = "Engine";
const NODE_PREFIX: &str = "cargopit";
const NODE_SEPARATOR: &str = ".";
const NODE_NAME_MAX: usize = 64;

pub struct SoundOpen {
    pub notices: Vec<(Level, String)>,
    pub ready: bool,
    pub request: Option<ShakerRequest>,
    pub voice: Option<SharedShaker>,
}

struct SoundSettings {
    effect: i32,
    tyre: i32,
    path: String,
    duration: f64,
    frequency: i64,
    frequency_max: i64,
    threshold: f64,
    amplitude: i64,
    motor: i64,
    volume: i64,
    mask: u32,
    channels: i64,
    noise: i64,
}

pub fn open_sound(
    entry: &DeviceEntry,
    effect: i32,
    path: &str,
    supports_haptics: bool,
) -> SoundOpen {
    if effect_uses_tyre(effect) && !supports_haptics {
        return SoundOpen {
            notices: vec![(Level::Warn, games::MSG_SOUND_SKIP_HAPTICS.to_string())],
            ready: false,
            request: None,
            voice: None,
        };
    }
    let settings = SoundSettings::read(entry, effect, path);
    let mut notices = settings.haptic_notices();
    let request = append_stream(&mut notices, &settings);
    let voice = request.as_ref().and_then(|_| settings.shared_voice());
    SoundOpen {
        ready: request.is_some(),
        request,
        voice,
        notices,
    }
}

pub fn stream_node_name(effect: i32, tyre: i32) -> Option<String> {
    let effect_name = names::name_for(names::EFFECTS, effect)
        .or_else(|| names::name_for(names::EFFECTS, names::EFFECT_ENGINE))?;
    let name = match tyre_label(effect, tyre) {
        Some(tyre_name) => {
            format!("{NODE_PREFIX}{NODE_SEPARATOR}{effect_name}{NODE_SEPARATOR}{tyre_name}")
        }
        None => format!("{NODE_PREFIX}{NODE_SEPARATOR}{effect_name}"),
    };
    if name.len() >= NODE_NAME_MAX {
        return None;
    }
    Some(name)
}

fn append_stream(
    notices: &mut Vec<(Level, String)>,
    settings: &SoundSettings,
) -> Option<ShakerRequest> {
    notices.extend(settings.stream_notices());
    let Some(name) = stream_node_name(settings.effect, settings.tyre) else {
        notices.push((Level::Error, games::sound_describe_error(settings.effect)));
        return None;
    };
    notices.push((Level::Info, games::sound_node_message(&name)));
    Some(settings.playback(&name))
}

fn effect_uses_tyre(effect: i32) -> bool {
    matches!(
        effect,
        names::EFFECT_TYRE_SLIP
            | names::EFFECT_TYRE_LOCK
            | names::EFFECT_ABS
            | names::EFFECT_SUSPENSION
    )
}

fn effect_label(effect: i32) -> &'static str {
    names::name_for(names::EFFECTS, effect)
        .or_else(|| names::name_for(names::EFFECTS, names::EFFECT_ENGINE))
        .unwrap_or(NODE_EFFECT_FALLBACK)
}

fn tyre_label(effect: i32, tyre: i32) -> Option<&'static str> {
    if !effect_uses_tyre(effect) {
        return None;
    }
    names::name_for(names::TYRES, tyre)
}

fn vibration_phrase(effect: i32) -> Option<&'static str> {
    match effect {
        names::EFFECT_ENGINE => Some(games::VIBRATION_ENGINE),
        names::EFFECT_GEAR => Some(games::VIBRATION_GEAR),
        names::EFFECT_TYRE_SLIP => Some(games::VIBRATION_SLIP),
        names::EFFECT_TYRE_LOCK => Some(games::VIBRATION_LOCK),
        names::EFFECT_ABS => Some(games::VIBRATION_ABS),
        names::EFFECT_SUSPENSION => Some(games::VIBRATION_SUSPENSION),
        _ => None,
    }
}

impl SoundSettings {
    fn read(entry: &DeviceEntry, effect: i32, path: &str) -> Self {
        let channels = configured_channels(entry);
        Self {
            effect,
            tyre: configured_tyre(entry, effect),
            path: path.to_string(),
            duration: configured_duration(entry, effect),
            frequency: entry
                .get_i64(keys::KEY_FREQUENCY)
                .unwrap_or(FREQUENCY_DEFAULT),
            frequency_max: entry
                .get_i64(keys::KEY_FREQUENCY_MAX)
                .unwrap_or(FREQUENCY_MAX_DEFAULT),
            threshold: entry
                .get_f64(keys::KEY_THRESHOLD)
                .unwrap_or(THRESHOLD_DEFAULT),
            amplitude: SOUND_AMPLITUDE_UNITY,
            motor: entry
                .get_i64(keys::KEY_MOTORS)
                .unwrap_or(SOUND_MOTOR_DEFAULT),
            volume: configured_volume(entry),
            mask: channel_mask(entry, channels),
            channels,
            noise: entry.get_i64(keys::KEY_NOISE).unwrap_or(NOISE_DEFAULT),
        }
    }

    fn haptic_notices(&self) -> Vec<(Level, String)> {
        let mut notices = Vec::new();
        match vibration_phrase(self.effect) {
            Some(phrase) => {
                notices.push((Level::Info, games::haptic_effect_message(phrase)));
            }
            None => {
                notices.push((Level::Warn, games::unknown_haptic_message(self.effect)));
            }
        }
        notices.push((
            Level::Info,
            games::haptic_summary_message(self.effect, self.tyre),
        ));
        notices.push((Level::Trace, games::haptic_duration_message(self.duration)));
        notices.push((
            Level::Trace,
            games::haptic_frequency_message(self.frequency),
        ));
        notices.push((
            Level::Trace,
            games::haptic_amplitude_message(self.amplitude),
        ));
        notices.push((Level::Trace, games::haptic_motor_message(self.motor)));
        notices.push((Level::Trace, games::sound_subtype_message(self.effect)));
        if let Some(phrase) = vibration_phrase(self.effect) {
            notices.push((Level::Info, games::sound_effect_message(phrase)));
        }
        notices.push((Level::Trace, games::sound_use_message(&self.path)));
        notices
    }

    fn playback(&self, node: &str) -> ShakerRequest {
        let effect_name = effect_label(self.effect).to_string();
        ShakerRequest {
            sink: self.path.clone(),
            node: node.to_string(),
            stream_name: effect_name.clone(),
            effect_name,
            tyre_name: tyre_label(self.effect, self.tyre).map(str::to_string),
            volume_percent: self.volume,
            channels: u8::try_from(self.channels).unwrap_or(CHANNEL_MIN as u8),
            mask: self.mask,
            gear: self.effect == names::EFFECT_GEAR,
        }
    }

    fn shared_voice(&self) -> Option<SharedShaker> {
        let effect = VibrationEffect::from_id(self.effect)?;
        let channels = u8::try_from(self.channels).unwrap_or(CHANNEL_MIN as u8);
        Some(Arc::new(Mutex::new(ShakerVoice::new(
            self.haptic(effect),
            channels,
        ))))
    }

    fn haptic(&self, effect: VibrationEffect) -> HapticSettings {
        HapticSettings {
            effect,
            tyre: TyreId::from_id(self.tyre),
            frequency: u32_from_i64(self.frequency),
            frequency_max: u32_from_i64(self.frequency_max),
            amplitude: u32_from_i64(self.amplitude),
            duration: self.duration,
            threshold: self.threshold,
            ..HapticSettings::default()
        }
    }

    fn stream_notices(&self) -> Vec<(Level, String)> {
        vec![
            (Level::Info, games::MSG_SOUND_STANDALONE.to_string()),
            (Level::Info, games::sound_volume_message(self.volume)),
            (Level::Info, games::sound_channel_mask_message(self.mask)),
            (Level::Info, games::sound_channels_message(self.channels)),
            (Level::Info, games::sound_noise_message(self.noise)),
        ]
    }
}

fn configured_tyre(entry: &DeviceEntry, effect: i32) -> i32 {
    if !effect_uses_tyre(effect) {
        return names::TYRE_FRONT_LEFT;
    }
    let Some(name) = entry.get_str(keys::KEY_TYRE) else {
        return names::TYRE_FRONT_LEFT;
    };
    names::tyre_or_all_four(name)
}

fn configured_duration(entry: &DeviceEntry, effect: i32) -> f64 {
    if let Some(value) = entry.get_f64(keys::KEY_DURATION) {
        return value;
    }
    if effect == names::EFFECT_GEAR {
        return GEAR_DURATION_DEFAULT_S;
    }
    DURATION_UNSET
}

fn configured_volume(entry: &DeviceEntry) -> i64 {
    let raw = entry
        .get_i64(keys::KEY_STREAM_VOLUME)
        .or_else(|| entry.get_i64(keys::KEY_VOLUME))
        .unwrap_or(VOLUME_UNITY);
    clamp_i64(raw, VOLUME_MIN, VOLUME_UNITY)
}

fn configured_channels(entry: &DeviceEntry) -> i64 {
    let raw = entry.get_i64(keys::KEY_CHANNELS).unwrap_or(CHANNEL_MIN);
    clamp_i64(raw, CHANNEL_MIN, CHANNEL_MAX)
}

fn channel_mask(entry: &DeviceEntry, channels: i64) -> u32 {
    let all = channel_mask_all(channels);
    if let Some(mask) = entry.get_i64(keys::KEY_CHANNEL_MASK) {
        return clipped_mask(mask, all);
    }
    let Some(pan) = entry.get_i64(keys::KEY_PAN) else {
        return all;
    };
    pan_mask(pan, channels, all)
}

fn channel_mask_all(channels: i64) -> u32 {
    let count = u32::try_from(channels).unwrap_or(0);
    if count == 0 || count >= u32::BITS {
        return 0;
    }
    (1u32 << count) - 1
}

fn clipped_mask(mask: i64, all: u32) -> u32 {
    let bits = u32::try_from(mask).unwrap_or(0) & all;
    if bits == 0 {
        return all;
    }
    bits
}

fn pan_mask(pan: i64, channels: i64, all: u32) -> u32 {
    if pan == SOUND_PAN_ALL || pan < 0 || pan >= channels {
        return all;
    }
    let Ok(shift) = u32::try_from(pan) else {
        return all;
    };
    1u32 << shift
}

fn u32_from_i64(value: i64) -> u32 {
    if value <= 0 {
        return 0;
    }
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn clamp_i64(value: i64, min: i64, max: i64) -> i64 {
    if value < min {
        return min;
    }
    if value > max {
        return max;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINK: &str = "alsa_output.test";
    const SAMPLE_VOLUME: i64 = 40;
    const LOUDER_VOLUME: i64 = 150;
    const STEREO: i64 = 2;
    const LEFT: i64 = 0;
    const SAMPLE_FREQUENCY: i64 = 50;
    const SAMPLE_DURATION_S: f64 = 0.1;

    fn gear_entry() -> DeviceEntry {
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_EFFECT, "Gear");
        device.set_int(keys::KEY_STREAM_VOLUME, SAMPLE_VOLUME);
        device.set_int(keys::KEY_VOLUME, VOLUME_UNITY);
        device.set_int(keys::KEY_CHANNELS, STEREO);
        device.set_int(keys::KEY_PAN, SOUND_PAN_ALL);
        device.set_int(keys::KEY_FREQUENCY, SAMPLE_FREQUENCY);
        device.set_float(keys::KEY_DURATION, SAMPLE_DURATION_S);
        device
    }

    #[test]
    fn node_names_match_the_c_stream() {
        assert_eq!(
            stream_node_name(names::EFFECT_GEAR, names::TYRE_FRONT_LEFT).as_deref(),
            Some("cargopit.Gear")
        );
        assert_eq!(
            stream_node_name(names::EFFECT_TYRE_LOCK, names::TYRE_ALL_FOUR).as_deref(),
            Some("cargopit.TyreLock.All")
        );
        assert_eq!(
            stream_node_name(names::EFFECT_SUSPENSION, names::TYRE_ALL_FOUR).as_deref(),
            Some("cargopit.Suspension.All")
        );
    }

    #[test]
    fn gear_init_uses_stream_volume_and_skips_the_tyre_suffix() {
        let opened = open_sound(&gear_entry(), names::EFFECT_GEAR, SINK, false);
        assert!(opened.ready);
        let volume = games::sound_volume_message(SAMPLE_VOLUME);
        let node = games::sound_node_message("cargopit.Gear");
        let summary = games::haptic_summary_message(names::EFFECT_GEAR, names::TYRE_FRONT_LEFT);
        let duration = games::haptic_duration_message(SAMPLE_DURATION_S);
        let motor = games::haptic_motor_message(SOUND_MOTOR_DEFAULT);
        let amplitude = games::haptic_amplitude_message(SOUND_AMPLITUDE_UNITY);
        let messages: Vec<&str> = opened
            .notices
            .iter()
            .map(|(_, message)| message.as_str())
            .collect();
        assert!(messages.contains(&volume.as_str()));
        assert!(messages.contains(&node.as_str()));
        assert!(messages.contains(&summary.as_str()));
        assert!(messages.contains(&duration.as_str()));
        assert!(messages.contains(&motor.as_str()));
        assert!(messages.contains(&amplitude.as_str()));
    }

    #[test]
    fn lock_without_haptics_does_not_init() {
        let mut device = gear_entry();
        device.set_str(keys::KEY_EFFECT, "TyreLock");
        device.set_str(keys::KEY_TYRE, "ALL");
        let opened = open_sound(&device, names::EFFECT_TYRE_LOCK, SINK, false);
        assert!(!opened.ready);
        assert_eq!(opened.notices.len(), 1);
        assert_eq!(opened.notices[0].1, games::MSG_SOUND_SKIP_HAPTICS);
    }

    #[test]
    fn volume_clamps_and_a_left_pan_selects_one_channel() {
        let mut device = DeviceEntry::new();
        device.set_int(keys::KEY_VOLUME, LOUDER_VOLUME);
        device.set_int(keys::KEY_CHANNELS, STEREO);
        device.set_int(keys::KEY_PAN, LEFT);
        let opened = open_sound(&device, names::EFFECT_ENGINE, SINK, true);
        assert!(opened.ready);
        const LEFT_CHANNEL_MASK: u32 = 1;
        let volume = games::sound_volume_message(VOLUME_UNITY);
        let mask = games::sound_channel_mask_message(LEFT_CHANNEL_MASK);
        let messages: Vec<&str> = opened
            .notices
            .iter()
            .map(|(_, message)| message.as_str())
            .collect();
        assert!(messages.contains(&volume.as_str()));
        assert!(messages.contains(&mask.as_str()));
    }
}
