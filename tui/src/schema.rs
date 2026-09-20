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
        match self {
            DeviceClass::Usb => DeviceClass::Sound,
            DeviceClass::Sound => DeviceClass::Serial,
            DeviceClass::Serial => DeviceClass::Usb,
        }
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
            FieldId::Pan => "Pan",
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
            FieldId::Pan => Some(consts::KEY_PAN),
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

const COMMON: &[FieldId] = &[FieldId::Class, FieldId::Type, FieldId::Enabled, FieldId::Fps];
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
            consts::TYPE_HAPTIC | consts::TYPE_WHEEL | consts::TYPE_USB_HAPTIC | consts::TYPE_USB_WHEEL => {
                &[FieldId::Subtype, FieldId::Devid, FieldId::Devpath]
            }
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
            consts::TYPE_HAPTIC | consts::TYPE_SERIAL_HAPTIC => {
                &[FieldId::Devpath, FieldId::Baud, FieldId::Ampfactor, FieldId::Motors]
            }
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
            consts::TYPE_CUSTOM | consts::TYPE_ARDUINO_CUSTOM => {
                &[FieldId::Devpath, FieldId::Baud, FieldId::Ampfactor, FieldId::ConfigPath]
            }
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
            consts::TYPE_HAPTIC | consts::TYPE_WHEEL | consts::TYPE_USB_HAPTIC | consts::TYPE_USB_WHEEL
        ),
        DeviceClass::Serial => matches!(type_name, consts::TYPE_HAPTIC | consts::TYPE_SERIAL_HAPTIC),
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

pub fn combo_choices(field: FieldId, class: DeviceClass, type_name: &str) -> &'static [&'static str] {
    match field {
        FieldId::Class => &[consts::CLASS_USB, consts::CLASS_SOUND, consts::CLASS_SERIAL],
        FieldId::Type => types_for_class(class),
        FieldId::Subtype => subtypes_for(class, type_name),
        FieldId::Effect => consts::EFFECTS,
        FieldId::Modulation => consts::MODULATIONS,
        FieldId::Tyre => consts::TYRES,
        FieldId::Motors => consts::MOTOR_LABELS,
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
            if device.get(consts::KEY_STREAM_VOLUME).is_none() && device.get(consts::KEY_VOLUME).is_none()
            {
                device.set_int(consts::KEY_STREAM_VOLUME, consts::DEFAULT_VOLUME);
                device.set_int(consts::KEY_VOLUME, consts::DEFAULT_VOLUME);
            }
            if device.get(consts::KEY_PAN).is_none() {
                device.set_int(consts::KEY_PAN, consts::DEFAULT_PAN);
            }
            if device.get(consts::KEY_CHANNELS).is_none() {
                device.set_int(consts::KEY_CHANNELS, consts::DEFAULT_CHANNELS);
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
            if type_name == consts::TYPE_SHIFT_LIGHTS && device.get(consts::KEY_NUMLIGHTS).is_none() {
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
        FieldId::Volume => device
            .get_i64(consts::KEY_STREAM_VOLUME)
            .or_else(|| device.get_i64(consts::KEY_VOLUME))
            .unwrap_or(consts::DEFAULT_VOLUME)
            .to_string(),
        FieldId::Motors => {
            let index = device.get_i64(consts::KEY_MOTORS).unwrap_or(0);
            motor_label(index).to_string()
        }
        FieldId::Devid => device.get_str(consts::KEY_DEVID).unwrap_or("").to_string(),
        FieldId::Devpath => device.get_str(consts::KEY_DEVPATH).unwrap_or("").to_string(),
        other => {
            let Some(key) = other.config_key() else {
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
            | FieldId::Pan
            | FieldId::Channels
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
    )
}

pub fn is_identity(field: FieldId) -> bool {
    matches!(field, FieldId::Devid | FieldId::Devpath)
}

pub fn validate(device: &DeviceEntry) -> Result<(), String> {
    let class = device.class();
    let type_name = normalize_type_name(class, device.type_name());
    if class != DeviceClass::Sound && !type_legal_for_class(class, type_name) {
        return Err(format!("type {type_name} is not legal for {}", class.as_str()));
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
    if matches!(type_name, consts::TYPE_SIMLEDS | consts::TYPE_CUSTOM | consts::TYPE_ARDUINO_CUSTOM)
    {
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
        assert!(allowed_keys(DeviceClass::Serial, consts::TYPE_WHEEL).contains(&consts::KEY_SUBTYPE));

        let mut wheel = DeviceEntry::new();
        apply_defaults(&mut wheel, DeviceClass::Serial, consts::TYPE_WHEEL);
        assert_eq!(
            wheel.get_str(consts::KEY_SUBTYPE),
            Some(consts::SUBTYPE_MOZA_R9)
        );
    }

    #[test]
    fn usb_types_do_not_accept_serial_shift_lights() {
        assert!(!type_legal_for_class(DeviceClass::Usb, consts::TYPE_SHIFT_LIGHTS));
        assert!(type_legal_for_class(DeviceClass::Serial, consts::TYPE_SHIFT_LIGHTS));
    }
}
