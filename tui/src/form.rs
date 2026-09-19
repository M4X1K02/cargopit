use crate::config::DeviceEntry;
use crate::consts::*;
use crate::hardware;
use crate::libconfig::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldId {
    Class,
    Type,
    HardwareSubtype,
    Device,
    Enabled,
    Fps,
    Baud,
    Fanpower,
    Volume,
    Channels,
    Pan,
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
    Motors,
    NumLeds,
    StartLed,
    EndLed,
    Granularity,
    ConfigPath,
}

#[derive(Clone, Debug)]
pub struct DeviceForm {
    pub device: DeviceEntry,
    pub config_index: usize,
    pub device_index: Option<usize>,
    pub field_index: usize,
    pub text_editing: bool,
    pub text_buffer: String,
}

impl DeviceForm {
    pub fn new(device: DeviceEntry, config_index: usize, device_index: Option<usize>) -> Self {
        Self {
            device,
            config_index,
            device_index,
            field_index: 0,
            text_editing: false,
            text_buffer: String::new(),
        }
    }

    pub fn visible_fields(&self) -> Vec<FieldId> {
        let class = self.device.class();
        let mut fields = vec![FieldId::Class, FieldId::Type];
        if class.eq_ignore_ascii_case(CLASS_USB) {
            fields.push(FieldId::HardwareSubtype);
        }
        fields.extend([FieldId::Device, FieldId::Enabled, FieldId::Fps]);
        if class.eq_ignore_ascii_case(CLASS_SERIAL) {
            fields.push(FieldId::Baud);
            if self
                .device
                .string_or(DEVICE_TYPE_KEY, "")
                .eq_ignore_ascii_case(TYPE_SIM_WIND)
            {
                fields.push(FieldId::Fanpower);
            }
            if self.device.has_haptic_fields() {
                fields.push(FieldId::Motors);
            }
        }
        if class.eq_ignore_ascii_case(CLASS_SOUND) {
            fields.extend([
                FieldId::Volume,
                FieldId::Channels,
                FieldId::Pan,
                FieldId::Noise,
            ]);
        }
        if self.device.has_haptic_fields() {
            fields.extend([
                FieldId::Effect,
                FieldId::Modulation,
                FieldId::Tyre,
                FieldId::Frequency,
                FieldId::FrequencyMax,
                FieldId::Amplitude,
                FieldId::AmplitudeMax,
                FieldId::Threshold,
                FieldId::Duration,
            ]);
        }
        if self.device.has_led_fields() {
            fields.extend([FieldId::NumLeds, FieldId::StartLed, FieldId::EndLed]);
        }
        if class.eq_ignore_ascii_case(CLASS_USB)
            && self
                .device
                .string_or(DEVICE_TYPE_KEY, "")
                .eq_ignore_ascii_case(TYPE_TACHOMETER)
        {
            fields.extend([FieldId::Granularity, FieldId::ConfigPath]);
        }
        fields
    }

    pub fn clamp_field(&mut self) {
        let count = self.visible_fields().len();
        if count == 0 {
            self.field_index = 0;
            return;
        }
        if self.field_index >= count {
            self.field_index = count - 1;
        }
    }

    pub fn move_field(&mut self, delta: i32) {
        let count = self.visible_fields().len() as i32;
        if count == 0 {
            return;
        }
        let next = (self.field_index as i32 + delta).rem_euclid(count);
        self.field_index = next as usize;
        self.text_editing = false;
    }

    pub fn current_field(&self) -> Option<FieldId> {
        self.visible_fields().get(self.field_index).copied()
    }

    pub fn field_label(field: FieldId) -> &'static str {
        match field {
            FieldId::Class => "Class",
            FieldId::Type => "Type",
            FieldId::HardwareSubtype => "Hardware",
            FieldId::Device => "Device",
            FieldId::Enabled => "Enabled",
            FieldId::Fps => "FPS",
            FieldId::Baud => "Baud",
            FieldId::Fanpower => "Fan power",
            FieldId::Volume => "Volume",
            FieldId::Channels => "Channels",
            FieldId::Pan => "Channel / pan",
            FieldId::Noise => "Noise",
            FieldId::Effect => "Effect",
            FieldId::Modulation => "Modulation",
            FieldId::Tyre => "Tyre",
            FieldId::Frequency => "Frequency",
            FieldId::FrequencyMax => "Max frequency",
            FieldId::Amplitude => "Amplitude",
            FieldId::AmplitudeMax => "Max amplitude",
            FieldId::Threshold => "Threshold",
            FieldId::Duration => "Duration",
            FieldId::Motors => "Motors",
            FieldId::NumLeds => "LEDs",
            FieldId::StartLed => "Start LED",
            FieldId::EndLed => "End LED",
            FieldId::Granularity => "Granularity",
            FieldId::ConfigPath => "Config file",
        }
    }

    pub fn field_value(&self, field: FieldId) -> String {
        match field {
            FieldId::Class => self.device.class(),
            FieldId::Type => self.device.string_or(DEVICE_TYPE_KEY, ""),
            FieldId::HardwareSubtype => self.device.string_or(DEVICE_SUBTYPE_KEY, ""),
            FieldId::Device => self.device.identity(),
            FieldId::Enabled => self.device.bool_or(ENABLED_KEY, true).to_string(),
            FieldId::Fps => self.device.int_or(FPS_KEY, DEFAULT_FPS as i64).to_string(),
            FieldId::Baud => self
                .device
                .int_or(BAUD_KEY, DEFAULT_BAUD as i64)
                .to_string(),
            FieldId::Fanpower => self
                .device
                .float_or(FANPOWER_KEY, DEFAULT_FANPOWER)
                .to_string(),
            FieldId::Volume => volume_value(&self.device).to_string(),
            FieldId::Channels => self
                .device
                .int_or(CHANNELS_KEY, DEFAULT_CHANNELS as i64)
                .to_string(),
            FieldId::Pan => self.device.int_or(PAN_KEY, DEFAULT_PAN as i64).to_string(),
            FieldId::Noise => self
                .device
                .int_or(NOISE_KEY, DEFAULT_NOISE as i64)
                .to_string(),
            FieldId::Effect => self.device.string_or(EFFECT_KEY, EFFECT_NAMES[0]),
            FieldId::Modulation => self.device.string_or(MODULATION_KEY, MODULATION_NAMES[0]),
            FieldId::Tyre => self.device.string_or(TYRE_KEY, ""),
            FieldId::Frequency => self
                .device
                .int_or(FREQUENCY_KEY, DEFAULT_FREQUENCY as i64)
                .to_string(),
            FieldId::FrequencyMax => self
                .device
                .int_or(FREQUENCY_MAX_KEY, DEFAULT_FREQUENCY_MAX as i64)
                .to_string(),
            FieldId::Amplitude => self
                .device
                .int_or(AMPLITUDE_KEY, DEFAULT_AMPLITUDE as i64)
                .to_string(),
            FieldId::AmplitudeMax => self
                .device
                .int_or(AMPLITUDE_MAX_KEY, DEFAULT_AMPLITUDE as i64)
                .to_string(),
            FieldId::Threshold => self
                .device
                .float_or(THRESHOLD_KEY, DEFAULT_THRESHOLD)
                .to_string(),
            FieldId::Duration => self
                .device
                .float_or(DURATION_KEY, DEFAULT_DURATION)
                .to_string(),
            FieldId::Motors => self.device.int_or(MOTORS_KEY, 0).to_string(),
            FieldId::NumLeds => self
                .device
                .int_or(NUMLEDS_KEY, DEFAULT_NUMLEDS as i64)
                .to_string(),
            FieldId::StartLed => self
                .device
                .int_or(STARTLED_KEY, DEFAULT_STARTLED as i64)
                .to_string(),
            FieldId::EndLed => self
                .device
                .int_or(ENDLED_KEY, DEFAULT_ENDLED as i64)
                .to_string(),
            FieldId::Granularity => self
                .device
                .int_or(GRANULARITY_KEY, DEFAULT_GRANULARITY as i64)
                .to_string(),
            FieldId::ConfigPath => self.device.string_or(CONFIG_PATH_KEY, ""),
        }
    }

    pub fn cycle(&mut self, delta: i32) {
        let Some(field) = self.current_field() else {
            return;
        };
        match field {
            FieldId::Class => self.cycle_class(delta),
            FieldId::Type => self.cycle_list(DEVICE_TYPE_KEY, self.type_choices(), delta),
            FieldId::HardwareSubtype => {
                self.cycle_list(DEVICE_SUBTYPE_KEY, hardware_choices(&self.device), delta)
            }
            FieldId::Device => self.cycle_device(delta),
            FieldId::Enabled => {
                let next = !self.device.bool_or(ENABLED_KEY, true);
                self.device.set(ENABLED_KEY, Value::Bool(next));
            }
            FieldId::Effect => self.cycle_list(EFFECT_KEY, EFFECT_NAMES.to_vec(), delta),
            FieldId::Modulation => {
                self.cycle_list(MODULATION_KEY, MODULATION_NAMES.to_vec(), delta)
            }
            FieldId::Tyre => self.cycle_list(TYRE_KEY, TYRE_NAMES.to_vec(), delta),
            FieldId::Fps => self.nudge_int(FPS_KEY, DEFAULT_FPS as i64, delta),
            FieldId::Baud => self.nudge_int(BAUD_KEY, DEFAULT_BAUD as i64, delta * 100),
            FieldId::Volume => self.nudge_int(VOLUME_KEY, DEFAULT_VOLUME as i64, delta),
            FieldId::Channels => self.nudge_int(CHANNELS_KEY, DEFAULT_CHANNELS as i64, delta),
            FieldId::Pan => self.nudge_int(PAN_KEY, DEFAULT_PAN as i64, delta),
            FieldId::Noise => self.nudge_int(NOISE_KEY, DEFAULT_NOISE as i64, delta),
            FieldId::Frequency => self.nudge_int(FREQUENCY_KEY, DEFAULT_FREQUENCY as i64, delta),
            FieldId::FrequencyMax => {
                self.nudge_int(FREQUENCY_MAX_KEY, DEFAULT_FREQUENCY_MAX as i64, delta)
            }
            FieldId::Amplitude => self.nudge_int(AMPLITUDE_KEY, DEFAULT_AMPLITUDE as i64, delta),
            FieldId::AmplitudeMax => {
                self.nudge_int(AMPLITUDE_MAX_KEY, DEFAULT_AMPLITUDE as i64, delta)
            }
            FieldId::NumLeds => self.nudge_int(NUMLEDS_KEY, DEFAULT_NUMLEDS as i64, delta),
            FieldId::StartLed => self.nudge_int(STARTLED_KEY, DEFAULT_STARTLED as i64, delta),
            FieldId::EndLed => self.nudge_int(ENDLED_KEY, DEFAULT_ENDLED as i64, delta),
            FieldId::Granularity => {
                self.nudge_int(GRANULARITY_KEY, DEFAULT_GRANULARITY as i64, delta)
            }
            FieldId::Fanpower => {
                self.nudge_float(FANPOWER_KEY, DEFAULT_FANPOWER, delta as f64 * 0.1)
            }
            FieldId::Threshold => {
                self.nudge_float(THRESHOLD_KEY, DEFAULT_THRESHOLD, delta as f64 * 0.05)
            }
            FieldId::Duration => {
                self.nudge_float(DURATION_KEY, DEFAULT_DURATION, delta as f64 * 0.05)
            }
            FieldId::Motors => self.nudge_int(MOTORS_KEY, 0, delta),
            FieldId::ConfigPath => {}
        }
        self.clamp_field();
    }

    fn cycle_class(&mut self, delta: i32) {
        let classes = [CLASS_USB, CLASS_SOUND, CLASS_SERIAL];
        let current = self.device.class();
        let index = classes
            .iter()
            .position(|name| name.eq_ignore_ascii_case(&current))
            .unwrap_or(0) as i32;
        let next = classes[wrap_index(index + delta, classes.len())];
        let identity = self.device.identity();
        self.device = crate::config::new_device(next);
        if !identity.is_empty() {
            self.set_identity(&identity);
        }
    }

    fn type_choices(&self) -> Vec<&'static str> {
        let class = self.device.class();
        if class.eq_ignore_ascii_case(CLASS_USB) {
            USB_TYPES.to_vec()
        } else if class.eq_ignore_ascii_case(CLASS_SERIAL) {
            SERIAL_TYPES.to_vec()
        } else {
            SOUND_TYPES.to_vec()
        }
    }

    fn cycle_list(&mut self, key: &str, choices: Vec<&str>, delta: i32) {
        if choices.is_empty() {
            return;
        }
        let current = self.device.string_or(key, choices[0]);
        let index = choices
            .iter()
            .position(|name| name.eq_ignore_ascii_case(&current))
            .unwrap_or(0) as i32;
        let next = choices[wrap_index(index + delta, choices.len())];
        self.device.set_str(key, next);
    }

    fn cycle_device(&mut self, delta: i32) {
        let choices = hardware::device_choices_for_class(&self.device.class());
        if choices.is_empty() {
            return;
        }
        let current = self.device.identity();
        let index = choices
            .iter()
            .position(|name| name == &current)
            .unwrap_or(0) as i32;
        let next = choices[wrap_index(index + delta, choices.len())].clone();
        self.set_identity(&next);
    }

    fn nudge_int(&mut self, key: &str, fallback: i64, delta: i32) {
        let next = (self.device.int_or(key, fallback) + delta as i64).max(0);
        self.device.set(key, Value::Int(next));
        if key == VOLUME_KEY {
            self.device.set(STREAM_VOLUME_KEY, Value::Int(next));
        }
    }

    fn nudge_float(&mut self, key: &str, fallback: f64, delta: f64) {
        let next = (self.device.float_or(key, fallback) + delta).max(0.0);
        self.device.set(key, Value::Float(next));
    }

    pub fn begin_text_edit(&mut self) {
        let Some(field) = self.current_field() else {
            return;
        };
        if !is_text_field(field) {
            return;
        }
        self.text_editing = true;
        self.text_buffer = self.field_value(field);
    }

    pub fn commit_text_edit(&mut self) {
        if !self.text_editing {
            return;
        }
        let Some(field) = self.current_field() else {
            return;
        };
        let text = self.text_buffer.clone();
        match field {
            FieldId::Device => self.set_identity(&text),
            FieldId::ConfigPath => self.device.set_str(CONFIG_PATH_KEY, &text),
            FieldId::Fps => self.parse_int_into(FPS_KEY, DEFAULT_FPS as i64, &text),
            FieldId::Baud => self.parse_int_into(BAUD_KEY, DEFAULT_BAUD as i64, &text),
            FieldId::Volume => self.parse_int_into(VOLUME_KEY, DEFAULT_VOLUME as i64, &text),
            FieldId::Channels => self.parse_int_into(CHANNELS_KEY, DEFAULT_CHANNELS as i64, &text),
            FieldId::Pan => self.parse_int_into(PAN_KEY, DEFAULT_PAN as i64, &text),
            FieldId::Noise => self.parse_int_into(NOISE_KEY, DEFAULT_NOISE as i64, &text),
            FieldId::Frequency => {
                self.parse_int_into(FREQUENCY_KEY, DEFAULT_FREQUENCY as i64, &text)
            }
            FieldId::FrequencyMax => {
                self.parse_int_into(FREQUENCY_MAX_KEY, DEFAULT_FREQUENCY_MAX as i64, &text)
            }
            FieldId::Amplitude => {
                self.parse_int_into(AMPLITUDE_KEY, DEFAULT_AMPLITUDE as i64, &text)
            }
            FieldId::AmplitudeMax => {
                self.parse_int_into(AMPLITUDE_MAX_KEY, DEFAULT_AMPLITUDE as i64, &text)
            }
            FieldId::NumLeds => self.parse_int_into(NUMLEDS_KEY, DEFAULT_NUMLEDS as i64, &text),
            FieldId::StartLed => self.parse_int_into(STARTLED_KEY, DEFAULT_STARTLED as i64, &text),
            FieldId::EndLed => self.parse_int_into(ENDLED_KEY, DEFAULT_ENDLED as i64, &text),
            FieldId::Granularity => {
                self.parse_int_into(GRANULARITY_KEY, DEFAULT_GRANULARITY as i64, &text)
            }
            FieldId::Fanpower => self.parse_float_into(FANPOWER_KEY, DEFAULT_FANPOWER, &text),
            FieldId::Threshold => self.parse_float_into(THRESHOLD_KEY, DEFAULT_THRESHOLD, &text),
            FieldId::Duration => self.parse_float_into(DURATION_KEY, DEFAULT_DURATION, &text),
            FieldId::Motors => self.parse_int_into(MOTORS_KEY, 0, &text),
            _ => {}
        }
        self.text_editing = false;
    }

    fn parse_int_into(&mut self, key: &str, fallback: i64, text: &str) {
        let value = text.parse::<i64>().unwrap_or(fallback).max(0);
        self.device.set(key, Value::Int(value));
    }

    fn parse_float_into(&mut self, key: &str, fallback: f64, text: &str) {
        let value = text.parse::<f64>().unwrap_or(fallback).max(0.0);
        self.device.set(key, Value::Float(value));
    }

    fn set_identity(&mut self, value: &str) {
        let class = self.device.class();
        if class.eq_ignore_ascii_case(CLASS_SOUND) {
            self.device.set_str(DEVID_KEY, value);
            self.device.fields.retain(|(name, _)| name != DEVPATH_KEY);
        } else {
            self.device.set_str(DEVPATH_KEY, value);
            if looks_like_usb_id(value) {
                self.device.set_str(DEVID_KEY, value);
            }
        }
    }
}

fn volume_value(device: &DeviceEntry) -> i64 {
    device.int_or(
        VOLUME_KEY,
        device.int_or(STREAM_VOLUME_KEY, DEFAULT_VOLUME as i64),
    )
}

fn hardware_choices(device: &DeviceEntry) -> Vec<&'static str> {
    if device
        .string_or(DEVICE_TYPE_KEY, "")
        .eq_ignore_ascii_case(TYPE_TACHOMETER)
    {
        USB_TACH_SUBTYPES.to_vec()
    } else {
        USB_HARDWARE_SUBTYPES.to_vec()
    }
}

fn is_text_field(field: FieldId) -> bool {
    matches!(
        field,
        FieldId::Device
            | FieldId::ConfigPath
            | FieldId::Fps
            | FieldId::Baud
            | FieldId::Fanpower
            | FieldId::Volume
            | FieldId::Channels
            | FieldId::Pan
            | FieldId::Noise
            | FieldId::Frequency
            | FieldId::FrequencyMax
            | FieldId::Amplitude
            | FieldId::AmplitudeMax
            | FieldId::Threshold
            | FieldId::Duration
            | FieldId::Motors
            | FieldId::NumLeds
            | FieldId::StartLed
            | FieldId::EndLed
            | FieldId::Granularity
    )
}

fn wrap_index(index: i32, len: usize) -> usize {
    index.rem_euclid(len as i32) as usize
}

fn looks_like_usb_id(value: &str) -> bool {
    value.contains(':') && !value.starts_with('/')
}

pub fn field_is_combo(field: FieldId) -> bool {
    matches!(
        field,
        FieldId::Class
            | FieldId::Type
            | FieldId::HardwareSubtype
            | FieldId::Device
            | FieldId::Enabled
            | FieldId::Effect
            | FieldId::Modulation
            | FieldId::Tyre
    )
}
