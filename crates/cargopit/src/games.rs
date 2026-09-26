//! Game-path decisions that stay in the host: UDP ports, stale simd, ACR bridge.

use std::path::{Path, PathBuf};

use crate::acr;

pub const DAEMON_PROBE_US: u64 = 50_000;
pub const DR2_GAME_PORT: u16 = 20779;
pub const DR2_BIND_PORT: u16 = 20777;
pub const STATE_SEARCHING: &str = "searching";
pub const STATE_MAPPING: &str = "mapping";
pub const STATE_EXITING: &str = "exiting";
pub const MAP_API_SIMD: i32 = 0;
pub const STATUS_MENU: i32 = 1;
pub const STATUS_ACTIVE_PLAY: i32 = 2;
pub const CHECK_INTERVAL_MS: u64 = 1000;
pub const SIM_NOT_DETECTED: &str = "None Detected";
pub const QUIT_KEY: u8 = b'q';
pub const MSG_USER_STOP: &str = "User requested stop, releasing devices";
pub const MSG_STOPPED_MAPPING: &str = "stopped mapping data, press q again to quit";
pub const MSG_SEARCHING: &str = "Searching for sim data... Press q to quit...";
pub const MSG_EXITING: &str = "Cargopit is exiting...";
pub const MSG_RELEASE_LOOP: &str = "release loop";
pub const MSG_RELEASING_DEVICES: &str = "releasing devices, please wait";
pub const MSG_RESTART_CHECK: &str = "restarting checking for data...";
pub const MSG_RELOAD: &str = "reload requested, releasing devices to load the saved profile";
pub const MSG_APPLYING_SETTINGS: &str = "applying settings";
pub const MSG_SETTINGS_APPLIED: &str = "settings applied";
pub const MSG_CHECKING_DIAMETERS: &str = "checking for diameters config";
pub const MSG_OPENED_CONFIG: &str = "Opened and validated cargopit configuration file";
pub const MSG_GAMELOOP_MODE: &str = "running cargopit in gameloop mode..";
pub const MSG_TEST_MODE_BANNER: &str = "running cargopit in test mode...";
pub const MSG_TEST_INDEX: &str = "Could not resolve config index for test";
pub const ERROR_NONE: i32 = 0;
pub const ERROR_UNKNOWN: i32 = 1;
pub const ERROR_INVALID_DEV: i32 = 3;
pub const ERROR_SIMD_REQUIRED: i32 = 7;
pub const CONFIG_CHECK_START: i32 = 0;
pub const CONFIG_IO_FILE: &str = "(null)";
pub const CONFIG_IO_LINE: i32 = 0;
pub const CONFIG_IO_TEXT: &str = "file I/O error";
pub const CONFIG_SYNTAX_TEXT: &str = "syntax error";
pub const MSG_PULSE_CONNECTING: &str = "connecting pulseaudio...";
pub const MSG_PULSE_CONNECTED: &str = "successfully connected pulseaudio...";
pub const MSG_PULSE_CONNECT_FAILED: &str = "pulseaudio connect failed";
pub const MSG_PULSE_CONTEXT_FREED: &str = "freed pulseaudio context";
pub const MSG_PARSING_CONFIG: &str = "Parsing config file";
pub const MSG_SKIP_AUDIO: &str =
    "skipping configured sound device due to disable_audio being specified...";
pub const SIMULATOR_API_NONE: i32 = 0;
pub const SIMULATOR_API_ASSETTO_CORSA: i32 = 1;
pub const MAPPING_START_MS: u64 = 2000;
const MS_PER_SECOND: f64 = 1000.0;
const HALF_MS: f64 = 0.5;
const MIN_MAP_INTERVAL_MS: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetrySource {
    Shm,
    Udp,
    Auto,
}

pub const UDP_BIND_ADDRESS: &str = "0.0.0.0";
pub const UDP_RECV_BYTES: usize = 65536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketRoute {
    Ignore,
    Acr,
    Datamap,
}

pub fn route_udp(packet: &[u8]) -> PacketRoute {
    if packet.is_empty() {
        return PacketRoute::Ignore;
    }
    if acr::packet_ok(packet) {
        return PacketRoute::Acr;
    }
    PacketRoute::Datamap
}

pub fn bind_port(requested: u16) -> u16 {
    if requested == DR2_GAME_PORT {
        return DR2_BIND_PORT;
    }
    requested
}

pub fn daemon_is_stale(before_mtick: u64, after_mtick: u64) -> bool {
    before_mtick == after_mtick
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeenSim {
    pub is_sim_on: bool,
    pub sim_status: i32,
    pub map_api: i32,
    pub simulator_api: i32,
    pub uses_udp: bool,
    pub sim_exe: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayPhase {
    Searching,
    Mapping,
    Exiting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayAction {
    Wait,
    StartMapping { use_udp: bool },
    Release,
}

pub fn simd_map_is_stale(map_api: i32, daemon_advancing: bool) -> bool {
    map_api == MAP_API_SIMD && !daemon_advancing
}

pub fn bridged_acr(seen: SeenSim) -> SeenSim {
    SeenSim {
        is_sim_on: true,
        sim_status: STATUS_ACTIVE_PLAY,
        uses_udp: true,
        simulator_api: SIMULATOR_API_ASSETTO_CORSA,
        ..seen
    }
}

pub fn sim_display_name(sim_exe: u64, exiting: bool) -> String {
    if exiting || sim_exe == 0 {
        return SIM_NOT_DETECTED.to_string();
    }
    let Ok(code) = u32::try_from(sim_exe) else {
        return SIM_NOT_DETECTED.to_string();
    };
    native_sim_name(code).unwrap_or_else(|| SIM_NOT_DETECTED.to_string())
}

fn native_sim_name(code: u32) -> Option<String> {
    let ptr = unsafe { simapi_sys::bindings::simapi_gametofullstr(code) };
    if ptr.is_null() {
        return None;
    }
    let name = unsafe { std::ffi::CStr::from_ptr(ptr) };
    let text = name.to_string_lossy();
    if text.is_empty() {
        return None;
    }
    Some(text.into_owned())
}

pub fn search_tick(seen: SeenSim, force_udp: bool, user_stopped: bool) -> PlayAction {
    if user_stopped || !seen.is_sim_on || seen.sim_status < STATUS_ACTIVE_PLAY {
        return PlayAction::Wait;
    }
    PlayAction::StartMapping {
        use_udp: force_udp || seen.uses_udp,
    }
}

pub fn map_interval_ms(fps: i32) -> u64 {
    let clamped = crate::scheduler::clamp_fps(fps) as f64;
    let interval = (MS_PER_SECOND / clamped + HALF_MS) as u64;
    if interval == 0 {
        return MIN_MAP_INTERVAL_MS;
    }
    interval
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitAction {
    Continue,
    Release,
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileLoadFault {
    NoProfiles,
    IndexOutOfRange { configs: i32 },
}

pub fn signal_stop_message(signum: i32) -> String {
    format!("signal {signum} received, stopping")
}

pub fn loading_profile_message(path: &str, config_index: i32) -> String {
    format!("loading device profile from {path} (config-index {config_index})")
}

pub fn no_profiles_message(path: &str) -> String {
    format!("no device profiles in {path}")
}

pub fn config_index_range_message(index: i32, configs: i32) -> String {
    format!("config-index {index} is out of range ({configs} configs)")
}

pub fn no_profile_message(config_index: i32) -> String {
    format!("no device profile to load (config-index {config_index})")
}

pub fn testing_config_message(path: &str) -> String {
    format!("Testing cargopit config file: {path}")
}

pub fn diameters_debug_message(path: &str, config_check: i32) -> String {
    format!("using diameters file {path} {config_check}")
}

pub fn config_issue_message(file: &str, line: i32, text: &str) -> String {
    format!("Issue with cargopit config file: {file}:{line} - {text}")
}

pub fn game_loop_exit_message(code: i32) -> String {
    format!("Game loop exited succesfully with error code: {code}")
}

pub fn game_loop_fail_message(code: i32) -> String {
    format!("Game loop exited with error code: {code}")
}

pub fn test_exit_message(code: i32) -> String {
    format!("Test exited succesfully with error code: {code}")
}

pub fn test_fail_message(code: i32) -> String {
    format!("Test exited with error code: {code}")
}

pub fn pulse_context_failed_message(state: i32) -> String {
    format!("pulseaudio context failed (state {state})")
}

pub fn loading_confignum_message(confignum: i32, devices: i32) -> String {
    format!("loading confignum {confignum}, with {devices} devices.")
}

pub fn initializing_simdevices_message(simulator_api: i32) -> String {
    format!("initializing simdevices for simapi {simulator_api}...")
}

pub fn skipping_disabled_message(index: i32) -> String {
    format!("skipping disabled device at index {index}")
}

pub fn home_config_file(name: &str) -> PathBuf {
    cargopit_config::paths::home_dir()
        .join(cargopit_config::keys::XDG_CONFIG_FALLBACK)
        .join(cargopit_config::keys::CONFIG_DIR_NAME)
        .join(name)
}

pub fn config_path_for(config_file: Option<&Path>, config_dir: Option<&Path>) -> PathBuf {
    if let Some(path) = config_file {
        if path.is_file() {
            return path.to_path_buf();
        }
    }
    if let Some(dir) = config_dir {
        if dir.is_dir() {
            return dir.join(cargopit_config::keys::CONFIG_FILE_NAME);
        }
    }
    home_config_file(cargopit_config::keys::CONFIG_FILE_NAME)
}

pub struct ConfigIssue {
    pub file: String,
    pub line: i32,
    pub text: String,
}

pub fn inspect_config(path: &Path) -> Result<(), ConfigIssue> {
    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(_) => {
            return Err(ConfigIssue {
                file: CONFIG_IO_FILE.to_string(),
                line: CONFIG_IO_LINE,
                text: CONFIG_IO_TEXT.to_string(),
            });
        }
    };
    if let Err(err) = cargopit_config::parse(&src) {
        let line = i32::try_from(err.line).unwrap_or(i32::MAX);
        return Err(ConfigIssue {
            file: path.display().to_string(),
            line,
            text: CONFIG_SYNTAX_TEXT.to_string(),
        });
    }
    Ok(())
}

pub fn profile_load_fault(profile_count: i32, requested: i32) -> Option<ProfileLoadFault> {
    if profile_count <= 0 {
        return Some(ProfileLoadFault::NoProfiles);
    }
    if requested < 0 {
        return None;
    }
    if requested >= profile_count {
        return Some(ProfileLoadFault::IndexOutOfRange {
            configs: profile_count,
        });
    }
    None
}

pub fn quit_action(key: Option<u8>, signalled: bool, mapping: bool) -> QuitAction {
    if signalled {
        return QuitAction::Exit;
    }
    if key != Some(QUIT_KEY) {
        return QuitAction::Continue;
    }
    if mapping {
        return QuitAction::Release;
    }
    QuitAction::Exit
}

pub fn mapping_should_stop(seen: SeenSim) -> bool {
    !seen.is_sim_on || seen.sim_status <= STATUS_MENU
}

pub fn mapping_tick(seen: SeenSim) -> PlayAction {
    if seen.map_api != MAP_API_SIMD {
        return PlayAction::Wait;
    }
    if !seen.is_sim_on {
        return PlayAction::Release;
    }
    PlayAction::Wait
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameSnapshot {
    frame: Vec<u8>,
    sequence: u64,
}

impl FrameSnapshot {
    pub fn new() -> Self {
        Self {
            frame: Vec::new(),
            sequence: 0,
        }
    }

    pub fn publish(&mut self, frame: &[u8]) {
        self.frame.clear();
        self.frame.extend_from_slice(frame);
        self.sequence = self.sequence.wrapping_add(1);
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn frame(&self) -> &[u8] {
        &self.frame
    }
}

impl Default for FrameSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

pub fn use_acr_bridge(simexe: u64, source: TelemetrySource, physics: Option<&[u8]>) -> bool {
    if simexe != acr::SIMEXE_ACR {
        return false;
    }
    if source == TelemetrySource::Shm {
        return false;
    }
    source == TelemetrySource::Udp || acr::physics_shm_is_blank(physics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udp_packets_follow_the_c_routes() {
        assert_eq!(route_udp(&[]), PacketRoute::Ignore);
        assert_eq!(route_udp(&[0, 1]), PacketRoute::Datamap);
        let mut acr = vec![0u8; 48];
        acr[..4].copy_from_slice(b"ACRM");
        assert_eq!(route_udp(&acr), PacketRoute::Acr);
        assert_eq!(UDP_BIND_ADDRESS, "0.0.0.0");
        assert_eq!(UDP_RECV_BYTES, 65536);
    }

    #[test]
    fn quit_key_releases_while_mapping_and_exits_while_searching() {
        assert_eq!(quit_action(None, true, true), QuitAction::Exit);
        assert_eq!(quit_action(Some(b'x'), false, true), QuitAction::Continue);
        assert_eq!(
            quit_action(Some(QUIT_KEY), false, true),
            QuitAction::Release
        );
        assert_eq!(quit_action(Some(QUIT_KEY), false, false), QuitAction::Exit);
        assert_eq!(MSG_USER_STOP, "User requested stop, releasing devices");
        assert_eq!(
            MSG_STOPPED_MAPPING,
            "stopped mapping data, press q again to quit"
        );
    }

    #[test]
    fn play_announcements_match_the_c_host() {
        const DEFAULT_INDEX: i32 = -1;
        const SAMPLE_INDEX: i32 = 3;
        const SAMPLE_CONFIGS: i32 = 1;
        const TWO_PROFILES: i32 = 2;
        const NO_PROFILES: i32 = 0;
        const SAMPLE_PATH: &str = "/tmp/cargopit.config";
        assert_eq!(
            MSG_SEARCHING,
            "Searching for sim data... Press q to quit..."
        );
        assert_eq!(MSG_EXITING, "Cargopit is exiting...");
        assert_eq!(MSG_RELEASE_LOOP, "release loop");
        assert_eq!(MSG_RELEASING_DEVICES, "releasing devices, please wait");
        assert_eq!(MSG_RESTART_CHECK, "restarting checking for data...");
        assert_eq!(
            MSG_RELOAD,
            "reload requested, releasing devices to load the saved profile"
        );
        assert_eq!(
            signal_stop_message(libc::SIGINT),
            "signal 2 received, stopping"
        );
        assert_eq!(
            signal_stop_message(libc::SIGTERM),
            "signal 15 received, stopping"
        );
        assert_eq!(
            loading_profile_message(SAMPLE_PATH, DEFAULT_INDEX),
            "loading device profile from /tmp/cargopit.config (config-index -1)"
        );
        assert_eq!(
            no_profiles_message(SAMPLE_PATH),
            "no device profiles in /tmp/cargopit.config"
        );
        assert_eq!(
            config_index_range_message(SAMPLE_INDEX, SAMPLE_CONFIGS),
            "config-index 3 is out of range (1 configs)"
        );
        assert_eq!(
            no_profile_message(DEFAULT_INDEX),
            "no device profile to load (config-index -1)"
        );
        assert_eq!(
            profile_load_fault(NO_PROFILES, DEFAULT_INDEX),
            Some(ProfileLoadFault::NoProfiles)
        );
        assert_eq!(profile_load_fault(SAMPLE_CONFIGS, DEFAULT_INDEX), None);
        assert_eq!(
            profile_load_fault(SAMPLE_CONFIGS, SAMPLE_CONFIGS),
            Some(ProfileLoadFault::IndexOutOfRange {
                configs: SAMPLE_CONFIGS
            })
        );
        assert_eq!(profile_load_fault(TWO_PROFILES, SAMPLE_CONFIGS), None);
    }

    #[test]
    fn startup_config_messages_match_the_c_host() {
        const SYNTAX_LINE: i32 = 2;
        let dir = std::env::temp_dir().join("cargopit-startup-config");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let missing = dir.join("missing.config");
        let broken = dir.join("broken.config");
        let valid = dir.join(cargopit_config::keys::CONFIG_FILE_NAME);
        let other_dir = dir.join("configs");
        std::fs::create_dir_all(&other_dir).expect("config dir");
        std::fs::write(&broken, "configs = (\n").expect("broken");
        std::fs::write(&valid, "configs = ();\n").expect("valid");

        assert_eq!(MSG_APPLYING_SETTINGS, "applying settings");
        assert_eq!(MSG_SETTINGS_APPLIED, "settings applied");
        assert_eq!(MSG_CHECKING_DIAMETERS, "checking for diameters config");
        assert_eq!(
            MSG_OPENED_CONFIG,
            "Opened and validated cargopit configuration file"
        );
        assert_eq!(MSG_GAMELOOP_MODE, "running cargopit in gameloop mode..");
        assert_eq!(MSG_TEST_MODE_BANNER, "running cargopit in test mode...");
        assert_eq!(MSG_TEST_INDEX, "Could not resolve config index for test");
        assert_eq!(
            testing_config_message("/tmp/cargopit.config"),
            "Testing cargopit config file: /tmp/cargopit.config"
        );
        assert_eq!(
            diameters_debug_message("/tmp/diameters.config", CONFIG_CHECK_START),
            "using diameters file /tmp/diameters.config 0"
        );
        assert_eq!(
            game_loop_exit_message(ERROR_NONE),
            "Game loop exited succesfully with error code: 0"
        );
        assert_eq!(
            game_loop_fail_message(ERROR_SIMD_REQUIRED),
            "Game loop exited with error code: 7"
        );
        assert_eq!(
            test_exit_message(ERROR_NONE),
            "Test exited succesfully with error code: 0"
        );
        assert_eq!(
            test_fail_message(ERROR_INVALID_DEV),
            "Test exited with error code: 3"
        );
        assert_eq!(ERROR_UNKNOWN, 1);
        assert_eq!(MSG_PULSE_CONNECTING, "connecting pulseaudio...");
        assert_eq!(MSG_PULSE_CONNECTED, "successfully connected pulseaudio...");
        assert_eq!(MSG_PULSE_CONNECT_FAILED, "pulseaudio connect failed");
        assert_eq!(MSG_PULSE_CONTEXT_FREED, "freed pulseaudio context");
        assert_eq!(
            pulse_context_failed_message(cargopit_devices::transport::PULSE_CONTEXT_FAILED),
            "pulseaudio context failed (state 5)"
        );
        assert_eq!(
            loading_confignum_message(0, 2),
            "loading confignum 0, with 2 devices."
        );
        assert_eq!(MSG_PARSING_CONFIG, "Parsing config file");
        assert_eq!(
            initializing_simdevices_message(SIMULATOR_API_ASSETTO_CORSA),
            "initializing simdevices for simapi 1..."
        );
        assert_eq!(
            skipping_disabled_message(3),
            "skipping disabled device at index 3"
        );
        assert_eq!(
            MSG_SKIP_AUDIO,
            "skipping configured sound device due to disable_audio being specified..."
        );
        assert_eq!(
            SIMULATOR_API_NONE,
            simapi_sys::bindings::SimulatorAPI_SIMULATORAPI_SIMAPI_TEST as i32
        );
        assert_eq!(
            SIMULATOR_API_ASSETTO_CORSA,
            simapi_sys::bindings::SimulatorAPI_SIMULATORAPI_ASSETTO_CORSA as i32
        );

        let missing_issue = inspect_config(&missing).expect_err("missing");
        assert_eq!(missing_issue.file, CONFIG_IO_FILE);
        assert_eq!(missing_issue.line, CONFIG_IO_LINE);
        assert_eq!(missing_issue.text, CONFIG_IO_TEXT);
        assert_eq!(
            config_issue_message(&missing_issue.file, missing_issue.line, &missing_issue.text),
            "Issue with cargopit config file: (null):0 - file I/O error"
        );

        let broken_issue = inspect_config(&broken).expect_err("syntax");
        assert_eq!(broken_issue.file, broken.display().to_string());
        assert_eq!(broken_issue.line, SYNTAX_LINE);
        assert_eq!(broken_issue.text, CONFIG_SYNTAX_TEXT);
        assert!(inspect_config(&valid).is_ok());

        assert_eq!(config_path_for(Some(&valid), None), valid);
        assert_eq!(
            config_path_for(Some(&missing), None),
            home_config_file(cargopit_config::keys::CONFIG_FILE_NAME)
        );
        assert_eq!(
            config_path_for(Some(&missing), Some(&other_dir)),
            other_dir.join(cargopit_config::keys::CONFIG_FILE_NAME)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dr2_bind_and_stale_mtick_match_the_c_host() {
        assert_eq!(bind_port(DR2_GAME_PORT), DR2_BIND_PORT);
        assert_eq!(bind_port(acr::UDP_PORT), acr::UDP_PORT);
        assert!(daemon_is_stale(10, 10));
        assert!(!daemon_is_stale(10, 11));
        assert_eq!(DAEMON_PROBE_US, 50_000);
    }

    #[test]
    fn only_blank_acr_uses_the_udp_bridge() {
        assert!(!use_acr_bridge(
            acr::SIMEXE_DIRT_RALLY_2,
            TelemetrySource::Auto,
            None
        ));
        assert!(!use_acr_bridge(acr::SIMEXE_ACR, TelemetrySource::Shm, None));
        assert!(use_acr_bridge(acr::SIMEXE_ACR, TelemetrySource::Auto, None));
        assert!(use_acr_bridge(
            acr::SIMEXE_ACR,
            TelemetrySource::Udp,
            Some(&[9])
        ));
    }

    fn live(map_api: i32, uses_udp: bool) -> SeenSim {
        SeenSim {
            is_sim_on: true,
            sim_status: STATUS_ACTIVE_PLAY,
            map_api,
            simulator_api: 0,
            uses_udp,
            sim_exe: 0,
        }
    }

    #[test]
    fn status_uses_the_c_sim_display_name() {
        use simapi_sys::bindings::{
            SimulatorEXE_SIMULATOREXE_ASSETTO_CORSA, SimulatorEXE_SIMULATOREXE_DIRT_RALLY_2,
        };
        const NAME_ASSETTO_CORSA: &str = "Assetto Corsa";
        const NAME_DIRT_RALLY_2: &str = "DiRT Rally 2.0";
        const NAME_UNKNOWN: &str = "default";
        assert_eq!(sim_display_name(0, false), SIM_NOT_DETECTED);
        assert_eq!(
            sim_display_name(u64::from(SimulatorEXE_SIMULATOREXE_ASSETTO_CORSA), true),
            SIM_NOT_DETECTED
        );
        assert_eq!(
            sim_display_name(u64::from(SimulatorEXE_SIMULATOREXE_ASSETTO_CORSA), false),
            NAME_ASSETTO_CORSA
        );
        assert_eq!(
            sim_display_name(u64::from(SimulatorEXE_SIMULATOREXE_DIRT_RALLY_2), false),
            NAME_DIRT_RALLY_2
        );
        assert_eq!(sim_display_name(u64::from(u32::MAX), false), NAME_UNKNOWN);
    }

    #[test]
    fn search_waits_until_the_sim_is_in_active_play() {
        let off = SeenSim {
            is_sim_on: false,
            sim_status: STATUS_ACTIVE_PLAY,
            map_api: MAP_API_SIMD,
            simulator_api: 0,
            uses_udp: false,
            sim_exe: 0,
        };
        let menu = SeenSim {
            sim_status: STATUS_MENU,
            ..live(MAP_API_SIMD, false)
        };
        assert_eq!(search_tick(off, false, false), PlayAction::Wait);
        assert_eq!(search_tick(menu, false, false), PlayAction::Wait);
        assert_eq!(search_tick(live(1, false), false, true), PlayAction::Wait);
        assert_eq!(
            search_tick(live(1, false), false, false),
            PlayAction::StartMapping { use_udp: false }
        );
        assert_eq!(
            search_tick(live(1, false), true, false),
            PlayAction::StartMapping { use_udp: true }
        );
    }

    #[test]
    fn stale_simd_map_asks_for_a_direct_remap() {
        assert!(simd_map_is_stale(MAP_API_SIMD, false));
        assert!(!simd_map_is_stale(MAP_API_SIMD, true));
        assert!(!simd_map_is_stale(1, false));
    }

    #[test]
    fn mapping_releases_only_a_stopped_simd_session() {
        assert_eq!(mapping_tick(live(1, true)), PlayAction::Wait);
        assert_eq!(
            mapping_tick(SeenSim {
                is_sim_on: false,
                ..live(1, true)
            }),
            PlayAction::Wait
        );
        assert_eq!(mapping_tick(live(MAP_API_SIMD, false)), PlayAction::Wait);
        assert_eq!(
            mapping_tick(SeenSim {
                is_sim_on: false,
                ..live(MAP_API_SIMD, false)
            }),
            PlayAction::Release
        );
    }

    #[test]
    fn map_interval_matches_the_c_rounding() {
        assert_eq!(map_interval_ms(60), 17);
        assert_eq!(map_interval_ms(1), 1000);
        assert_eq!(map_interval_ms(0), 1000);
        assert_eq!(MAPPING_START_MS, 2000);
    }

    #[test]
    fn mapping_stops_when_the_sim_leaves_active_play() {
        let live = SeenSim {
            is_sim_on: true,
            sim_status: STATUS_ACTIVE_PLAY,
            map_api: 1,
            simulator_api: 0,
            uses_udp: false,
            sim_exe: 0,
        };
        assert!(!mapping_should_stop(live));
        assert!(mapping_should_stop(SeenSim {
            is_sim_on: false,
            ..live
        }));
        assert!(mapping_should_stop(SeenSim {
            sim_status: STATUS_MENU,
            ..live
        }));
    }

    #[test]
    fn publish_copies_the_frame_and_bumps_sequence() {
        let mut snapshot = FrameSnapshot::new();
        snapshot.publish(&[1, 2, 3]);
        assert_eq!(snapshot.sequence(), 1);
        assert_eq!(snapshot.frame(), &[1, 2, 3]);
        snapshot.publish(&[9]);
        assert_eq!(snapshot.sequence(), 2);
        assert_eq!(snapshot.frame(), &[9]);
    }

    #[test]
    fn acr_bridge_starts_udp_mapping() {
        let seen = bridged_acr(SeenSim {
            is_sim_on: false,
            sim_status: 0,
            map_api: MAP_API_SIMD,
            simulator_api: 0,
            uses_udp: false,
            sim_exe: acr::SIMEXE_ACR,
        });
        assert_eq!(seen.simulator_api, SIMULATOR_API_ASSETTO_CORSA);
        assert_eq!(
            search_tick(seen, false, false),
            PlayAction::StartMapping { use_udp: true }
        );
    }
}
