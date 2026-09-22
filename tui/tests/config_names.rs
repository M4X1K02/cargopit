//! Every config name the TUI can write must be one cargopit's C parser accepts.
//! The C side lists them once, in src/cargopit/helper/devicenames.h.

use std::collections::HashMap;

use cargopit_tui::consts;

const DEVICENAMES_H: &str = include_str!("../../src/cargopit/helper/devicenames.h");
const TABLE_PREFIX: &str = "#define CARGOPIT_";
const TABLE_SUFFIX: &str = "_NAMES(X)";
const ENTRY_PREFIX: &str = "X(\"";
const QUOTE: char = '"';

/// Table name (e.g. `EFFECT`) -> lowercased names listed in that X-macro table.
fn c_name_tables() -> HashMap<String, Vec<String>> {
    let mut tables: HashMap<String, Vec<String>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in DEVICENAMES_H.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix(TABLE_PREFIX) {
            current = rest.split_once(TABLE_SUFFIX).map(|(name, _)| name.to_string());
            continue;
        }
        let Some(table) = current.as_ref() else {
            continue;
        };
        let Some(rest) = line.strip_prefix(ENTRY_PREFIX) else {
            current = None;
            continue;
        };
        if let Some((name, _)) = rest.split_once(QUOTE) {
            tables.entry(table.clone()).or_default().push(name.to_lowercase());
        }
    }
    tables
}

fn assert_all_accepted(tables: &HashMap<String, Vec<String>>, table: &str, tui_names: &[&str]) {
    let accepted = tables
        .get(table)
        .unwrap_or_else(|| panic!("devicenames.h has no CARGOPIT_{table}_NAMES table"));
    for name in tui_names {
        assert!(
            accepted.contains(&name.to_lowercase()),
            "TUI writes {name:?} but CARGOPIT_{table}_NAMES in devicenames.h does not accept it"
        );
    }
}

#[test]
fn tui_device_names_are_accepted_by_c_parser() {
    let tables = c_name_tables();
    assert_all_accepted(
        &tables,
        "DEVICE_CLASS",
        &[consts::CLASS_USB, consts::CLASS_SOUND, consts::CLASS_SERIAL],
    );
    assert_all_accepted(&tables, "USB_TYPE", consts::USB_TYPES);
    assert_all_accepted(&tables, "SERIAL_TYPE", consts::SERIAL_TYPES);
    assert_all_accepted(&tables, "SOUND_TYPE", consts::SOUND_TYPES);
    assert_all_accepted(&tables, "HARDWARE", consts::USB_HARDWARE_SUBTYPES);
    assert_all_accepted(&tables, "HARDWARE", consts::TACHOMETER_SUBTYPES);
    assert_all_accepted(&tables, "HARDWARE", consts::SERIAL_WHEEL_SUBTYPES);
}

#[test]
fn tui_effect_names_are_accepted_by_c_parser() {
    let tables = c_name_tables();
    assert_all_accepted(&tables, "EFFECT", consts::EFFECTS);
    assert_all_accepted(&tables, "TYRE", consts::TYRES);
    assert_all_accepted(&tables, "MODULATION", consts::MODULATIONS);
    assert_all_accepted(&tables, "MODULATION", &[consts::MODULATION_FREQUENCY_ALT]);
}
