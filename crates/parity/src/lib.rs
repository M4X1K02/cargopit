pub mod scenarios;

#[cfg(test)]
mod usb_goldens {
    use std::fs;
    use std::path::PathBuf;

    use cargopit_devices::telemetry::Telemetry;
    use cargopit_devices::usb::{capture_usb, USB_DEVICES};

    use crate::scenarios;

    fn golden_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity/golden")
    }

    fn lua_fixture() -> String {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity/fixtures/leds.lua");
        fs::read_to_string(path).expect("leds.lua")
    }

    #[test]
    fn usb_captures_match_c_goldens() {
        let lua = lua_fixture();
        let dir = golden_dir();
        for scenario in scenarios::all() {
            let frames: Vec<Telemetry> = scenario
                .frames
                .iter()
                .map(|frame| Telemetry::from_buf(frame.clone()))
                .collect();
            for name in USB_DEVICES {
                let got = capture_usb(name, &frames, &lua).expect(name);
                let path = dir.join(format!("{name}__{}.golden", scenario.name));
                let expected = fs::read_to_string(&path)
                    .unwrap_or_else(|err| panic!("missing {}: {err}", path.display()));
                assert_eq!(got, expected, "{}", path.display());
            }
        }
    }
}

#[cfg(test)]
mod serial_goldens {
    use std::fs;
    use std::path::PathBuf;

    use cargopit_devices::serial::{capture_serial, SERIAL_DEVICES};
    use cargopit_devices::telemetry::Telemetry;

    use crate::scenarios;

    fn golden_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity/golden")
    }

    fn lua_fixture() -> String {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity/fixtures/leds.lua");
        fs::read_to_string(path).expect("leds.lua")
    }

    #[test]
    fn serial_captures_match_c_goldens() {
        let lua = lua_fixture();
        let dir = golden_dir();
        for scenario in scenarios::all() {
            let frames: Vec<Telemetry> = scenario
                .frames
                .iter()
                .map(|frame| Telemetry::from_buf(frame.clone()))
                .collect();
            for name in SERIAL_DEVICES {
                let got = capture_serial(name, &frames, &lua).expect(name);
                let path = dir.join(format!("{name}__{}.golden", scenario.name));
                let expected = fs::read_to_string(&path)
                    .unwrap_or_else(|err| panic!("missing {}: {err}", path.display()));
                assert_eq!(got, expected, "{}", path.display());
            }
        }
    }
}

#[cfg(test)]
mod sound_goldens {
    use std::fs;
    use std::path::PathBuf;

    use cargopit_devices::sound::{capture_sound, SOUND_DEVICES};
    use cargopit_devices::telemetry::Telemetry;

    use crate::scenarios;

    fn golden_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity/golden")
    }

    #[test]
    fn sound_captures_match_c_goldens() {
        let dir = golden_dir();
        for scenario in scenarios::all() {
            let frames: Vec<Telemetry> = scenario
                .frames
                .iter()
                .map(|frame| Telemetry::from_buf(frame.clone()))
                .collect();
            for name in SOUND_DEVICES {
                let got = capture_sound(name, &frames).expect(name);
                let path = dir.join(format!("{name}__{}.golden", scenario.name));
                let expected = fs::read_to_string(&path)
                    .unwrap_or_else(|err| panic!("missing {}: {err}", path.display()));
                assert_eq!(got, expected, "{}", path.display());
            }
        }
    }
}
