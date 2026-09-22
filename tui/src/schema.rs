use crate::config::DeviceEntry;
use crate::consts;
use crate::libconfig::Value;
use crate::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Usb,
    Sound,
    Serial,
}

impl DeviceClass {
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceClass::Usb => consts::CLASS_USB,
            DeviceClass::Sound => consts::CLASS_SOUND,
            DeviceClass::Serial => consts::CLASS_SERIAL,
        }
    }

    pub fn cycle(self) -> Self {
        self.cycle_by(1)
    }

    pub fn cycle_by(self, delta: i32) -> Self {
        let all = Self::all();
        let index = all.iter().position(|item| *item == self).unwrap_or(0) as i32;
        let next = (index + delta).rem_euclid(all.len() as i32) as usize;
        all[next]
    }

    pub fn all() -> [DeviceClass; 3] {
        [DeviceClass::Usb, DeviceClass::Sound, DeviceClass::Serial]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldId {
    Class,
    Type,
    Subtype,
    Enabled,
    Fps,
    Devid,
    Devpath,
    ConfigPath,
    Granularity,
    Baud,
    Ampfactor,
    Fanpower,
    Motors,
    NumLights,
    NumLeds,
    StartLed,
    EndLed,
    Volume,
    Pan,
    Channels,
    Noise,
    Effect,
    Modulation,
    Tyre,
    Frequency,
    FrequencyMax,
    Amplitude,
    AmplitudeMax,
    Threshold,
    Duration,
}

impl FieldId {
    pub fn label(self) -> &'static str {
        match self {
            FieldId::Class => "Class",
            FieldId::Type => "Type",
            FieldId::Subtype => "Hardware",
            FieldId::Enabled => "Enabled",
            FieldId::Fps => "FPS",
            FieldId::Devid => "Device id",
            FieldId::Devpath => "Device path",
            FieldId::ConfigPath => "Config file",
            FieldId::Granularity => "Granularity",
            FieldId::Baud => "Baud",
            FieldId::Ampfactor => "Amp factor",
            FieldId::Fanpower => "Fan power",
            FieldId::Motors => "Motors",
            FieldId::NumLights => "Num lights",
            FieldId::NumLeds => "Num LEDs",
            FieldId::StartLed => "Start LED",
            FieldId::EndLed => "End LED",
            FieldId::Volume => "Volume",
            FieldId::Pan => "Output",
            FieldId::Channels => "Channels",
            FieldId::Noise => "Noise",
            FieldId::Effect => "Effect",
            FieldId::Modulation => "Modulation",
            FieldId::Tyre => "Tyre",
            FieldId::Frequency => "Frequency",
            FieldId::FrequencyMax => "Frequency max",
            FieldId::Amplitude => "Amplitude",
            FieldId::AmplitudeMax => "Amplitude max",
            FieldId::Threshold => "Threshold",
            FieldId::Duration => "Duration",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            FieldId::Class => "Transport: USB HID, PulseAudio sound, or serial/Arduino.",
            FieldId::Type => "Device role: haptic, wheel, tachometer, lights, wind, or custom Lua.",
            FieldId::Subtype => "Hardware model used to pick the USB or serial protocol.",
            FieldId::Enabled => "When false, the device stays in the profile but is not started.",
            FieldId::Fps => "Device update rate, 1-1000. Unset uses cargopit's 60 fps default.",
            FieldId::Devid => "USB vendor:product id or PulseAudio sink name.",
            FieldId::Devpath => "Serial port (/dev/ttyACM*) or sysfs rumble path.",
            FieldId::ConfigPath => "Tachometer XML or Lua script path for this device.",
            FieldId::Granularity => "Tachometer LED grouping (1, 2, or 4). Unset uses 1.",
            FieldId::Baud => "Serial baud rate. Unset uses 9600.",
            FieldId::Ampfactor => "Scales serial haptic output. Unset uses 1.0.",
            FieldId::Fanpower => "SimWind fan power from 0 to 1. Unset uses 0.6.",
            FieldId::Motors => {
                "Which shaker motors receive this effect. Unset leaves the engine default."
            }
            FieldId::NumLights => "Shift-light LED count. Unset uses 6.",
            FieldId::NumLeds => "Simleds strip length. Unset uses 6.",
            FieldId::StartLed => "First LED in the strip that this effect owns (1-based).",
            FieldId::EndLed => "Last LED in the strip that this effect owns (1-based).",
            FieldId::Volume => "PulseAudio volume 0-100. Unset uses the engine unity default.",
            FieldId::Pan => {
                "Speakers that play this effect. Left/right selects a speaker, space toggles it. Backspace restores all channels."
            }
            FieldId::Channels => "Mix width (1, 2, 4, 6, or 8). Unset uses 2.",
            FieldId::Noise => "Extra hertz added to the tone. Unset uses 0.",
            FieldId::Effect => "Telemetry source that drives this haptic or sound device.",
            FieldId::Modulation => "Frequency or amplitude modulation, or none. Unset is none.",
            FieldId::Tyre => "Which tyre(s) feed slip, lock, ABS, or suspension effects.",
            FieldId::Frequency => "Base tone or motor frequency. Unset uses 0 (engine default).",
            FieldId::FrequencyMax => {
                "Upper frequency when modulation is Frequency. Must be greater than Frequency."
            }
            FieldId::Amplitude => "Base effect strength. Unset uses the engine unity default.",
            FieldId::AmplitudeMax => "Upper strength when modulation is Amplitude.",
            FieldId::Threshold => "Minimum telemetry value before the effect plays. Unset uses 0.",
            FieldId::Duration => "Gear-shift pulse length in seconds. Unset uses 0.125 for Gear.",
        }
    }

    pub fn config_key(self) -> Option<&'static str> {
        match self {
            FieldId::Class => Some(consts::KEY_DEVICE),
            FieldId::Type => Some(consts::KEY_TYPE),
            FieldId::Subtype => Some(consts::KEY_SUBTYPE),
            FieldId::Enabled => Some(consts::KEY_ENABLED),
            FieldId::Fps => Some(consts::KEY_FPS),
            FieldId::Devid => Some(consts::KEY_DEVID),
            FieldId::Devpath => Some(consts::KEY_DEVPATH),
            FieldId::ConfigPath => Some(consts::KEY_CONFIG),
            FieldId::Granularity => Some(consts::KEY_GRANULARITY),
            FieldId::Baud => Some(consts::KEY_BAUD),
            FieldId::Ampfactor => Some(consts::KEY_AMPFACTOR),
            FieldId::Fanpower => Some(consts::KEY_FANPOWER),
            FieldId::Motors => Some(consts::KEY_MOTORS),
            FieldId::NumLights => Some(consts::KEY_NUMLIGHTS),
            FieldId::NumLeds => Some(consts::KEY_NUMLEDS),
            FieldId::StartLed => Some(consts::KEY_STARTLED),
            FieldId::EndLed => Some(consts::KEY_ENDLED),
            FieldId::Volume => Some(consts::KEY_STREAM_VOLUME),
            FieldId::Pan => Some(consts::KEY_CHANNEL_MASK),
            FieldId::Channels => Some(consts::KEY_CHANNELS),
            FieldId::Noise => Some(consts::KEY_NOISE),
            FieldId::Effect => Some(consts::KEY_EFFECT),
            FieldId::Modulation => Some(consts::KEY_MODULATION),
            FieldId::Tyre => Some(consts::KEY_TYRE),
            FieldId::Frequency => Some(consts::KEY_FREQUENCY),
            FieldId::FrequencyMax => Some(consts::KEY_FREQUENCY_MAX),
            FieldId::Amplitude => Some(consts::KEY_AMPLITUDE),
            FieldId::AmplitudeMax => Some(consts::KEY_AMPLITUDE_MAX),
            FieldId::Threshold => Some(consts::KEY_THRESHOLD),
            FieldId::Duration => Some(consts::KEY_DURATION),
        }
    }
}

const COMMON: &[FieldId] = &[
    FieldId::Class,
    FieldId::Type,
    FieldId::Enabled,
    FieldId::Fps,
];
const HAPTIC_BLOCK: &[FieldId] = &[
    FieldId::Effect,
    FieldId::Modulation,
    FieldId::Tyre,
    FieldId::Frequency,
    FieldId::FrequencyMax,
    FieldId::Amplitude,
    FieldId::AmplitudeMax,
    FieldId::Threshold,
    FieldId::Duration,
];

fn type_fields(class: DeviceClass, type_name: &str) -> &'static [FieldId] {
    match class {
        DeviceClass::Usb => match type_name {
            consts::TYPE_TACHOMETER => &[
                FieldId::Subtype,
                FieldId::Devid,
                FieldId::Granularity,
                FieldId::ConfigPath,
            ],
            consts::TYPE_HAPTIC
            | consts::TYPE_WHEEL
            | consts::TYPE_USB_HAPTIC
            | consts::TYPE_USB_WHEEL => &[FieldId::Subtype, FieldId::Devid, FieldId::Devpath],
            _ => &[FieldId::Subtype, FieldId::Devid],
        },
        DeviceClass::Sound => &[
            FieldId::Devid,
            FieldId::Volume,
            FieldId::Pan,
            FieldId::Channels,
            FieldId::Noise,
        ],
        DeviceClass::Serial => match type_name {
            consts::TYPE_SIM_WIND => &[
                FieldId::Devpath,
                FieldId::Baud,
                FieldId::Ampfactor,
                FieldId::Fanpower,
            ],
            consts::TYPE_HAPTIC | consts::TYPE_SERIAL_HAPTIC => &[
                FieldId::Devpath,
                FieldId::Baud,
                FieldId::Ampfactor,
                FieldId::Motors,
            ],
            consts::TYPE_SHIFT_LIGHTS => &[
                FieldId::Devpath,
                FieldId::Baud,
                FieldId::Ampfactor,
                FieldId::NumLights,
            ],
            consts::TYPE_SIMLEDS => &[
                FieldId::Devpath,
                FieldId::Baud,
                FieldId::Ampfactor,
                FieldId::NumLeds,
                FieldId::StartLed,
                FieldId::EndLed,
                FieldId::ConfigPath,
            ],
            consts::TYPE_CUSTOM | consts::TYPE_ARDUINO_CUSTOM => &[
                FieldId::Devpath,
                FieldId::Baud,
                FieldId::Ampfactor,
                FieldId::ConfigPath,
            ],
            consts::TYPE_WHEEL => &[
                FieldId::Subtype,
                FieldId::Devpath,
                FieldId::Baud,
                FieldId::Ampfactor,
            ],
            _ => &[FieldId::Devpath, FieldId::Baud, FieldId::Ampfactor],
        },
    }
}

fn has_haptic(class: DeviceClass, type_name: &str) -> bool {
    match class {
        DeviceClass::Sound => true,
        DeviceClass::Usb => matches!(
            type_name,
            consts::TYPE_HAPTIC
                | consts::TYPE_WHEEL
                | consts::TYPE_USB_HAPTIC
                | consts::TYPE_USB_WHEEL
        ),
        DeviceClass::Serial => {
            matches!(type_name, consts::TYPE_HAPTIC | consts::TYPE_SERIAL_HAPTIC)
        }
    }
}

pub fn visible_fields(class: DeviceClass, type_name: &str) -> Vec<FieldId> {
    let mut fields = COMMON.to_vec();
    fields.extend_from_slice(type_fields(class, type_name));
    if has_haptic(class, type_name) {
        fields.extend_from_slice(HAPTIC_BLOCK);
    }
    fields
}

pub fn types_for_class(class: DeviceClass) -> &'static [&'static str] {
    match class {
        DeviceClass::Usb => consts::USB_TYPES,
        DeviceClass::Sound => consts::SOUND_TYPES,
        DeviceClass::Serial => consts::SERIAL_TYPES,
    }
}

pub fn subtypes_for(class: DeviceClass, type_name: &str) -> &'static [&'static str] {
    match class {
        DeviceClass::Usb if type_name == consts::TYPE_TACHOMETER => consts::TACHOMETER_SUBTYPES,
        DeviceClass::Usb => consts::USB_HARDWARE_SUBTYPES,
        DeviceClass::Serial if type_name == consts::TYPE_WHEEL => consts::SERIAL_WHEEL_SUBTYPES,
        _ => &[],
    }
}

pub fn combo_choices(
    field: FieldId,
    class: DeviceClass,
    type_name: &str,
) -> &'static [&'static str] {
    match field {
        FieldId::Class => &[consts::CLASS_USB, consts::CLASS_SOUND, consts::CLASS_SERIAL],
        FieldId::Type => types_for_class(class),
        FieldId::Subtype => subtypes_for(class, type_name),
        FieldId::Effect => consts::EFFECTS,
        FieldId::Modulation => consts::MODULATIONS,
        FieldId::Tyre => consts::TYRES,
        FieldId::Motors => consts::MOTOR_LABELS,
        FieldId::Channels => consts::SOUND_CHANNEL_COUNT_LABELS,
        _ => &[],
    }
}

pub fn allowed_keys(class: DeviceClass, type_name: &str) -> Vec<&'static str> {
    let mut keys = vec![
        consts::KEY_DEVICE,
        consts::KEY_TYPE,
        consts::KEY_ENABLED,
        consts::KEY_FPS,
    ];
    for field in visible_fields(class, type_name) {
        if let Some(key) = field.config_key() {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    if class == DeviceClass::Sound {
        keys.push(consts::KEY_VOLUME);
        keys.push(consts::KEY_STREAM_VOLUME);
        keys.push(consts::KEY_PAN);
        keys.push(consts::KEY_CHANNEL_MASK);
    }
    keys
}

pub fn normalize_type_name(class: DeviceClass, raw: &str) -> &'static str {
    let lowered = raw.to_ascii_lowercase();
    match class {
        DeviceClass::Usb => {
            if lowered == consts::TYPE_TACHOMETER.to_ascii_lowercase() {
                consts::TYPE_TACHOMETER
            } else if lowered.contains("wheel") {
                consts::TYPE_WHEEL
            } else {
                consts::TYPE_HAPTIC
            }
        }
        DeviceClass::Sound => consts::TYPE_HAPTIC,
        DeviceClass::Serial => {
            if lowered == consts::TYPE_SHIFT_LIGHTS.to_ascii_lowercase() {
                consts::TYPE_SHIFT_LIGHTS
            } else if lowered == consts::TYPE_SIM_WIND.to_ascii_lowercase() {
                consts::TYPE_SIM_WIND
            } else if lowered == consts::TYPE_SIMLEDS.to_ascii_lowercase() {
                consts::TYPE_SIMLEDS
            } else if lowered.contains("custom") {
                consts::TYPE_CUSTOM
            } else if lowered.contains("wheel") {
                consts::TYPE_WHEEL
            } else {
                consts::TYPE_HAPTIC
            }
        }
    }
}

pub fn type_legal_for_class(class: DeviceClass, type_name: &str) -> bool {
    types_for_class(class)
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(type_name))
}

pub fn default_type(class: DeviceClass) -> &'static str {
    types_for_class(class)[0]
}

pub fn apply_defaults(device: &mut DeviceEntry, class: DeviceClass, type_name: &str) {
    device.set_str(consts::KEY_DEVICE, class.as_str());
    if class == DeviceClass::Sound {
        device.remove(consts::KEY_TYPE);
    } else {
        device.set_str(consts::KEY_TYPE, type_name);
    }
    if device.get(consts::KEY_ENABLED).is_none() {
        device.set_bool(consts::KEY_ENABLED, consts::DEFAULT_ENABLED);
    }
    if device.get(consts::KEY_FPS).is_none() {
        device.set_int(consts::KEY_FPS, consts::DEFAULT_FPS);
    }
    match class {
        DeviceClass::Usb => {
            if type_name == consts::TYPE_TACHOMETER {
                if device.get(consts::KEY_SUBTYPE).is_none() {
                    device.set_str(consts::KEY_SUBTYPE, consts::SUBTYPE_REVBURNER);
                }
                if device.get(consts::KEY_GRANULARITY).is_none() {
                    device.set_int(consts::KEY_GRANULARITY, consts::DEFAULT_GRANULARITY);
                }
            } else if device.get(consts::KEY_SUBTYPE).is_none() {
                device.set_str(consts::KEY_SUBTYPE, consts::SUBTYPE_CSL_ELITE_V3);
            }
        }
        DeviceClass::Sound => {
            if device.get(consts::KEY_STREAM_VOLUME).is_none()
                && device.get(consts::KEY_VOLUME).is_none()
            {
                device.set_int(consts::KEY_STREAM_VOLUME, consts::DEFAULT_VOLUME);
                device.set_int(consts::KEY_VOLUME, consts::DEFAULT_VOLUME);
            }
            if device.get(consts::KEY_CHANNELS).is_none() {
                device.set_int(consts::KEY_CHANNELS, consts::DEFAULT_CHANNELS);
            }
            if device.get(consts::KEY_CHANNEL_MASK).is_none()
                && device.get(consts::KEY_PAN).is_none()
            {
                write_sound_channel_mask(device, sound_channel_mask_all(consts::DEFAULT_CHANNELS));
            }
            if device.get(consts::KEY_NOISE).is_none() {
                device.set_int(consts::KEY_NOISE, consts::DEFAULT_NOISE);
            }
        }
        DeviceClass::Serial => {
            if device.get(consts::KEY_BAUD).is_none() {
                device.set_int(consts::KEY_BAUD, consts::DEFAULT_BAUD);
            }
            if device.get(consts::KEY_AMPFACTOR).is_none() {
                device.set_float(consts::KEY_AMPFACTOR, consts::DEFAULT_AMPFACTOR);
            }
            if type_name == consts::TYPE_WHEEL && device.get(consts::KEY_SUBTYPE).is_none() {
                device.set_str(consts::KEY_SUBTYPE, consts::SUBTYPE_MOZA_R9);
            }
            if type_name == consts::TYPE_SIM_WIND && device.get(consts::KEY_FANPOWER).is_none() {
                device.set_float(consts::KEY_FANPOWER, consts::DEFAULT_FANPOWER);
            }
            if type_name == consts::TYPE_SHIFT_LIGHTS && device.get(consts::KEY_NUMLIGHTS).is_none()
            {
                device.set_int(consts::KEY_NUMLIGHTS, consts::DEFAULT_NUMLIGHTS);
            }
            if type_name == consts::TYPE_SIMLEDS {
                if device.get(consts::KEY_NUMLEDS).is_none() {
                    device.set_int(consts::KEY_NUMLEDS, consts::DEFAULT_NUMLEDS);
                }
                if device.get(consts::KEY_STARTLED).is_none() {
                    device.set_int(consts::KEY_STARTLED, consts::DEFAULT_STARTLED);
                }
                if device.get(consts::KEY_ENDLED).is_none() {
                    device.set_int(consts::KEY_ENDLED, consts::DEFAULT_ENDLED);
                }
            }
        }
    }
    if has_haptic(class, type_name) && device.get(consts::KEY_EFFECT).is_none() {
        device.set_str(consts::KEY_EFFECT, consts::EFFECT_ENGINE);
        device.set_str(consts::KEY_MODULATION, consts::MODULATION_FREQUENCY);
        device.set_str(consts::KEY_TYRE, consts::TYRE_ALL);
        device.set_int(consts::KEY_FREQUENCY, consts::DEFAULT_FREQUENCY);
        device.set_int(consts::KEY_FREQUENCY_MAX, consts::DEFAULT_FREQUENCY_MAX);
        device.set_int(consts::KEY_AMPLITUDE, consts::DEFAULT_AMPLITUDE);
        device.set_int(consts::KEY_AMPLITUDE_MAX, consts::DEFAULT_AMPLITUDE_MAX);
        device.set_float(consts::KEY_THRESHOLD, consts::DEFAULT_THRESHOLD);
        device.set_float(consts::KEY_DURATION, consts::DEFAULT_DURATION);
    }
}

pub fn display_value(device: &DeviceEntry, field: FieldId) -> String {
    match field {
        FieldId::Class => device.class().as_str().to_string(),
        FieldId::Type => normalize_type_name(device.class(), device.type_name()).to_string(),
        FieldId::Enabled => {
            if device.enabled() {
                "true".into()
            } else {
                "false".into()
            }
        }
        FieldId::Volume => volume_display(device),
        FieldId::Motors => motors_display(device),
        FieldId::Pan => sound_output_display(device),
        FieldId::Devid => device.get_str(consts::KEY_DEVID).unwrap_or("").to_string(),
        FieldId::Devpath => device
            .get_str(consts::KEY_DEVPATH)
            .unwrap_or("")
            .to_string(),
        other => stored_display(device, other),
    }
}

fn volume_display(device: &DeviceEntry) -> String {
    let value = device
        .get_i64(consts::KEY_STREAM_VOLUME)
        .or_else(|| device.get_i64(consts::KEY_VOLUME));
    match value {
        Some(volume) => volume.to_string(),
        None => String::new(),
    }
}

fn motors_display(device: &DeviceEntry) -> String {
    match device.get_i64(consts::KEY_MOTORS) {
        Some(index) => motor_label(index).to_string(),
        None => String::new(),
    }
}

fn stored_display(device: &DeviceEntry, field: FieldId) -> String {
    let Some(key) = field.config_key() else {
        return String::new();
    };
    match device.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Int(v)) => v.to_string(),
        Some(Value::Float(v)) => v.to_string(),
        Some(Value::Bool(v)) => v.to_string(),
        _ => String::new(),
    }
}

pub fn is_optional(field: FieldId) -> bool {
    !matches!(field, FieldId::Class | FieldId::Type | FieldId::Enabled)
}

pub fn field_is_set(device: &DeviceEntry, field: FieldId) -> bool {
    match field {
        FieldId::Class | FieldId::Type | FieldId::Enabled => true,
        FieldId::Volume => {
            device.get(consts::KEY_STREAM_VOLUME).is_some()
                || device.get(consts::KEY_VOLUME).is_some()
        }
        FieldId::Pan => {
            device.get(consts::KEY_CHANNEL_MASK).is_some() || device.get(consts::KEY_PAN).is_some()
        }
        other => other
            .config_key()
            .map(|key| device.get(key).is_some())
            .unwrap_or(false),
    }
}

pub fn clear_field(device: &mut DeviceEntry, field: FieldId) {
    if !is_optional(field) {
        return;
    }
    if field == FieldId::Volume {
        device.remove(consts::KEY_STREAM_VOLUME);
        device.remove(consts::KEY_VOLUME);
        return;
    }
    if field == FieldId::Pan {
        write_sound_channel_mask(device, sound_channel_mask_all(sound_channel_count(device)));
        return;
    }
    if let Some(key) = field.config_key() {
        device.remove(key);
    }
}

pub fn motor_label(index: i64) -> &'static str {
    consts::MOTOR_LABELS
        .get(index as usize)
        .copied()
        .unwrap_or(consts::MOTOR_LABELS[0])
}

pub fn motor_index(label: &str) -> i64 {
    consts::MOTOR_LABELS
        .iter()
        .position(|item| *item == label)
        .unwrap_or(0) as i64
}

pub fn sound_channel_count(device: &DeviceEntry) -> i64 {
    let raw = device
        .get_i64(consts::KEY_CHANNELS)
        .unwrap_or(consts::DEFAULT_CHANNELS);
    raw.clamp(
        consts::SOUND_CHANNEL_COUNT_MIN,
        consts::SOUND_CHANNEL_COUNT_MAX,
    )
}

pub fn sound_channel_mask_all(channels: i64) -> u32 {
    let count = channels.clamp(
        consts::SOUND_CHANNEL_COUNT_MIN,
        consts::SOUND_CHANNEL_COUNT_MAX,
    );
    (1u32 << count as u32) - 1
}

pub fn sound_channel_bit(index: i64) -> u32 {
    if index < 0 || index >= consts::SOUND_CHANNEL_COUNT_MAX {
        return 0;
    }
    1u32 << index as u32
}

pub fn sound_resolve_channel_mask(device: &DeviceEntry) -> u32 {
    let channels = sound_channel_count(device);
    let all = sound_channel_mask_all(channels);
    if let Some(mask) = device.get_i64(consts::KEY_CHANNEL_MASK) {
        let clipped = mask as u32 & all;
        if clipped != 0 {
            return clipped;
        }
        return all;
    }
    let Some(pan) = device.get_i64(consts::KEY_PAN) else {
        return all;
    };
    if pan == consts::SOUND_PAN_ALL_CHANNELS || pan < 0 || pan >= channels {
        return all;
    }
    sound_channel_bit(pan)
}

pub fn sound_first_channel(mask: u32) -> i64 {
    for index in 0..consts::SOUND_CHANNEL_COUNT_MAX {
        if mask & sound_channel_bit(index) != 0 {
            return index;
        }
    }
    consts::DEFAULT_PAN
}

pub fn write_sound_channel_mask(device: &mut DeviceEntry, mask: u32) {
    let channels = sound_channel_count(device);
    let all = sound_channel_mask_all(channels);
    let mut clipped = mask & all;
    if clipped == 0 {
        clipped = all;
    }
    device.set_int(consts::KEY_CHANNEL_MASK, i64::from(clipped));
    device.set_int(consts::KEY_PAN, sound_first_channel(clipped));
}

pub fn clip_sound_output(device: &mut DeviceEntry) {
    write_sound_channel_mask(device, sound_resolve_channel_mask(device));
}

pub fn toggle_sound_channel(device: &mut DeviceEntry, index: usize) {
    let channels = sound_channel_count(device) as usize;
    if index >= channels {
        return;
    }
    let bit = sound_channel_bit(index as i64);
    let mask = sound_resolve_channel_mask(device);
    if mask & bit != 0 {
        let next = mask & !bit;
        if next == 0 {
            return;
        }
        write_sound_channel_mask(device, next);
        return;
    }
    write_sound_channel_mask(device, mask | bit);
}

pub fn sound_channel_label(channels: i64, index: usize) -> &'static str {
    let layout = sound_channel_layout(channels);
    if let Some(label) = layout.get(index) {
        return label;
    }
    consts::SOUND_LAYOUT_NUMBERED
        .get(index)
        .copied()
        .unwrap_or(consts::SOUND_LAYOUT_NUMBERED[0])
}

pub fn sound_channel_layout(channels: i64) -> &'static [&'static str] {
    match channels {
        consts::SOUND_CHANNELS_MONO => consts::SOUND_LAYOUT_MONO,
        consts::SOUND_CHANNELS_STEREO => consts::SOUND_LAYOUT_STEREO,
        consts::SOUND_CHANNELS_QUAD => consts::SOUND_LAYOUT_QUAD,
        consts::SOUND_CHANNELS_SURROUND_51 => consts::SOUND_LAYOUT_51,
        consts::SOUND_CHANNELS_SURROUND_71 => consts::SOUND_LAYOUT_71,
        _ => consts::SOUND_LAYOUT_NUMBERED,
    }
}

fn sound_output_display(device: &DeviceEntry) -> String {
    let channels = sound_channel_count(device);
    let mask = sound_resolve_channel_mask(device);
    if mask == sound_channel_mask_all(channels) {
        return consts::SOUND_OUTPUT_ALL.to_string();
    }
    let mut labels = Vec::new();
    for index in 0..channels as usize {
        if mask & sound_channel_bit(index as i64) == 0 {
            continue;
        }
        labels.push(sound_channel_label(channels, index));
    }
    labels.join(consts::SOUND_OUTPUT_SEP)
}

pub fn is_numeric(field: FieldId) -> bool {
    matches!(
        field,
        FieldId::Fps
            | FieldId::Granularity
            | FieldId::Baud
            | FieldId::Ampfactor
            | FieldId::Fanpower
            | FieldId::NumLights
            | FieldId::NumLeds
            | FieldId::StartLed
            | FieldId::EndLed
            | FieldId::Volume
            | FieldId::Noise
            | FieldId::Frequency
            | FieldId::FrequencyMax
            | FieldId::Amplitude
            | FieldId::AmplitudeMax
            | FieldId::Threshold
            | FieldId::Duration
    )
}

pub fn is_float(field: FieldId) -> bool {
    matches!(
        field,
        FieldId::Ampfactor | FieldId::Fanpower | FieldId::Threshold | FieldId::Duration
    )
}

pub fn is_combo(field: FieldId) -> bool {
    matches!(
        field,
        FieldId::Class
            | FieldId::Type
            | FieldId::Subtype
            | FieldId::Enabled
            | FieldId::Effect
            | FieldId::Modulation
            | FieldId::Tyre
            | FieldId::Motors
            | FieldId::Granularity
            | FieldId::Channels
            | FieldId::Pan
    )
}

pub fn is_identity(field: FieldId) -> bool {
    matches!(field, FieldId::Devid | FieldId::Devpath)
}

pub fn validate(device: &DeviceEntry) -> Result<(), String> {
    let class = device.class();
    let type_name = normalize_type_name(class, device.type_name());
    if class != DeviceClass::Sound && !type_legal_for_class(class, type_name) {
        return Err(format!(
            "type {type_name} is not legal for {}",
            class.as_str()
        ));
    }
    let identity_empty = match class {
        DeviceClass::Serial => device.get_str(consts::KEY_DEVPATH).unwrap_or("").is_empty(),
        DeviceClass::Sound => device.get_str(consts::KEY_DEVID).unwrap_or("").is_empty(),
        DeviceClass::Usb => {
            let devid = device.get_str(consts::KEY_DEVID).unwrap_or("");
            let devpath = device.get_str(consts::KEY_DEVPATH).unwrap_or("");
            devid.is_empty() && devpath.is_empty()
        }
    };
    if identity_empty {
        return Err("device identity is required".into());
    }
    if type_name == consts::TYPE_TACHOMETER {
        let gran = device
            .get_i64(consts::KEY_GRANULARITY)
            .unwrap_or(consts::DEFAULT_GRANULARITY);
        if !consts::GRANULARITY_ALLOWED.contains(&gran) {
            return Err("granularity must be 1, 2, or 4".into());
        }
        let path = device.get_str(consts::KEY_CONFIG).unwrap_or("");
        if path.is_empty() {
            return Err("tachometer XML path is required".into());
        }
        if !paths::expand_tilde(path).exists() {
            return Err(format!("tachometer XML not found: {path}"));
        }
    }
    if matches!(
        type_name,
        consts::TYPE_SIMLEDS | consts::TYPE_CUSTOM | consts::TYPE_ARDUINO_CUSTOM
    ) {
        let path = device.get_str(consts::KEY_CONFIG).unwrap_or("");
        if path.is_empty() {
            return Err("Lua config path is required".into());
        }
        if !paths::expand_tilde(path).exists() {
            return Err(format!("Lua config not found: {path}"));
        }
    }
    if class == DeviceClass::Sound {
        let volume = device
            .get_i64(consts::KEY_STREAM_VOLUME)
            .or_else(|| device.get_i64(consts::KEY_VOLUME))
            .unwrap_or(consts::DEFAULT_VOLUME);
        if volume < consts::VOLUME_MIN || volume > consts::VOLUME_MAX {
            return Err("volume must be between 0 and 100".into());
        }
    }
    let modulation = device
        .get_str(consts::KEY_MODULATION)
        .unwrap_or(consts::MODULATION_NONE);
    if modulation.eq_ignore_ascii_case(consts::MODULATION_FREQUENCY)
        || modulation.eq_ignore_ascii_case(consts::MODULATION_FREQUENCY_ALT)
    {
        let freq = device.get_i64(consts::KEY_FREQUENCY).unwrap_or(0);
        let freq_max = device.get_i64(consts::KEY_FREQUENCY_MAX).unwrap_or(0);
        if freq_max <= freq {
            return Err("frequencyMax must be greater than frequency".into());
        }
    }
    Ok(())
}

pub fn catalog_field_ids() -> &'static [FieldId] {
    &[
        FieldId::Class,
        FieldId::Type,
        FieldId::Subtype,
        FieldId::Enabled,
        FieldId::Fps,
        FieldId::Devid,
        FieldId::Devpath,
        FieldId::ConfigPath,
        FieldId::Granularity,
        FieldId::Baud,
        FieldId::Ampfactor,
        FieldId::Fanpower,
        FieldId::Motors,
        FieldId::NumLights,
        FieldId::NumLeds,
        FieldId::StartLed,
        FieldId::EndLed,
        FieldId::Volume,
        FieldId::Pan,
        FieldId::Channels,
        FieldId::Noise,
        FieldId::Effect,
        FieldId::Modulation,
        FieldId::Tyre,
        FieldId::Frequency,
        FieldId::FrequencyMax,
        FieldId::Amplitude,
        FieldId::AmplitudeMax,
        FieldId::Threshold,
        FieldId::Duration,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DeviceEntry;

    #[test]
    fn class_cycle_by_respects_direction() {
        assert_eq!(DeviceClass::Usb.cycle_by(1), DeviceClass::Sound);
        assert_eq!(DeviceClass::Usb.cycle_by(-1), DeviceClass::Serial);
        assert_eq!(DeviceClass::Sound.cycle_by(-1), DeviceClass::Usb);
    }

    #[test]
    fn rejects_bad_granularity_and_volume() {
        let mut tach = DeviceEntry::new();
        apply_defaults(&mut tach, DeviceClass::Usb, consts::TYPE_TACHOMETER);
        tach.set_str(consts::KEY_DEVID, "1111:2222");
        tach.set_str(consts::KEY_CONFIG, "/no/such/file.xml");
        tach.set_int(consts::KEY_GRANULARITY, 3);
        assert!(validate(&tach).is_err());

        let mut sound = DeviceEntry::new();
        apply_defaults(&mut sound, DeviceClass::Sound, consts::TYPE_HAPTIC);
        sound.set_str(consts::KEY_DEVID, "sink");
        sound.set_int(consts::KEY_STREAM_VOLUME, 150);
        sound.set_int(consts::KEY_VOLUME, 150);
        assert!(validate(&sound).is_err());
    }

    #[test]
    fn serial_simleds_includes_lua_and_ampfactor() {
        let fields = visible_fields(DeviceClass::Serial, consts::TYPE_SIMLEDS);
        assert!(fields.contains(&FieldId::Ampfactor));
        assert!(fields.contains(&FieldId::ConfigPath));
        assert!(fields.contains(&FieldId::NumLeds));
    }

    #[test]
    fn serial_wheel_keeps_moza_r9_subtype() {
        let fields = visible_fields(DeviceClass::Serial, consts::TYPE_WHEEL);
        assert!(fields.contains(&FieldId::Subtype));
        assert!(fields.contains(&FieldId::Devpath));
        assert_eq!(
            subtypes_for(DeviceClass::Serial, consts::TYPE_WHEEL),
            consts::SERIAL_WHEEL_SUBTYPES
        );
        assert!(
            allowed_keys(DeviceClass::Serial, consts::TYPE_WHEEL).contains(&consts::KEY_SUBTYPE)
        );

        let mut wheel = DeviceEntry::new();
        apply_defaults(&mut wheel, DeviceClass::Serial, consts::TYPE_WHEEL);
        assert_eq!(
            wheel.get_str(consts::KEY_SUBTYPE),
            Some(consts::SUBTYPE_MOZA_R9)
        );
    }

    #[test]
    fn usb_types_do_not_accept_serial_shift_lights() {
        assert!(!type_legal_for_class(
            DeviceClass::Usb,
            consts::TYPE_SHIFT_LIGHTS
        ));
        assert!(type_legal_for_class(
            DeviceClass::Serial,
            consts::TYPE_SHIFT_LIGHTS
        ));
    }

    #[test]
    fn optional_fields_can_return_to_unset() {
        let mut sound = DeviceEntry::new();
        apply_defaults(&mut sound, DeviceClass::Sound, consts::TYPE_HAPTIC);
        assert!(field_is_set(&sound, FieldId::Frequency));
        clear_field(&mut sound, FieldId::Frequency);
        assert!(!field_is_set(&sound, FieldId::Frequency));
        assert!(display_value(&sound, FieldId::Frequency).is_empty());
        clear_field(&mut sound, FieldId::Class);
        assert_eq!(sound.class(), DeviceClass::Sound);
    }

    #[test]
    fn every_catalog_field_has_help() {
        for field in catalog_field_ids() {
            assert!(!field.help().is_empty(), "missing help for {:?}", field);
            assert!(
                field.label().len() <= consts::FIELD_LABEL_WIDTH,
                "label {:?} is wider than FIELD_LABEL_WIDTH",
                field
            );
        }
    }

    #[test]
    fn sound_output_resolves_legacy_pan_and_never_uses_minus_one() {
        let mut device = DeviceEntry::new();
        apply_defaults(&mut device, DeviceClass::Sound, consts::TYPE_HAPTIC);
        assert_eq!(
            display_value(&device, FieldId::Pan),
            consts::SOUND_OUTPUT_ALL
        );
        assert!(!display_value(&device, FieldId::Pan).contains('-'));

        device.remove(consts::KEY_CHANNEL_MASK);
        device.set_int(consts::KEY_CHANNELS, consts::DEFAULT_CHANNELS);
        device.set_int(consts::KEY_PAN, consts::SOUND_PAN_ALL_CHANNELS);
        assert_eq!(
            sound_resolve_channel_mask(&device),
            sound_channel_mask_all(consts::DEFAULT_CHANNELS)
        );
        assert_eq!(
            display_value(&device, FieldId::Pan),
            consts::SOUND_OUTPUT_ALL
        );

        device.set_int(consts::KEY_PAN, 1);
        assert_eq!(sound_resolve_channel_mask(&device), sound_channel_bit(1));
        assert_eq!(display_value(&device, FieldId::Pan), consts::LABEL_SOUND_FR);

        device.set_int(
            consts::KEY_CHANNEL_MASK,
            i64::from(sound_channel_bit(0) | sound_channel_bit(1)),
        );
        assert_eq!(
            display_value(&device, FieldId::Pan),
            consts::SOUND_OUTPUT_ALL
        );

        toggle_sound_channel(&mut device, 0);
        assert_eq!(sound_resolve_channel_mask(&device), sound_channel_bit(1));
        toggle_sound_channel(&mut device, 1);
        assert_eq!(sound_resolve_channel_mask(&device), sound_channel_bit(1));
        assert_eq!(device.get_i64(consts::KEY_PAN), Some(1));
        assert_ne!(
            device.get_i64(consts::KEY_PAN),
            Some(consts::SOUND_PAN_ALL_CHANNELS)
        );
    }
}
