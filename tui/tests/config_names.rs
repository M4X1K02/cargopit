//! Every config name the TUI can write must be one the shared parser accepts.

use cargopit_config::names::{self, NameEntry};
use cargopit_tui::consts;

fn accepted(table: &[NameEntry]) -> Vec<String> {
    names::names(table)
        .into_iter()
        .map(|name| name.to_lowercase())
        .collect()
}

fn assert_all_accepted(table: &[NameEntry], table_name: &str, tui_names: &[&str]) {
    let accepted = accepted(table);
    for name in tui_names {
        assert!(
            accepted.contains(&name.to_lowercase()),
            "TUI writes {name:?} but {table_name} does not accept it"
        );
    }
}

#[test]
fn tui_device_names_are_accepted_by_parser() {
    assert_all_accepted(
        names::DEVICE_CLASSES,
        "DEVICE_CLASSES",
        &[consts::CLASS_USB, consts::CLASS_SOUND, consts::CLASS_SERIAL],
    );
    assert_all_accepted(names::USB_TYPES, "USB_TYPES", consts::USB_TYPES);
    assert_all_accepted(names::SERIAL_TYPES, "SERIAL_TYPES", consts::SERIAL_TYPES);
    assert_all_accepted(names::SOUND_TYPES, "SOUND_TYPES", consts::SOUND_TYPES);
    assert_all_accepted(names::HARDWARE, "HARDWARE", consts::USB_HARDWARE_SUBTYPES);
    assert_all_accepted(names::HARDWARE, "HARDWARE", consts::TACHOMETER_SUBTYPES);
    assert_all_accepted(names::HARDWARE, "HARDWARE", consts::SERIAL_WHEEL_SUBTYPES);
}

#[test]
fn tui_effect_names_are_accepted_by_parser() {
    assert_all_accepted(names::EFFECTS, "EFFECTS", consts::EFFECTS);
    assert_all_accepted(names::TYRES, "TYRES", consts::TYRES);
    assert_all_accepted(names::MODULATIONS, "MODULATIONS", consts::MODULATIONS);
    assert_all_accepted(
        names::MODULATIONS,
        "MODULATIONS",
        &[consts::MODULATION_FREQUENCY_ALT],
    );
}
