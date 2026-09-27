//! Shared cargopit and simd configuration.

pub mod config;
pub mod keys;
pub mod libconfig;
pub mod names;
pub mod paths;
pub mod simd_config;
pub mod tach;

pub use config::{CargopitConfig, DeviceClass, DeviceEntry, SimProfile};
pub use libconfig::{parse, render, Value};
pub use simd_config::SimdConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_numeric_lookups_do_not_autoconvert() {
        let root = parse("volume = 70; threshold = 0.2;").expect("parse");
        let volume = root.lookup("volume").expect("volume");
        let threshold = root.lookup("threshold").expect("threshold");
        assert_eq!(volume.strict_i64(), Some(70));
        assert_eq!(volume.strict_f64(), None);
        assert_eq!(threshold.strict_f64(), Some(0.2));
        assert_eq!(threshold.strict_i64(), None);
        assert_eq!(volume.as_f64(), Some(70.0));
    }

    #[test]
    fn hardware_aliases_and_unknown_tyre() {
        assert_eq!(
            names::lookup(names::HARDWARE, "MozaR8"),
            Some(names::HARDWARE_MOZA_R5)
        );
        assert_eq!(
            names::lookup(names::HARDWARE, "MozaR3"),
            Some(names::HARDWARE_MOZA_R5)
        );
        assert_eq!(names::lookup(names::HARDWARE, "not-a-real-subtype"), None);
        assert_eq!(
            names::lookup(names::EFFECTS, "Engine"),
            Some(names::EFFECT_ENGINE)
        );
        assert_eq!(
            names::map_device("USB", "Tachometer"),
            Ok(names::SUBTYPE_TACHOMETER)
        );
        assert_eq!(
            names::map_device("USB", "not-a-real-subtype"),
            Err(names::ERROR_INVALID_DEV)
        );
        assert_eq!(names::tyre_or_all_four("fronts"), names::TYRE_FRONTS);
        assert_eq!(names::tyre_or_all_four("missing"), names::TYRE_ALL_FOUR);
        assert_eq!(
            names::name_for(names::MODULATIONS, names::MODULATION_AMPLIFY),
            Some("Amplitude")
        );
        assert_eq!(
            names::lookup(names::MODULATIONS, "AMPLIFY"),
            Some(names::MODULATION_AMPLIFY)
        );
        const FREQ: i64 = 40;
        const FREQ_EQUAL: i64 = 40;
        const FREQ_MAX: i64 = 80;
        const FREQ_UNSET: i64 = 0;
        const INVALID_MODULATION: &str = "bogus";
        let frequency = names::name_for(names::MODULATIONS, names::MODULATION_FREQUENCY)
            .expect("frequency name");
        let amplify =
            names::name_for(names::MODULATIONS, names::MODULATION_AMPLIFY).expect("amplify name");
        let amplify_alias = "Amplify";
        assert_eq!(
            names::parse_modulation(None, FREQ_UNSET, FREQ_UNSET),
            names::ModulationParse::Missing
        );
        assert_eq!(
            names::parse_modulation(Some(INVALID_MODULATION), FREQ_UNSET, FREQ_UNSET),
            names::ModulationParse::Invalid(INVALID_MODULATION)
        );
        assert_eq!(
            names::parse_modulation(Some(frequency), FREQ, FREQ_UNSET),
            names::ModulationParse::FrequencyNeedsMax
        );
        assert_eq!(
            names::parse_modulation(Some(frequency), FREQ, FREQ_EQUAL),
            names::ModulationParse::Found(names::MODULATION_FREQUENCY)
        );
        assert_eq!(
            names::parse_modulation(Some(frequency), FREQ, FREQ_MAX),
            names::ModulationParse::Found(names::MODULATION_FREQUENCY)
        );
        assert_eq!(
            names::parse_modulation(Some(amplify_alias), FREQ_UNSET, FREQ_UNSET),
            names::ModulationParse::Found(names::MODULATION_AMPLIFY)
        );
        assert_eq!(
            names::modulation_value(Some(frequency), FREQ, FREQ_UNSET),
            names::MODULATION_NONE
        );
        assert_eq!(
            names::modulation_value(Some(frequency), FREQ, FREQ_MAX),
            names::MODULATION_FREQUENCY
        );
        assert_eq!(names::modulation_label(names::MODULATION_AMPLIFY), amplify);
    }

    #[test]
    fn profile_index_and_fps_match_c_bounds() {
        assert_eq!(names::resolve_profile_index(2, 1), 1);
        assert_eq!(
            names::resolve_profile_index(2, keys::CONFIG_INDEX_UNSET),
            keys::CONFIG_INDEX_FIRST
        );
        assert_eq!(
            names::resolve_profile_index(2, 99),
            keys::CONFIG_INDEX_UNSET
        );
        assert_eq!(names::clamp_fps(0), keys::FPS_MIN);
        assert_eq!(names::clamp_fps(-5), keys::FPS_MIN);
        assert_eq!(names::clamp_fps(keys::FPS_DEFAULT), keys::FPS_DEFAULT);
        assert_eq!(names::clamp_fps(keys::FPS_MAX + 1), keys::FPS_MAX);
    }

    #[test]
    fn revburner_sample_xml_keeps_rpm_and_pulses() {
        let path = paths::source_root()
            .join(keys::CONF_DIRNAME)
            .join(keys::REVBURNER_XML_NAME);
        let src = std::fs::read_to_string(path).expect("sample xml");
        let points = tach::parse_tach_xml(&src).expect("tach");
        assert_eq!(points[0].rpm, 300);
        assert_eq!(points[0].pulses, 43600);
        assert!(points.len() > 1);
    }
}
