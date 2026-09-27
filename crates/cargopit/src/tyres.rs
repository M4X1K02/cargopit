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

use crate::games;
use crate::log::Level;

pub const LOAD_NOT_FOUND: i32 = -1;
pub const LOAD_UNAVAILABLE: i32 = 1;
pub const USECONFIG_PLAY: bool = true;

const LOAD_OK: i32 = 0;

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
    pub notices: Vec<(Level, String)>,
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

pub fn load_tyre_config(
    sim: &mut Telemetry,
    car: &[u8],
    path: &Path,
    set_diameters: bool,
) -> (i32, Vec<(Level, String)>) {
    let mut notices = Vec::new();
    let code = load_tyre_config_into(sim, car, path, set_diameters, &mut notices);
    (code, notices)
}

pub fn save_tyre_config(sim: &Telemetry, car: &[u8], path: &Path) -> (bool, Vec<(Level, String)>) {
    let mut notices = Vec::new();
    let wrote = save_tyre_config_into(sim, car, path, &mut notices);
    (wrote, notices)
}

pub fn check_tyres(sim: &mut Telemetry, request: &TyreCheckRequest<'_>) -> TyreCheck {
    let mut notices = Vec::new();
    let mut checked = request.config_checked;
    let mut updated = false;
    if !request.car.is_empty() {
        notices.push((Level::Info, games::car_is_message(&car_text(request.car))));
        updated |= load_if_needed(sim, request, &mut checked, &mut notices);
        updated |= measure_if_needed(sim, &mut notices);
    }
    let stop_timer = save_if_ready(sim, request, &mut notices);
    TyreCheck {
        config_checked: checked,
        stop_timer,
        updated,
        notices,
    }
}

fn load_if_needed(
    sim: &mut Telemetry,
    request: &TyreCheckRequest<'_>,
    checked: &mut bool,
    notices: &mut Vec<(Level, String)>,
) -> bool {
    if diameter_probe(sim, notices) || *checked {
        return false;
    }
    notices.push((Level::Info, games::MSG_TYRE_LOAD_ATTEMPT.to_string()));
    let code = load_tyre_config_into(sim, request.car, request.path, true, notices);
    if code != LOAD_OK {
        notices.push((Level::Warn, games::MSG_TYRE_LOAD_FAILED.to_string()));
    }
    *checked = true;
    has_tyre_diameter(sim)
}

fn measure_if_needed(sim: &mut Telemetry, notices: &mut Vec<(Level, String)>) -> bool {
    if diameter_probe(sim, notices) {
        return false;
    }
    notices.push((Level::Trace, games::MSG_TYRE_CALCULATE.to_string()));
    if !measure_tyre_diameter(sim) {
        return false;
    }
    notices.push((Level::Info, games::MSG_TYRE_MEASURED.to_string()));
    true
}

fn save_if_ready(
    sim: &mut Telemetry,
    request: &TyreCheckRequest<'_>,
    notices: &mut Vec<(Level, String)>,
) -> bool {
    if !diameter_probe(sim, notices) {
        return false;
    }
    if load_tyre_config_into(sim, request.car, request.path, false, notices) < LOAD_OK {
        notices.push((
            Level::Info,
            games::tyre_saving_message(&car_text(request.car)),
        ));
        save_tyre_config_into(sim, request.car, request.path, notices);
    }
    true
}

fn diameter_probe(sim: &Telemetry, notices: &mut Vec<(Level, String)>) -> bool {
    if has_tyre_diameter(sim) {
        notices.push((Level::Trace, games::MSG_TYRE_FOUND.to_string()));
        return true;
    }
    notices.push((Level::Trace, games::MSG_TYRE_MISSING.to_string()));
    false
}

fn load_tyre_config_into(
    sim: &mut Telemetry,
    car: &[u8],
    path: &Path,
    set_diameters: bool,
    notices: &mut Vec<(Level, String)>,
) -> i32 {
    let Ok(text) = std::fs::read_to_string(path) else {
        notices.push((Level::Warn, games::MSG_TYRE_FILE_OPEN.to_string()));
        return LOAD_UNAVAILABLE;
    };
    let Ok(root) = parse(&text) else {
        notices.push((Level::Warn, games::MSG_TYRE_FILE_OPEN.to_string()));
        return LOAD_UNAVAILABLE;
    };
    let Some(cars) = root.lookup(KEY_CARS).and_then(Value::as_list) else {
        notices.push((Level::Debug, games::MSG_TYRE_FILE_CORRUPT.to_string()));
        return LOAD_UNAVAILABLE;
    };
    notices.push((Level::Trace, games::MSG_TYRE_PARSING.to_string()));
    find_car(sim, car, cars, set_diameters, notices)
}

fn save_tyre_config_into(
    sim: &Telemetry,
    car: &[u8],
    path: &Path,
    notices: &mut Vec<(Level, String)>,
) -> bool {
    let parsed = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| parse(&text).ok());
    let mut root = match parsed {
        Some(root) => root,
        None => {
            notices.push((Level::Warn, games::MSG_TYRE_FILE_CREATE.to_string()));
            Value::Group(Vec::new())
        }
    };
    let Some(cars) = ensure_cars(&mut root) else {
        return false;
    };
    cars.push(diameter_entry(car, sim));
    let wrote = std::fs::write(path, render(&root)).is_ok();
    if !wrote {
        notices.push((Level::Info, games::MSG_TYRE_WRITE_ERROR.to_string()));
    }
    notices.push((
        Level::Info,
        games::tyre_saved_message(
            &path.display().to_string(),
            slog_simexe(sim.simexe()),
            &car_text(car),
        ),
    ));
    wrote
}

fn find_car(
    sim: &mut Telemetry,
    car: &[u8],
    cars: &[Value],
    set_diameters: bool,
    notices: &mut Vec<(Level, String)>,
) -> i32 {
    for (index, entry) in cars.iter().enumerate() {
        if !log_and_match(entry, car, sim.simexe(), notices) {
            continue;
        }
        notices.push((
            Level::Info,
            games::tyre_found_car_message(
                entry.lookup(KEY_CAR).and_then(Value::as_str).unwrap_or(""),
                &logged_diameters(entry),
            ),
        ));
        if set_diameters {
            apply_diameters(sim, entry);
        }
        return i32::try_from(index).unwrap_or(LOAD_NOT_FOUND);
    }
    LOAD_NOT_FOUND
}

fn log_and_match(
    entry: &Value,
    car: &[u8],
    simexe: u64,
    notices: &mut Vec<(Level, String)>,
) -> bool {
    let Some(saved) = entry.lookup(KEY_CAR).and_then(Value::as_str) else {
        return false;
    };
    if car.is_empty() || saved.is_empty() {
        return false;
    }
    let stored = entry.lookup(KEY_SIM).map(stored_sim).unwrap_or(0);
    notices.push((
        Level::Trace,
        games::tyre_compare_message(&car_text(car), saved, slog_simexe(simexe), stored),
    ));
    car.eq_ignore_ascii_case(saved.as_bytes()) && stored as u64 == simexe
}

fn logged_diameters(entry: &Value) -> [f64; WHEEL_COUNT] {
    let mut values = [0.0; WHEEL_COUNT];
    for (index, key) in TYRE_KEYS.iter().enumerate() {
        values[index] = entry.lookup(key).and_then(Value::strict_f64).unwrap_or(0.0);
    }
    values
}

fn slog_simexe(simexe: u64) -> i32 {
    simexe as i32
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
    use crate::games;
    use crate::log::Level;
    use cargopit_config::names::EFFECT_ENGINE;

    const SAMPLE_SIMEXE: u64 = 690_790;
    const SAMPLE_CAR: &[u8] = b"mx5";
    const SAMPLE_CAR_TEXT: &str = "mx5";
    const OTHER_CAR: &[u8] = b"other";
    const OTHER_CAR_TEXT: &str = "other";
    const SAVED_DIAMETER: f64 = 0.62;
    const LOAD_INDEX_FIRST: i32 = 0;

    fn load_code(sim: &mut Telemetry, car: &[u8], path: &Path, set_diameters: bool) -> i32 {
        load_tyre_config(sim, car, path, set_diameters).0
    }

    fn save_ok(sim: &Telemetry, car: &[u8], path: &Path) -> bool {
        save_tyre_config(sim, car, path).0
    }

    fn notice_texts(notices: &[(Level, String)]) -> Vec<String> {
        notices.iter().map(|(_, message)| message.clone()).collect()
    }

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
            load_code(&mut sim, SAMPLE_CAR, &path, true),
            LOAD_UNAVAILABLE
        );
        for index in 0..WHEEL_COUNT {
            sim.set_tyre_diameter(index, SAVED_DIAMETER);
        }
        assert!(save_ok(&sim, SAMPLE_CAR, &path));
        let mut loaded = Telemetry::new();
        loaded.set_simexe(SAMPLE_SIMEXE);
        assert_eq!(
            load_code(&mut loaded, b"MX5", &path, true),
            LOAD_INDEX_FIRST
        );
        assert!((loaded.tyre_diameter(0) - SAVED_DIAMETER).abs() < f64::EPSILON);
        loaded.set_simexe(SAMPLE_SIMEXE + 1);
        assert_eq!(
            load_code(&mut loaded, SAMPLE_CAR, &path, false),
            LOAD_NOT_FOUND
        );
        sim.set_simexe(SAMPLE_SIMEXE);
        assert!(save_ok(&sim, SAMPLE_CAR, &path));
        loaded.set_simexe(SAMPLE_SIMEXE);
        assert_eq!(
            load_code(&mut loaded, SAMPLE_CAR, &path, false),
            LOAD_INDEX_FIRST
        );
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
        assert!(save_ok(&stored, SAMPLE_CAR, &path));
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
        assert!(save_ok(&other, OTHER_CAR, &path));
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

    fn sample_sim_i32() -> i32 {
        i32::try_from(SAMPLE_SIMEXE).unwrap_or(i32::MAX)
    }

    fn saved_diameters() -> [f64; WHEEL_COUNT] {
        [SAVED_DIAMETER; WHEEL_COUNT]
    }

    #[test]
    fn check_logs_the_c_load_path_when_the_car_is_saved() {
        let path = temp_path("slog-saved.config");
        let _ = std::fs::remove_file(&path);
        let mut stored = Telemetry::new();
        stored.set_simexe(SAMPLE_SIMEXE);
        for index in 0..WHEEL_COUNT {
            stored.set_tyre_diameter(index, SAVED_DIAMETER);
        }
        assert!(save_ok(&stored, SAMPLE_CAR, &path));
        let mut sim = Telemetry::new();
        sim.set_simexe(SAMPLE_SIMEXE);
        let result = check_tyres(
            &mut sim,
            &TyreCheckRequest {
                car: SAMPLE_CAR,
                path: &path,
                config_checked: false,
            },
        );
        let sim_i = sample_sim_i32();
        let compare = games::tyre_compare_message(SAMPLE_CAR_TEXT, SAMPLE_CAR_TEXT, sim_i, sim_i);
        let found = games::tyre_found_car_message(SAMPLE_CAR_TEXT, &saved_diameters());
        assert_eq!(
            notice_texts(&result.notices),
            vec![
                games::car_is_message(SAMPLE_CAR_TEXT),
                games::MSG_TYRE_MISSING.to_string(),
                games::MSG_TYRE_LOAD_ATTEMPT.to_string(),
                games::MSG_TYRE_PARSING.to_string(),
                compare,
                found,
                games::MSG_TYRE_FOUND.to_string(),
                games::MSG_TYRE_FOUND.to_string(),
                games::MSG_TYRE_PARSING.to_string(),
                games::tyre_compare_message(SAMPLE_CAR_TEXT, SAMPLE_CAR_TEXT, sim_i, sim_i),
                games::tyre_found_car_message(SAMPLE_CAR_TEXT, &saved_diameters()),
            ]
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn check_logs_the_c_measure_and_save_path() {
        let path = temp_path("slog-measure.config");
        let _ = std::fs::remove_file(&path);
        let mut other = Telemetry::new();
        other.set_simexe(SAMPLE_SIMEXE);
        for index in 0..WHEEL_COUNT {
            other.set_tyre_diameter(index, SAVED_DIAMETER);
        }
        assert!(save_ok(&other, OTHER_CAR, &path));
        let mut sim = coasting();
        let result = check_tyres(
            &mut sim,
            &TyreCheckRequest {
                car: SAMPLE_CAR,
                path: &path,
                config_checked: false,
            },
        );
        let sim_i = sample_sim_i32();
        let compare = games::tyre_compare_message(SAMPLE_CAR_TEXT, OTHER_CAR_TEXT, sim_i, sim_i);
        let texts = notice_texts(&result.notices);
        assert_eq!(texts[0], games::car_is_message(SAMPLE_CAR_TEXT));
        assert_eq!(texts[1], games::MSG_TYRE_MISSING);
        assert_eq!(texts[2], games::MSG_TYRE_LOAD_ATTEMPT);
        assert_eq!(texts[3], games::MSG_TYRE_PARSING);
        assert_eq!(texts[4], compare);
        assert_eq!(texts[5], games::MSG_TYRE_LOAD_FAILED);
        assert_eq!(texts[6], games::MSG_TYRE_MISSING);
        assert_eq!(texts[7], games::MSG_TYRE_CALCULATE);
        assert_eq!(texts[8], games::MSG_TYRE_MEASURED);
        assert_eq!(texts[9], games::MSG_TYRE_FOUND);
        assert_eq!(texts[10], games::MSG_TYRE_PARSING);
        assert_eq!(texts[11], compare);
        assert_eq!(texts[12], games::tyre_saving_message(SAMPLE_CAR_TEXT));
        assert_eq!(
            texts[13],
            games::tyre_saved_message(&path.display().to_string(), sim_i, SAMPLE_CAR_TEXT)
        );
        assert_eq!(texts.len(), 14);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_logs_open_failures_without_saving() {
        let path = temp_path("slog-missing.config");
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
        assert_eq!(
            notice_texts(&result.notices),
            vec![
                games::car_is_message(SAMPLE_CAR_TEXT),
                games::MSG_TYRE_MISSING.to_string(),
                games::MSG_TYRE_LOAD_ATTEMPT.to_string(),
                games::MSG_TYRE_FILE_OPEN.to_string(),
                games::MSG_TYRE_LOAD_FAILED.to_string(),
                games::MSG_TYRE_MISSING.to_string(),
                games::MSG_TYRE_CALCULATE.to_string(),
                games::MSG_TYRE_MEASURED.to_string(),
                games::MSG_TYRE_FOUND.to_string(),
                games::MSG_TYRE_FILE_OPEN.to_string(),
            ]
        );
        assert!(!path.exists());
    }

    #[test]
    fn empty_car_still_probes_for_saved_diameters() {
        let path = temp_path("slog-empty.config");
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
        assert_eq!(
            notice_texts(&result.notices),
            vec![
                games::MSG_TYRE_FOUND.to_string(),
                games::MSG_TYRE_FILE_OPEN.to_string(),
            ]
        );
        let _ = std::fs::remove_file(&path);
    }
}
