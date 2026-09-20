//! Pure row/gauge helpers for Devices, tyres, and the offline tune page.

use crate::config::DeviceEntry;
use crate::consts;
use crate::schema::FieldId;
use crate::tyres::TyreCar;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRow {
    pub enabled: &'static str,
    pub summary: String,
    pub identity: String,
    pub presence: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GaugeSpec {
    pub title: &'static str,
    pub ratio: f64,
    pub label: String,
}

pub fn enabled_mark(enabled: bool) -> &'static str {
    if enabled {
        consts::ENABLED_MARK
    } else {
        consts::DISABLED_MARK
    }
}

pub fn gauge_ratio(value: f64, max: f64) -> f64 {
    if max <= 0.0 {
        return consts::GAUGE_RATIO_MIN;
    }
    (value / max).clamp(consts::GAUGE_RATIO_MIN, consts::GAUGE_RATIO_MAX)
}

pub fn volume_ratio(volume: i64) -> f64 {
    gauge_ratio(volume as f64, consts::VOLUME_MAX as f64)
}

pub fn fanpower_ratio(value: f64) -> f64 {
    gauge_ratio(value, consts::FANPOWER_GAUGE_MAX)
}

pub fn amplitude_ratio(value: i64, max: i64) -> f64 {
    let max = if max <= 0 {
        consts::DEFAULT_AMPLITUDE_MAX
    } else {
        max
    };
    gauge_ratio(value as f64, max as f64)
}

pub fn device_row(device: &DeviceEntry, presence: &str) -> DeviceRow {
    let identity = device.identity();
    let identity = if identity.is_empty() {
        consts::IDENTITY_EMPTY.to_string()
    } else {
        identity
    };
    DeviceRow {
        enabled: enabled_mark(device.enabled()),
        summary: device.summary(),
        identity,
        presence: presence.to_string(),
    }
}

pub fn tyre_cells(car: &TyreCar) -> [String; 6] {
    [
        car.sim.clone(),
        car.car.clone(),
        format!("{:.3}", car.tyre0),
        format!("{:.3}", car.tyre1),
        format!("{:.3}", car.tyre2),
        format!("{:.3}", car.tyre3),
    ]
}

pub fn tune_gauge_specs(device: &DeviceEntry, fields: &[FieldId]) -> Vec<GaugeSpec> {
    let mut specs = Vec::new();
    if fields.contains(&FieldId::Volume) {
        let volume = device
            .get_i64(consts::KEY_STREAM_VOLUME)
            .or_else(|| device.get_i64(consts::KEY_VOLUME))
            .unwrap_or(consts::DEFAULT_VOLUME);
        specs.push(GaugeSpec {
            title: consts::GAUGE_VOLUME_TITLE,
            ratio: volume_ratio(volume),
            label: format!("{volume}/{}", consts::VOLUME_MAX),
        });
    }
    if fields.contains(&FieldId::Amplitude) {
        let value = device
            .get_i64(consts::KEY_AMPLITUDE)
            .unwrap_or(consts::DEFAULT_AMPLITUDE);
        let max = device
            .get_i64(consts::KEY_AMPLITUDE_MAX)
            .unwrap_or(consts::DEFAULT_AMPLITUDE_MAX);
        specs.push(GaugeSpec {
            title: consts::GAUGE_AMPLITUDE_TITLE,
            ratio: amplitude_ratio(value, max),
            label: format!("{value}/{max}"),
        });
    }
    if fields.contains(&FieldId::Fanpower) {
        let value = device
            .get_f64(consts::KEY_FANPOWER)
            .unwrap_or(consts::DEFAULT_FANPOWER);
        specs.push(GaugeSpec {
            title: consts::GAUGE_FANPOWER_TITLE,
            ratio: fanpower_ratio(value),
            label: format!("{value:.2}"),
        });
    }
    specs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{self, DeviceClass};

    #[test]
    fn enabled_mark_uses_named_symbols() {
        assert_eq!(enabled_mark(true), consts::ENABLED_MARK);
        assert_eq!(enabled_mark(false), consts::DISABLED_MARK);
    }

    #[test]
    fn gauge_ratio_clamps_and_rejects_zero_max() {
        assert_eq!(gauge_ratio(50.0, 100.0), 0.5);
        assert_eq!(gauge_ratio(-10.0, 100.0), consts::GAUGE_RATIO_MIN);
        assert_eq!(gauge_ratio(200.0, 100.0), consts::GAUGE_RATIO_MAX);
        assert_eq!(gauge_ratio(10.0, 0.0), consts::GAUGE_RATIO_MIN);
    }

    #[test]
    fn device_row_fills_empty_identity() {
        let mut device = DeviceEntry::new();
        schema::apply_defaults(&mut device, DeviceClass::Sound, consts::TYPE_HAPTIC);
        device.set_str(consts::KEY_DEVID, "");
        let row = device_row(&device, consts::PRESENCE_UNKNOWN);
        assert_eq!(row.enabled, consts::ENABLED_MARK);
        assert_eq!(row.identity, consts::IDENTITY_EMPTY);
        assert!(row.summary.contains(consts::CLASS_SOUND));
    }

    #[test]
    fn tune_gauges_include_volume_for_sound() {
        let mut device = DeviceEntry::new();
        schema::apply_defaults(&mut device, DeviceClass::Sound, consts::TYPE_HAPTIC);
        device.set_int(consts::KEY_STREAM_VOLUME, 40);
        let specs = tune_gauge_specs(&device, &[FieldId::Volume, FieldId::Pan]);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].title, consts::GAUGE_VOLUME_TITLE);
        assert_eq!(specs[0].ratio, volume_ratio(40));
    }
}
