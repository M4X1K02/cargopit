use crate::config::DeviceEntry;
use crate::consts;
use crate::hardware::Discovery;
use crate::libconfig::Value;
use crate::schema::{self, DeviceClass, FieldId};

#[derive(Debug, Clone)]
pub struct DeviceForm {
    pub device: DeviceEntry,
    pub original: DeviceEntry,
    pub field_index: usize,
    pub is_new: bool,
    pub edit_buffer: Option<String>,
    pub error: Option<String>,
}

impl DeviceForm {
    pub fn new(device: DeviceEntry, is_new: bool) -> Self {
        Self {
            original: device.clone(),
            device,
            field_index: 0,
            is_new,
            edit_buffer: None,
            error: None,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.device != self.original || self.edit_buffer.is_some()
    }

    pub fn mark_saved(&mut self) {
        self.original = self.device.clone();
        self.is_new = false;
        self.error = None;
    }

    pub fn blank() -> Self {
        let mut device = DeviceEntry::new();
        schema::apply_defaults(&mut device, DeviceClass::Sound, consts::TYPE_HAPTIC);
        Self::new(device, true)
    }

    pub fn fields(&self) -> Vec<FieldId> {
        schema::visible_fields(
            self.device.class(),
            schema::normalize_type_name(self.device.class(), self.device.type_name()),
        )
    }

    pub fn current_field(&self) -> Option<FieldId> {
        self.fields().get(self.field_index).copied()
    }

    pub fn move_field(&mut self, delta: isize) {
        let len = self.fields().len() as isize;
        if len == 0 {
            return;
        }
        let next = (self.field_index as isize + delta).rem_euclid(len);
        self.field_index = next as usize;
        self.edit_buffer = None;
    }

    pub fn cycle_current(&mut self, discovery: &Discovery, delta: i32) {
        let Some(field) = self.current_field() else {
            return;
        };
        if field == FieldId::Enabled {
            let enabled = !self.device.enabled();
            self.device.set_bool(consts::KEY_ENABLED, enabled);
            return;
        }
        if field == FieldId::Granularity {
            self.cycle_granularity(delta);
            return;
        }
        if schema::is_identity(field) {
            self.cycle_identity(discovery, field, delta);
            return;
        }
        if schema::is_numeric(field) {
            self.nudge(field, delta, false);
            return;
        }
        let class = self.device.class();
        let type_name = schema::normalize_type_name(class, self.device.type_name());
        if field == FieldId::Class {
            self.change_class(class.cycle_by(delta));
            return;
        }
        if field == FieldId::Type {
            self.change_type(cycle_slice(schema::types_for_class(class), type_name, delta));
            return;
        }
        let choices = schema::combo_choices(field, class, type_name);
        if choices.is_empty() {
            return;
        }
        let current = schema::display_value(&self.device, field);
        let next = next_combo_value(choices, &current, delta, schema::is_optional(field));
        if next.is_empty() {
            schema::clear_field(&mut self.device, field);
            return;
        }
        self.apply_combo(field, &next);
    }

    pub fn clear_current(&mut self) {
        let Some(field) = self.current_field() else {
            return;
        };
        schema::clear_field(&mut self.device, field);
        self.edit_buffer = None;
    }

    fn cycle_granularity(&mut self, delta: i32) {
        let current = match self.device.get_i64(consts::KEY_GRANULARITY) {
            Some(value) => value.to_string(),
            None => String::new(),
        };
        let labels: Vec<String> = consts::GRANULARITY_ALLOWED.iter().map(|v| v.to_string()).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let next = next_combo_value(&refs, &current, delta, true);
        if next.is_empty() {
            schema::clear_field(&mut self.device, FieldId::Granularity);
            return;
        }
        if let Ok(value) = next.parse::<i64>() {
            self.device.set_int(consts::KEY_GRANULARITY, value);
        }
    }

    fn cycle_identity(&mut self, discovery: &Discovery, field: FieldId, delta: i32) {
        let class = self.device.class();
        let want_path = field == FieldId::Devpath;
        let choices = discovery.choices_for(class, want_path);
        let current = schema::display_value(&self.device, field);
        let mut values: Vec<&str> = vec![""];
        values.extend(choices.iter().map(|choice| choice.value.as_str()));
        let next = cycle_slice(&values, &current, delta);
        if next.is_empty() {
            schema::clear_field(&mut self.device, field);
            return;
        }
        self.device.set_str(field.config_key().unwrap_or(""), next);
    }

    pub fn nudge(&mut self, field: FieldId, delta: i32, large: bool) {
        if schema::is_float(field) {
            let step = if large {
                consts::NUDGE_FLOAT_LARGE
            } else {
                consts::NUDGE_FLOAT_SMALL
            };
            let key = field.config_key().unwrap_or("");
            let current = self.device.get_f64(key).unwrap_or(0.0);
            let next = (current + step * f64::from(delta)).max(0.0);
            self.device.set_float(key, next);
            return;
        }
        let step = if large {
            consts::NUDGE_INT_LARGE
        } else {
            consts::NUDGE_INT_SMALL
        };
        if field == FieldId::Volume {
            let current = self
                .device
                .get_i64(consts::KEY_STREAM_VOLUME)
                .or_else(|| self.device.get_i64(consts::KEY_VOLUME))
                .unwrap_or(consts::DEFAULT_VOLUME);
            let next = (current + step * i64::from(delta)).clamp(consts::VOLUME_MIN, consts::VOLUME_MAX);
            self.device.set_int(consts::KEY_STREAM_VOLUME, next);
            self.device.set_int(consts::KEY_VOLUME, next);
            return;
        }
        let Some(key) = field.config_key() else {
            return;
        };
        let current = self.device.get_i64(key).unwrap_or(0);
        self.device.set_int(key, (current + step * i64::from(delta)).max(0));
    }

    fn apply_combo(&mut self, field: FieldId, value: &str) {
        match field {
            FieldId::Motors => {
                self.device.set_int(consts::KEY_MOTORS, schema::motor_index(value));
            }
            FieldId::Volume => {
                if let Ok(parsed) = value.parse::<i64>() {
                    self.device.set_int(consts::KEY_STREAM_VOLUME, parsed);
                    self.device.set_int(consts::KEY_VOLUME, parsed);
                }
            }
            _ => {
                if let Some(key) = field.config_key() {
                    self.device.set_str(key, value);
                }
            }
        }
    }

    pub fn change_class(&mut self, class: DeviceClass) {
        let type_name = schema::default_type(class);
        let identity = self.device.identity();
        let extra = self.device.settings.clone();
        let mut next = DeviceEntry::new();
        schema::apply_defaults(&mut next, class, type_name);
        restore_compatible(&extra, &mut next, class, type_name);
        match class {
            DeviceClass::Serial => {
                if !identity.is_empty() {
                    next.set_str(consts::KEY_DEVPATH, identity);
                }
            }
            DeviceClass::Usb | DeviceClass::Sound => {
                if !identity.is_empty() {
                    next.set_str(consts::KEY_DEVID, identity);
                }
            }
        }
        self.device = next;
        self.field_index = 0;
    }

    pub fn change_type(&mut self, type_name: &str) {
        let class = self.device.class();
        let extra = self.device.settings.clone();
        let mut next = DeviceEntry::new();
        schema::apply_defaults(&mut next, class, type_name);
        restore_compatible(&extra, &mut next, class, type_name);
        self.device = next;
        self.field_index = 0;
    }

    pub fn begin_edit(&mut self) {
        let Some(field) = self.current_field() else {
            return;
        };
        if schema::is_combo(field) && field != FieldId::Granularity {
            return;
        }
        self.edit_buffer = Some(schema::display_value(&self.device, field));
    }

    pub fn commit_edit(&mut self) {
        let Some(buffer) = self.edit_buffer.take() else {
            return;
        };
        let Some(field) = self.current_field() else {
            return;
        };
        if buffer.trim().is_empty() {
            schema::clear_field(&mut self.device, field);
            return;
        }
        apply_typed(&mut self.device, field, &buffer);
    }

    pub fn cancel_edit(&mut self) {
        self.edit_buffer = None;
    }

    pub fn validate(&self) -> Result<(), String> {
        schema::validate(&self.device)
    }
}

fn restore_compatible(
    extra: &[(String, Value)],
    next: &mut DeviceEntry,
    class: DeviceClass,
    type_name: &str,
) {
    let allowed = schema::allowed_keys(class, type_name);
    for (key, value) in extra {
        if allowed.contains(&key.as_str()) {
            next.set(key, value.clone());
        }
    }
    schema::apply_defaults(next, class, type_name);
}

fn apply_typed(device: &mut DeviceEntry, field: FieldId, buffer: &str) {
    if field == FieldId::Volume {
        if let Ok(parsed) = buffer.parse::<i64>() {
            let clamped = parsed.clamp(consts::VOLUME_MIN, consts::VOLUME_MAX);
            device.set_int(consts::KEY_STREAM_VOLUME, clamped);
            device.set_int(consts::KEY_VOLUME, clamped);
        }
        return;
    }
    let Some(key) = field.config_key() else {
        return;
    };
    if schema::is_float(field) {
        if let Ok(parsed) = buffer.parse::<f64>() {
            device.set_float(key, parsed);
        }
        return;
    }
    if schema::is_numeric(field) {
        if let Ok(parsed) = buffer.parse::<i64>() {
            device.set_int(key, parsed);
        }
        return;
    }
    device.set_str(key, buffer);
}

fn cycle_slice<'a>(items: &'a [&'a str], current: &str, delta: i32) -> &'a str {
    if items.is_empty() {
        return "";
    }
    let index = items
        .iter()
        .position(|item| item.eq_ignore_ascii_case(current))
        .unwrap_or(0);
    let len = items.len() as i32;
    let next = (index as i32 + delta).rem_euclid(len) as usize;
    items[next]
}

fn next_combo_value(choices: &[&str], current: &str, delta: i32, include_blank: bool) -> String {
    if !include_blank {
        return cycle_slice(choices, current, delta).to_string();
    }
    let mut items = Vec::with_capacity(choices.len() + 1);
    items.push("");
    items.extend_from_slice(choices);
    cycle_slice(&items, current, delta).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_combo_can_return_to_blank() {
        let mut form = DeviceForm::blank();
        form.device.remove(consts::KEY_EFFECT);
        let effect_index = form
            .fields()
            .iter()
            .position(|field| *field == FieldId::Effect)
            .expect("effect field");
        form.field_index = effect_index;
        let discovery = Discovery::default();
        form.cycle_current(&discovery, 1);
        assert!(schema::field_is_set(&form.device, FieldId::Effect));
        form.cycle_current(&discovery, -1);
        assert!(!schema::field_is_set(&form.device, FieldId::Effect));
        assert!(schema::display_value(&form.device, FieldId::Effect).is_empty());
    }

    #[test]
    fn clear_current_restores_unset_numeric() {
        let mut form = DeviceForm::blank();
        form.device.remove(consts::KEY_FREQUENCY);
        let original = form.device.clone();
        form.original = original;
        let freq_index = form
            .fields()
            .iter()
            .position(|field| *field == FieldId::Frequency)
            .expect("frequency field");
        form.field_index = freq_index;
        form.nudge(FieldId::Frequency, 1, false);
        assert!(schema::field_is_set(&form.device, FieldId::Frequency));
        assert!(form.is_dirty());
        form.clear_current();
        assert!(!schema::field_is_set(&form.device, FieldId::Frequency));
        assert!(!form.is_dirty());
    }

    #[test]
    fn empty_edit_unsets_optional_field() {
        let mut form = DeviceForm::blank();
        let freq_index = form
            .fields()
            .iter()
            .position(|field| *field == FieldId::Frequency)
            .expect("frequency field");
        form.field_index = freq_index;
        form.edit_buffer = Some(String::new());
        form.commit_edit();
        assert!(!schema::field_is_set(&form.device, FieldId::Frequency));
    }
}
