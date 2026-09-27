pub mod scenarios;

use std::fs;
use std::path::{Path, PathBuf};

use cargopit_devices::serial::{capture_serial, SERIAL_DEVICES};
use cargopit_devices::sound::{capture_sound, SOUND_DEVICES};
use cargopit_devices::telemetry::Telemetry;
use cargopit_devices::usb::{capture_usb, USB_DEVICES};

const GOLDEN_REL: &str = "../../tests/parity/golden";
const LUA_REL: &str = "../../tests/parity/fixtures/leds.lua";

pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GOLDEN_REL)
}

pub fn lua_fixture() -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(LUA_REL)).expect("leds.lua")
}

fn frames_of(scenario: &scenarios::Scenario) -> Vec<Telemetry> {
    scenario
        .frames
        .iter()
        .map(|frame| Telemetry::from_buf(frame.clone()))
        .collect()
}

fn write_one(dir: &Path, device: &str, scenario: &str, body: String) -> Result<(), String> {
    let path = dir.join(format!("{device}__{scenario}.golden"));
    fs::write(&path, body).map_err(|err| format!("{}: {err}", path.display()))
}

pub fn write_goldens(dir: &Path) -> Result<usize, String> {
    let lua = lua_fixture();
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let mut count = 0usize;
    for scenario in scenarios::all() {
        let frames = frames_of(&scenario);
        for name in USB_DEVICES {
            let got = capture_usb(name, &frames, &lua)
                .ok_or_else(|| format!("usb capture failed for {name}"))?;
            write_one(dir, name, scenario.name, got)?;
            count += 1;
        }
        for name in SERIAL_DEVICES {
            let got = capture_serial(name, &frames, &lua)
                .ok_or_else(|| format!("serial capture failed for {name}"))?;
            write_one(dir, name, scenario.name, got)?;
            count += 1;
        }
        for name in SOUND_DEVICES {
            let got = capture_sound(name, &frames)
                .ok_or_else(|| format!("sound capture failed for {name}"))?;
            write_one(dir, name, scenario.name, got)?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
fn assert_golden(dir: &Path, device: &str, scenario: &str, got: &str) {
    let path = dir.join(format!("{device}__{scenario}.golden"));
    let expected =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("missing {}: {err}", path.display()));
    assert_eq!(got, expected, "{}", path.display());
}

#[cfg(test)]
mod usb_goldens {
    use super::*;

    #[test]
    fn usb_captures_match_committed_goldens() {
        let lua = lua_fixture();
        let dir = golden_dir();
        for scenario in scenarios::all() {
            let frames = frames_of(&scenario);
            for name in USB_DEVICES {
                let got = capture_usb(name, &frames, &lua).expect(name);
                assert_golden(&dir, name, scenario.name, &got);
            }
        }
    }
}

#[cfg(test)]
mod serial_goldens {
    use super::*;

    #[test]
    fn serial_captures_match_committed_goldens() {
        let lua = lua_fixture();
        let dir = golden_dir();
        for scenario in scenarios::all() {
            let frames = frames_of(&scenario);
            for name in SERIAL_DEVICES {
                let got = capture_serial(name, &frames, &lua).expect(name);
                assert_golden(&dir, name, scenario.name, &got);
            }
        }
    }
}

#[cfg(test)]
mod sound_goldens {
    use super::*;

    #[test]
    fn sound_captures_match_committed_goldens() {
        let dir = golden_dir();
        for scenario in scenarios::all() {
            let frames = frames_of(&scenario);
            for name in SOUND_DEVICES {
                let got = capture_sound(name, &frames).expect(name);
                assert_golden(&dir, name, scenario.name, &got);
            }
        }
    }
}
