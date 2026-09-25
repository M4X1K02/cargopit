//! Tyre-diameter config used while mapping. Hardware stays closed.

use std::path::Path;

use cargopit_config::keys::{
    KEY_CAR, KEY_CARS, KEY_SIM, KEY_TYRE0, KEY_TYRE1, KEY_TYRE2, KEY_TYRE3,
};
use cargopit_config::names::{EFFECT_ABS, EFFECT_TYRE_LOCK, EFFECT_TYRE_SLIP};
use cargopit_config::{parse, render, Value};
use cargopit_devices::haptic::{has_tyre_diameter, measure_tyre_diameter};
use cargopit_devices::telemetry::Telemetry;
use simapi_sys::{CAR_BYTES, OFF_CAR, WHEEL_COUNT};

pub const LOAD_NOT_FOUND: i32 = -1;
pub const LOAD_UNAVAILABLE: i32 = 1;
pub const USECONFIG_PLAY: bool = true;

const TYRE_KEYS: [&str; WHEEL_COUNT] = [KEY_TYRE0, KEY_TYRE1, KEY_TYRE2, KEY_TYRE3];

#[derive(Clone, Copy)]
pub struct TyreSimFlags {
    pub calculates_diameter: bool,
    pub supports_haptics: bool,
    pub calculates_slip: bool,
}

#[derive(Clone, Copy)]
pub struct TyreSimNeed {
    pub calculates_diameter: bool,
    pub supports_haptics: bool,
    pub calculates_slip: bool,
    pub use_config: bool,
    pub has_config_path: bool,
}

#[derive(Clone, Copy)]
pub struct TyreCheckRequest<'a> {
    pub car: &'a [u8],
    pub path: &'a Path,
    pub config_checked: bool,
}

pub struct TyreCheck {
    pub config_checked: bool,
    pub stop_timer: bool,
    pub updated: bool,
}

pub fn device_needs_tyre_diameter(effect: i32) -> bool {
    effect == EFFECT_TYRE_LOCK || effect == EFFECT_TYRE_SLIP || effect == EFFECT_ABS
}

pub fn sim_needs_tyre_diameter(need: &TyreSimNeed) -> bool {
    if need.calculates_diameter || !need.supports_haptics || need.calculates_slip {
        return false;
    }
    need.use_config && need.has_config_path
}

pub fn config_path_present(path: &Path) -> bool {
    !path.as_os_str().is_empty()
}

pub fn car_bytes(frame: &[u8]) -> &[u8] {
    if frame.len() <= OFF_CAR {
        return &[];
    }
    let end = (OFF_CAR + CAR_BYTES).min(frame.len());
    let raw = &frame[OFF_CAR..end];
    let len = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    &raw[..len]
}

pub fn load_tyre_config(sim: &mut Telemetry, car: &[u8], path: &Path, set_diameters: bool) -> i32 {
    let Ok(text) = std::fs::read_to_string(path) else {
        return LOAD_UNAVAILABLE;
    };
    let Ok(root) = parse(&text) else {
        return LOAD_UNAVAILABLE;
    };
    let Some(cars) = root.lookup(KEY_CARS).and_then(Value::as_list) else {
        return LOAD_UNAVAILABLE;
    };
    find_car(sim, car, cars, set_diameters)
}

pub fn save_tyre_config(sim: &Telemetry, car: &[u8], path: &Path) -> bool {
    let mut root = read_root(path);
    let Some(cars) = ensure_cars(&mut root) else {
        return false;
    };
    cars.push(diameter_entry(car, sim));
    std::fs::write(path, render(&root)).is_ok()
}

pub fn check_tyres(sim: &mut Telemetry, request: &TyreCheckRequest<'_>) -> TyreCheck {
    let mut checked = request.config_checked;
    let mut updated = false;
    if !request.car.is_empty() && !has_tyre_diameter(sim) && !checked {
        let _ = load_tyre_config(sim, request.car, request.path, true);
        checked = true;
        updated = has_tyre_diameter(sim);
    }
    if !request.car.is_empty() && !has_tyre_diameter(sim) && measure_tyre_diameter(sim) {
        updated = true;
    }
    let mut stop_timer = false;
    if has_tyre_diameter(sim) {
        if load_tyre_config(sim, request.car, request.path, false) < 0 {
            let _ = save_tyre_config(sim, request.car, request.path);
        }
        stop_timer = true;
    }
    TyreCheck {
        config_checked: checked,
        stop_timer,
        updated,
    }
}

fn find_car(sim: &mut Telemetry, car: &[u8], cars: &[Value], set_diameters: bool) -> i32 {
    for (index, entry) in cars.iter().enumerate() {
        if !entry_matches(entry, car, sim.simexe()) {
            continue;
        }
        if set_diameters {
            apply_diameters(sim, entry);
        }
        return i32::try_from(index).unwrap_or(LOAD_NOT_FOUND);
    }
    LOAD_NOT_FOUND
}

fn entry_matches(entry: &Value, car: &[u8], simexe: u64) -> bool {
    if car.is_empty() {
        return false;
    }
    let Some(saved) = entry.lookup(KEY_CAR).and_then(Value::as_str) else {
        return false;
    };
    if saved.is_empty() || !car.eq_ignore_ascii_case(saved.as_bytes()) {
        return false;
    }
    let stored = entry.lookup(KEY_SIM).map(stored_sim).unwrap_or(0);
    stored as u64 == simexe
}

fn stored_sim(value: &Value) -> i32 {
    let Some(raw) = value.strict_i64() else {
        return 0;
    };
    if raw < i64::from(i32::MIN) || raw > i64::from(i32::MAX) {
        return 0;
    }
    raw as i32
}

fn apply_diameters(sim: &mut Telemetry, entry: &Value) {
    let mut values = [0.0; WHEEL_COUNT];
    for (index, key) in TYRE_KEYS.iter().enumerate() {
        let Some(value) = entry.lookup(key).and_then(Value::strict_f64) else {
            return;
        };
        values[index] = value;
    }
    for (index, value) in values.iter().enumerate() {
        sim.set_tyre_diameter(index, *value);
    }
}

fn read_root(path: &Path) -> Value {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Value::Group(Vec::new());
    };
    parse(&text).unwrap_or_else(|_| Value::Group(Vec::new()))
}

fn ensure_cars(root: &mut Value) -> Option<&mut Vec<Value>> {
    if !matches!(root, Value::Group(_)) {
        *root = Value::Group(Vec::new());
    }
    let items = match root {
        Value::Group(items) => items,
        _ => return None,
    };
    if !items.iter().any(|(key, _)| key == KEY_CARS) {
        items.push((KEY_CARS.to_string(), Value::List(Vec::new())));
    }
    let value = items
        .iter_mut()
        .rev()
        .find(|(key, _)| key == KEY_CARS)
        .map(|(_, value)| value)?;
    match value {
        Value::List(items) | Value::Array(items) => Some(items),
        _ => None,
    }
}

fn diameter_entry(car: &[u8], sim: &Telemetry) -> Value {
    let mut items = vec![
        (KEY_CAR.to_string(), Value::String(car_text(car))),
        (KEY_SIM.to_string(), Value::Int(sim.simexe() as i64)),
    ];
    for (index, key) in TYRE_KEYS.iter().enumerate() {
        items.push(((*key).to_string(), Value::Float(sim.tyre_diameter(index))));
    }
    Value::Group(items)
}

fn car_text(car: &[u8]) -> String {
    String::from_utf8_lossy(car).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargopit_config::names::EFFECT_ENGINE;

    const SAMPLE_SIMEXE: u64 = 690_790;
    const SAMPLE_CAR: &[u8] = b"mx5";
    const SAVED_DIAMETER: f64 = 0.62;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cargopit-tyres-{}-{}", std::process::id(), name))
    }

    fn coasting() -> Telemetry {
        let mut sim = Telemetry::new();
        sim.set_simexe(SAMPLE_SIMEXE);
        sim.set_velocity(100);
        for index in 0..WHEEL_COUNT {
            sim.set_tyre_rps(index, 10.0);
        }
        sim
    }

    #[test]
    fn effects_and_sim_flags_match_the_c_gate() {
        assert!(device_needs_tyre_diameter(EFFECT_TYRE_LOCK));
        assert!(device_needs_tyre_diameter(EFFECT_TYRE_SLIP));
        assert!(device_needs_tyre_diameter(EFFECT_ABS));
        assert!(!device_needs_tyre_diameter(EFFECT_ENGINE));
        let need = TyreSimNeed {
            calculates_diameter: false,
            supports_haptics: true,
            calculates_slip: false,
            use_config: USECONFIG_PLAY,
            has_config_path: true,
        };
        assert!(sim_needs_tyre_diameter(&need));
        assert!(!sim_needs_tyre_diameter(&TyreSimNeed {
            calculates_diameter: true,
            ..need
        }));
        assert!(!sim_needs_tyre_diameter(&TyreSimNeed {
            calculates_slip: true,
            ..need
        }));
        assert!(!sim_needs_tyre_diameter(&TyreSimNeed {
            supports_haptics: false,
            ..need
        }));
        assert!(!sim_needs_tyre_diameter(&TyreSimNeed {
            use_config: false,
            ..need
        }));
        assert!(!config_path_present(Path::new("")));
    }

    #[test]
    fn load_applies_a_case_insensitive_car_and_save_appends() {
        let path = temp_path("roundtrip.config");
        let _ = std::fs::remove_file(&path);
        let mut sim = Telemetry::new();
        sim.set_simexe(SAMPLE_SIMEXE);
        assert_eq!(
            load_tyre_config(&mut sim, SAMPLE_CAR, &path, true),
            LOAD_UNAVAILABLE
        );
        for index in 0..WHEEL_COUNT {
            sim.set_tyre_diameter(index, SAVED_DIAMETER);
        }
        assert!(save_tyre_config(&sim, SAMPLE_CAR, &path));
        let mut loaded = Telemetry::new();
        loaded.set_simexe(SAMPLE_SIMEXE);
        assert_eq!(load_tyre_config(&mut loaded, b"MX5", &path, true), 0);
        assert!((loaded.tyre_diameter(0) - SAVED_DIAMETER).abs() < f64::EPSILON);
        loaded.set_simexe(SAMPLE_SIMEXE + 1);
        assert_eq!(
            load_tyre_config(&mut loaded, SAMPLE_CAR, &path, false),
            LOAD_NOT_FOUND
        );
        sim.set_simexe(SAMPLE_SIMEXE);
        assert!(save_tyre_config(&sim, SAMPLE_CAR, &path));
        loaded.set_simexe(SAMPLE_SIMEXE);
        assert_eq!(load_tyre_config(&mut loaded, SAMPLE_CAR, &path, false), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn check_loads_saved_diameters_without_measuring() {
        let path = temp_path("saved.config");
        let _ = std::fs::remove_file(&path);
        let mut stored = Telemetry::new();
        stored.set_simexe(SAMPLE_SIMEXE);
        for index in 0..WHEEL_COUNT {
            stored.set_tyre_diameter(index, SAVED_DIAMETER);
        }
        assert!(save_tyre_config(&stored, SAMPLE_CAR, &path));
        let mut sim = coasting();
        for index in 0..WHEEL_COUNT {
            sim.set_tyre_rps(index, 0.0);
        }
        let result = check_tyres(
            &mut sim,
            &TyreCheckRequest {
                car: SAMPLE_CAR,
                path: &path,
                config_checked: false,
            },
        );
        assert!(result.config_checked);
        assert!(result.updated);
        assert!(result.stop_timer);
        assert!((sim.tyre_diameter(0) - SAVED_DIAMETER).abs() < f64::EPSILON);
        let text = std::fs::read_to_string(&path).expect("saved");
        assert_eq!(text.matches("car = \"mx5\"").count(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn check_measures_and_appends_when_the_car_is_missing() {
        let path = temp_path("measure.config");
        let _ = std::fs::remove_file(&path);
        let mut other = Telemetry::new();
        other.set_simexe(SAMPLE_SIMEXE);
        for index in 0..WHEEL_COUNT {
            other.set_tyre_diameter(index, SAVED_DIAMETER);
        }
        assert!(save_tyre_config(&other, b"other", &path));
        let mut sim = coasting();
        let result = check_tyres(
            &mut sim,
            &TyreCheckRequest {
                car: SAMPLE_CAR,
                path: &path,
                config_checked: false,
            },
        );
        assert!(result.updated);
        assert!(result.stop_timer);
        assert!(has_tyre_diameter(&sim));
        let text = std::fs::read_to_string(&path).expect("saved");
        assert!(text.contains("car = \"mx5\""));
        assert!(text.contains("car = \"other\""));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_does_not_save_after_a_measurement() {
        let path = temp_path("missing.config");
        let _ = std::fs::remove_file(&path);
        let mut sim = coasting();
        let result = check_tyres(
            &mut sim,
            &TyreCheckRequest {
                car: SAMPLE_CAR,
                path: &path,
                config_checked: false,
            },
        );
        assert!(result.updated);
        assert!(result.stop_timer);
        assert!(has_tyre_diameter(&sim));
        assert!(!path.exists());
    }

    #[test]
    fn empty_car_does_not_mark_the_config_checked() {
        let path = temp_path("empty-car.config");
        let _ = std::fs::remove_file(&path);
        let mut sim = Telemetry::new();
        for index in 0..WHEEL_COUNT {
            sim.set_tyre_diameter(index, SAVED_DIAMETER);
        }
        let result = check_tyres(
            &mut sim,
            &TyreCheckRequest {
                car: b"",
                path: &path,
                config_checked: false,
            },
        );
        assert!(!result.config_checked);
        assert!(result.stop_timer);
        assert!(!result.updated);
        let _ = std::fs::remove_file(&path);
    }
}
