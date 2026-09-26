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
pub const USB_INIT_LUA_FAILED: i32 = -1;
pub const USB_INIT_CSL_PERMISSION: i32 = 2;
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

pub const MSG_INIT_USB: &str = "initializing usb device...";
pub const MSG_INIT_WHEEL: &str = "initializing wheel or pedals device...";
pub const MSG_G29_ATTEMPT: &str = "Attempting to initialize Logitech G29";
pub const MSG_G29_INIT: &str = "initializing Logitech G29 wheel...";
pub const MSG_G29_FOUND: &str = "Found Logitech G29 Wheel...";
pub const MSG_G29_MISSING: &str = "Could not find attached Logitech G29 Wheel";
pub const MSG_C5_ATTEMPT: &str = "Attempting to initialize cammus C5";
pub const MSG_C5_INIT: &str = "initializing cammus c5 wheel...";
pub const MSG_C5_FOUND: &str = "Found Cammus C5 Wheel...";
pub const MSG_C5_MISSING: &str = "Could not find attached Cammus C5 Wheel";
pub const MSG_C12_ATTEMPT: &str = "Attempting to initialize cammus C12";
pub const MSG_C12_INIT: &str = "initializing cammus c12 wheel...";
pub const MSG_C12_FOUND: &str = "Found Cammus C12 Wheel...";
pub const MSG_C12_MISSING: &str = "Could not find attached Cammus C12 Wheel";
pub const MSG_C12_LUA: &str = "Using lua file for cammus c12 device";
pub const MSG_C12_LUA_ISSUE: &str = "There is an issue with your lua script";
pub const MSG_C12_LUA_CLOSE: &str = "closing lua";
pub const MSG_GT_ATTEMPT: &str = "Attempting to initialize Simagic GT Neo";
pub const MSG_GT_INIT: &str = "initializing Simagic GT Neo wheel...";
pub const MSG_GT_FOUND: &str = "Found Simagic GT Neo Wheel...";
pub const MSG_GT_MISSING: &str = "Could not find attached GT Neo Wheel";
pub const MSG_GT_LUA: &str = "Using lua file";
pub const MSG_GT_NEEDS_CONFIG: &str = "Simagic GT Neo requires lua config file to function";
pub const MSG_GT_FEATURE_FAILED: &str = "Failed to send HID feature report";
pub const MSG_GT_FEATURE_CHUNK_FAILED: &str = "Failed to send HID feature report chunk";
pub const MSG_USB_NO_HAPTICS: &str = "This sim does not support haptic effects";
pub const MSG_CSL_ATTEMPT: &str = "Attempting to initialize CSL Elite V3 Pedals";
pub const MSG_CSL_INIT: &str = "initializing CSL Elite V3 Pedals...";
pub const MSG_CSL_FOUND: &str = "CSL Elite V3 Pedals Successfully initialized...";
pub const MSG_CSL_MISSING: &str = "Could not find attached Club Sport Elite V3 Pedals";
pub const MSG_CSL_PERMISSION: &str = "Permissions issue finding Club Sport Elite V3 Pedals";
pub const MSG_CSL_OPEN: &str = "Could not open pedal device...";
pub const P1000_DEVICE_NAME: &str = "SIMAGIC P1000 Pedals";
pub const MSG_INIT_TACH: &str = "initializing tachometer device...";
pub const MSG_INIT_REVBURNER: &str = "initializing revburner tachometer...";
pub const MSG_REVBURNER_MISSING: &str = "Could not find attached RevBurner tachometer";
pub const DEVICE_NAME_MISSING: &str = "(null)";

pub fn g29_write_message(report: &[u8], rpm: i32) -> String {
    let byte = |index: usize| report.get(index).copied().unwrap_or(0);
    format!(
        "writing bytes x{:02x}x{:02x}x{:02x}x{:02x}x{:02x} from rpm {rpm}",
        byte(cargopit_devices::usb::G29_BYTE_REPORT),
        byte(cargopit_devices::usb::G29_BYTE_CMD),
        byte(cargopit_devices::usb::G29_BYTE_LEDS),
        byte(cargopit_devices::usb::G29_BYTE_PAD),
        byte(cargopit_devices::usb::G29_BYTE_TAIL),
    )
}

pub fn c5_write_message(report: &[u8], rpm: i32, velocity: i32, gear: i32) -> String {
    let byte = |index: usize| report.get(index).copied().unwrap_or(0);
    format!(
        "writing bytes x{:02x}x{:02x}x{:02x}x{:02x}x{:02x} from rpm {rpm} velocity {velocity} gear {gear}",
        byte(cargopit_devices::usb::C5_BYTE_REPORT),
        byte(cargopit_devices::usb::C5_BYTE_LEDS),
        byte(cargopit_devices::usb::C5_BYTE_VELOCITY_HIGH),
        byte(cargopit_devices::usb::C5_BYTE_VELOCITY_LOW),
        byte(cargopit_devices::usb::C5_BYTE_GEAR),
    )
}

pub fn c12_led_message(report: &[u8]) -> String {
    let byte = |index: usize| report.get(index).copied().unwrap_or(0);
    let red = i32::from(byte(cargopit_devices::usb::C12_BYTE_RED));
    let green = i32::from(byte(cargopit_devices::usb::C12_BYTE_GREEN));
    let blue = i32::from(byte(cargopit_devices::usb::C12_BYTE_BLUE));
    format!(
        "writing bytes x{:02x}x{:02x}x{:02x}x{:02x}x{:02x}x{:02x}x{:02x} from red {red} green {green} blue {blue}",
        byte(cargopit_devices::usb::C12_BYTE_MARK0),
        byte(cargopit_devices::usb::C12_BYTE_MARK1),
        byte(cargopit_devices::usb::C12_BYTE_MARK2),
        byte(cargopit_devices::usb::C12_BYTE_LED),
        byte(cargopit_devices::usb::C12_BYTE_RED),
        byte(cargopit_devices::usb::C12_BYTE_GREEN),
        byte(cargopit_devices::usb::C12_BYTE_BLUE),
    )
}

pub fn lua_load_failed_message(detail: &str) -> String {
    format!("Couldn't load file: {detail}")
}

pub fn lua_call_failed_message(detail: &str) -> String {
    format!("Error calling Lua script: {detail}")
}

pub fn c12_write_message(report: &[u8], rpm: i32, velocity: i32, gear: i32) -> String {
    let byte = |index: usize| report.get(index).copied().unwrap_or(0);
    format!(
        "writing bytes x{:02x}x{:02x}x{:02x}x{:02x}x{:02x}x{:02x}x{:02x} from rpm {rpm} velocity {velocity} gear {gear}",
        byte(cargopit_devices::usb::C12_BYTE_MARK0),
        byte(cargopit_devices::usb::C12_BYTE_MARK1),
        byte(cargopit_devices::usb::C12_BYTE_MARK2),
        byte(cargopit_devices::usb::C12_BYTE_PERCENT),
        byte(cargopit_devices::usb::C12_BYTE_VELOCITY_LOW),
        byte(cargopit_devices::usb::C12_BYTE_VELOCITY_HIGH),
        byte(cargopit_devices::usb::C12_BYTE_GEAR),
    )
}

pub fn p1000_init_message() -> String {
    format!("initializing {P1000_DEVICE_NAME}...")
}

pub fn p1000_missing_message() -> String {
    format!("Could not find attached {P1000_DEVICE_NAME}")
}

pub fn p1000_found_message() -> String {
    format!("Found {P1000_DEVICE_NAME}...")
}

pub fn p1000_problem_message() -> String {
    format!("Problem with initialization of {P1000_DEVICE_NAME}")
}

pub fn p1000_sent_message(nbytes: i32) -> String {
    format!("sent {nbytes} bytes to {P1000_DEVICE_NAME}")
}

pub fn p1000_bytes_message(report: &[u8]) -> String {
    let byte = |index: usize| report.get(index).copied().unwrap_or(0);
    format!(
        "sent bytes {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
        byte(cargopit_devices::usb::P1000_BYTE_MARK),
        byte(cargopit_devices::usb::P1000_BYTE_CMD),
        byte(cargopit_devices::usb::P1000_BYTE_KIND),
        byte(cargopit_devices::usb::P1000_BYTE_FLAG0),
        byte(cargopit_devices::usb::P1000_BYTE_FLAG1),
        byte(cargopit_devices::usb::P1000_BYTE_FLAG2),
        byte(cargopit_devices::usb::P1000_BYTE_INIT0),
    )
}

pub fn p1000_init_result_message(code: i32) -> String {
    format!("Initialization returned {code}")
}

pub const MSG_SIMNET_INIT: &str = "initializing simnet pedals...";
pub const MSG_SIMNET_MISSING: &str = "Could not find attached Simnet Pedals";
pub const SIMNET_CAPTURED_HANDLE: i32 = 0;

pub fn simnet_found_message(handle: i32) -> String {
    format!("Found Simnet Pedals handle {handle}...")
}

pub fn simnet_write_message(report: &[u8]) -> String {
    let mut message = String::from("writing bytes ");
    for index in 0..cargopit_devices::usb::SIMNET_TRACE_LEN {
        let byte = report.get(index).copied().unwrap_or(0);
        message.push_str(&format!("x{byte:02x}"));
    }
    message
}

pub fn usb_init_error_message(code: i32) -> String {
    format!("Did not initialize usb device due to error code {code}")
}

pub fn could_not_initialize_message(class_name: &str) -> String {
    format!("Could not initialize {class_name} device")
}

pub fn initialized_devices_message(count: i32) -> String {
    format!("initialized {count} devices")
}

pub const MSG_TACH_CONFIG_NONE: &str = "config set to none";
pub const MSG_TACH_CONFIG_REQUIRED: &str = "Tachometer must have a device specific config file!";
pub const MSG_TACH_XML_INVALID: &str = "Invalid rev burner xml";
pub const MSG_TACH_XML_EMPTY: &str = "Rev burner XML contains no settings";
pub const MSG_TACH_GRANULARITY_INVALID: &str =
    "No or invalid valid set for tachometer granularity, setting to 1";
pub const MSG_TACH_GETTING_PULSES: &str = "Getting pulses for current tachometer revs";
pub const MSG_REVBURNER_NO_HANDLE: &str = "no handle";
pub const SERIAL_OPEN_ERROR: i32 = -1;
pub const UNSUPPORTED_SIM_FEATURE: i32 = 6;
pub const MSG_SHIFTLIGHTS_INIT: &str = "Initializing arduino device for shiftlights.";
pub const MSG_SERIAL_HAPTIC_INIT: &str = "Initializing arduino device for haptic effects.";
pub const MSG_SIMLED_INIT: &str = "Initializing arduino device for simled.";
pub const MSG_SIMLED_CUSTOM_INIT: &str = "Initializing arduino device for custom simled.";
pub const MSG_ARDUINO_CUSTOM_INIT: &str = "Initializing custom arduino device.";
pub const MSG_SIMLED_COUNT_ATTEMPT: &str = "Attempting to retrieve num lights from port...";
pub const MSG_SIMLED_COUNT_SEND: &str = "Sending message to get num lights";
pub const MSG_SIMLED_LUA_INIT: &str = "LUA config specified for this device... initializing...";
pub const MSG_SIMLED_LUA_OK: &str = "LUA config setup successful.";
pub const MSG_SERIAL_HAPTIC_UPDATING: &str = "arduino haptic device updating";
pub const MSG_SERIAL_HAPTIC_ZERO: &str = "set zero to arduino device";
pub const MSG_MOZA_NEW_INIT: &str = "Initializing new firmware Moza serial wheel device.";
pub const MSG_SERIAL_START: &str = "Starting serial device initialization";
pub const MSG_SERIAL_NO_EXISTING: &str = "no existing connections found, looking to create new";
pub const MSG_SERIAL_OPENING: &str = "opening physical serial device...";
pub const MSG_SERIAL_OPEN_ERROR: &str = "Error opening serial port";
pub const MSG_SERIAL_PORT_OPENED: &str = "Opening port";
pub const MSG_SERIAL_SETUP_OK: &str = "Successfully setup cargopit serial device...";
pub const MSG_MOZA_ARM_FAILED: &str = "Moza R9 telemetry arm failed";
pub const MSG_MOZA_ARMED: &str = "Moza R9 telemetry mode armed";
pub const MSG_MOZA_RPM_FAILED: &str = "Moza R9 RPM bitmask write failed";
pub const MSG_SHARE_HANDLE: &str = "could not get native serial handle to share port";
pub const MSG_SHARE_EXCLUSIVE: &str = "TIOCNXCL failed; Boxflat may still contend for the wheel";
pub const MSG_SHARE_HUPCL: &str = "could not clear HUPCL on Moza serial";

pub fn serial_init_error_message(code: i32) -> String {
    format!("Did not initialize serial device due to error code {code}")
}

pub fn serial_subtype_message(subtype: i32) -> String {
    format!("Attempting to configure arduino device with subtype: {subtype}")
}

pub fn serial_init_port_message(path: &str, baud: i64) -> String {
    format!("initializing serial device on port {path} to {baud}...")
}

pub fn serial_looking_message(path: &str) -> String {
    format!("looking to open physical serialdevice {path}")
}

pub fn serial_looking_for_port_message(path: &str) -> String {
    format!("Looking for port {path}")
}

pub fn serial_baud_message(baud: u32) -> String {
    format!("Setting port to {baud} 8N1, no flow control")
}

pub fn moza_opened_message(path: &str) -> String {
    format!("Moza R9 wheel opened on {path}")
}

pub fn moza_arm_retry_message(path: &str) -> String {
    format!("Moza R9 opened on {path} but telemetry arm will retry")
}

pub const VIBRATION_ENGINE: &str = "engine vibrations.";
pub const VIBRATION_GEAR: &str = "gear shift vibrations.";
pub const VIBRATION_SLIP: &str = "tyre slip vibrations.";
pub const VIBRATION_LOCK: &str = "tyre lock vibrations.";
pub const VIBRATION_ABS: &str = "abs vibrations.";
pub const VIBRATION_SUSPENSION: &str = "suspension vibrations.";
pub const MSG_SOUND_SKIP_HAPTICS: &str =
    "Skipping sound effect setup because sim does not support haptic effects";
pub const MSG_SOUND_STANDALONE: &str = "initializing standalone sound device...";

pub fn haptic_effect_message(phrase: &str) -> String {
    format!("Initializing haptic effect for {phrase}")
}

pub fn unknown_haptic_message(effect: i32) -> String {
    format!("Initializing unknown haptic effect type {effect}.")
}

pub fn haptic_summary_message(effect: i32, tyre: i32) -> String {
    format!("Haptic effect: {effect} {effect}, tyre {tyre} {tyre}")
}

pub fn haptic_duration_message(duration: f64) -> String {
    format!("haptic duration: {duration:.6}")
}

pub fn haptic_frequency_message(frequency: i64) -> String {
    format!("haptic base frequency: {frequency}")
}

pub fn haptic_amplitude_message(amplitude: i64) -> String {
    format!("haptic base amplitude: {amplitude}")
}

pub fn haptic_motor_message(motor: i64) -> String {
    format!("haptic motorposition: {motor}")
}

pub fn sound_subtype_message(effect: i32) -> String {
    format!("Attempting to configure sound device with subtype: {effect}")
}

pub fn sound_effect_message(phrase: &str) -> String {
    format!("Initializing sound device for {phrase}")
}

pub fn sound_use_message(path: &str) -> String {
    format!("Attempting to use sound device {path}")
}

pub fn sound_volume_message(volume: i64) -> String {
    format!("pipewire stream volume is: {volume}")
}

pub fn sound_channel_mask_message(mask: u32) -> String {
    format!("channel mask is: {mask}")
}

pub fn sound_channels_message(channels: i64) -> String {
    format!("channels is: {channels}")
}

pub fn sound_noise_message(noise: i64) -> String {
    format!("noise is: {noise}")
}

pub fn sound_node_message(name: &str) -> String {
    format!("sound stream node name is: {name}")
}

pub fn sound_describe_error(effect: i32) -> String {
    format!("could not describe sound stream for effect {effect}")
}

pub const SOUND_DEFAULT_SINK: &str = "the default sink";

pub fn sound_connect_error(node: &str, sink: &str, err: &str) -> String {
    let label = if sink.is_empty() {
        SOUND_DEFAULT_SINK
    } else {
        sink
    };
    format!("could not connect sound stream {node} to {label}: {err}")
}

pub fn serial_free_message(path: &str) -> String {
    format!("freeing physical device {path}")
}

pub fn shiftlights_lit_message(lit: i32) -> String {
    format!("Updating arduino device lights to {lit}")
}

pub fn simled_count_message(count: i32) -> String {
    format!("numlights is {count}\n")
}

pub fn simled_count_invalid_message(text: &str) -> String {
    format!("Invalid LED count received from serial device: {text}")
}

pub fn simled_count_wait_message(code: i32) -> String {
    format!("Error getting bytes available from serial port: {code}")
}

pub fn simled_custom_wrote_message(size: i32) -> String {
    format!("custom led wrote {size} bytes")
}

pub fn arduino_custom_wrote_message(message: &str, size: i32) -> String {
    format!("custom arduino wrote message {message} of {size} bytes")
}

pub fn arduino_copy_message(size: i32) -> String {
    format!("copying {size} bytes to arduino device")
}

pub fn simwind_init_message(fanpower: f64) -> String {
    format!("Initializing arduino devices for sim wind with fanpower: {fanpower:.6}")
}

pub fn simwind_speed_message(mph: i32) -> String {
    format!("Updating arduino device speed to {mph}")
}

pub fn simwind_fan_message(fan: i32, configured: f64) -> String {
    format!("Sending fanpower: {fan} (from config: {configured:.6})")
}

pub fn serial_haptic_channel_message(
    effect: i32,
    speed: i32,
    motor: i32,
    raw_play: f64,
    ampfactor: f64,
) -> String {
    format!(
        "Updating arduino haptic device with effect type {effect} speed motor speed {speed} on motor {motor} from original effect {raw_play:.6} with ampfactor {ampfactor:.6}"
    )
}

pub fn tach_config_load_message(path: &str) -> String {
    format!("will try to load config file at {path}")
}

pub fn tach_xml_read_message(path: &str) -> String {
    format!("Could not read revburner xml config file {path}")
}

pub fn tach_granularity_message(granularity: i64) -> String {
    format!("Tachometer granularity set to {granularity}")
}

pub fn tach_settings_size_message(size: i32) -> String {
    format!("Tach settings size {size}")
}

pub fn tach_element_message(element: i32) -> String {
    format!("Retrieveing element {element}")
}

pub fn tach_pulses_message(pulses: u32) -> String {
    format!("Settings tachometer pulses to {pulses}")
}

pub fn starting_device_message(device_type: i32, id: i32, fps: u32) -> String {
    format!("starting device type {device_type} at id {id} on its own thread at {fps} fps")
}

pub fn device_runner_message(index: i32, updates: u64, overruns: u64) -> String {
    format!("device {index}: {updates} updates, {overruns} overruns")
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
        assert_eq!(MSG_INIT_USB, "initializing usb device...");
        assert_eq!(MSG_INIT_WHEEL, "initializing wheel or pedals device...");
        assert_eq!(MSG_G29_ATTEMPT, "Attempting to initialize Logitech G29");
        assert_eq!(MSG_G29_INIT, "initializing Logitech G29 wheel...");
        assert_eq!(MSG_G29_FOUND, "Found Logitech G29 Wheel...");
        assert_eq!(
            MSG_G29_MISSING,
            "Could not find attached Logitech G29 Wheel"
        );
        const G29_SAMPLE_RPM: u32 = 6_000;
        const G29_SAMPLE_MAX: u32 = 7_000;
        let report = cargopit_devices::usb::g29_report(G29_SAMPLE_RPM, G29_SAMPLE_MAX);
        assert_eq!(
            g29_write_message(&report, i32::try_from(G29_SAMPLE_RPM).unwrap_or(i32::MAX)),
            "writing bytes xf8x12x1fx00x01 from rpm 6000"
        );
        assert_eq!(MSG_C5_ATTEMPT, "Attempting to initialize cammus C5");
        assert_eq!(MSG_C5_INIT, "initializing cammus c5 wheel...");
        assert_eq!(MSG_C5_FOUND, "Found Cammus C5 Wheel...");
        assert_eq!(MSG_C5_MISSING, "Could not find attached Cammus C5 Wheel");
        const C5_SAMPLE_GEAR: u32 = 4;
        const C5_SAMPLE_VELOCITY: u32 = 300;
        let c5 = cargopit_devices::usb::c5_report(
            G29_SAMPLE_RPM,
            G29_SAMPLE_MAX,
            C5_SAMPLE_GEAR,
            C5_SAMPLE_VELOCITY,
        );
        assert_eq!(
            c5_write_message(
                &c5,
                i32::try_from(G29_SAMPLE_RPM).unwrap_or(i32::MAX),
                i32::try_from(C5_SAMPLE_VELOCITY).unwrap_or(i32::MAX),
                i32::try_from(C5_SAMPLE_GEAR).unwrap_or(i32::MAX),
            ),
            "writing bytes xfcx09x01x2cx03 from rpm 6000 velocity 300 gear 4"
        );
        assert_eq!(MSG_C12_ATTEMPT, "Attempting to initialize cammus C12");
        assert_eq!(MSG_C12_INIT, "initializing cammus c12 wheel...");
        assert_eq!(MSG_C12_FOUND, "Found Cammus C12 Wheel...");
        assert_eq!(MSG_C12_MISSING, "Could not find attached Cammus C12 Wheel");
        let c12 = cargopit_devices::usb::c12_report(
            G29_SAMPLE_RPM,
            G29_SAMPLE_MAX,
            C5_SAMPLE_GEAR,
            C5_SAMPLE_VELOCITY,
        );
        assert_eq!(
            c12_write_message(
                &c12,
                i32::try_from(G29_SAMPLE_RPM).unwrap_or(i32::MAX),
                i32::try_from(C5_SAMPLE_VELOCITY).unwrap_or(i32::MAX),
                i32::try_from(C5_SAMPLE_GEAR).unwrap_or(i32::MAX),
            ),
            "writing bytes xfaxfbxd4x56x2cx01x03 from rpm 6000 velocity 300 gear 4"
        );
        assert_eq!(MSG_C12_LUA, "Using lua file for cammus c12 device");
        assert_eq!(MSG_C12_LUA_ISSUE, "There is an issue with your lua script");
        assert_eq!(MSG_C12_LUA_CLOSE, "closing lua");
        assert_eq!(MSG_GT_ATTEMPT, "Attempting to initialize Simagic GT Neo");
        assert_eq!(MSG_GT_INIT, "initializing Simagic GT Neo wheel...");
        assert_eq!(MSG_GT_FOUND, "Found Simagic GT Neo Wheel...");
        assert_eq!(MSG_GT_MISSING, "Could not find attached GT Neo Wheel");
        assert_eq!(MSG_GT_LUA, "Using lua file");
        assert_eq!(
            MSG_GT_NEEDS_CONFIG,
            "Simagic GT Neo requires lua config file to function"
        );
        assert_eq!(MSG_GT_FEATURE_FAILED, "Failed to send HID feature report");
        assert_eq!(
            MSG_GT_FEATURE_CHUNK_FAILED,
            "Failed to send HID feature report chunk"
        );
        assert_eq!(USB_INIT_CSL_PERMISSION, 2);
        assert_eq!(
            MSG_USB_NO_HAPTICS,
            "This sim does not support haptic effects"
        );
        assert_eq!(
            MSG_CSL_ATTEMPT,
            "Attempting to initialize CSL Elite V3 Pedals"
        );
        assert_eq!(MSG_CSL_INIT, "initializing CSL Elite V3 Pedals...");
        assert_eq!(
            MSG_CSL_FOUND,
            "CSL Elite V3 Pedals Successfully initialized..."
        );
        assert_eq!(
            MSG_CSL_MISSING,
            "Could not find attached Club Sport Elite V3 Pedals"
        );
        assert_eq!(
            MSG_CSL_PERMISSION,
            "Permissions issue finding Club Sport Elite V3 Pedals"
        );
        assert_eq!(MSG_CSL_OPEN, "Could not open pedal device...");
        assert_eq!(P1000_DEVICE_NAME, "SIMAGIC P1000 Pedals");
        assert_eq!(p1000_init_message(), "initializing SIMAGIC P1000 Pedals...");
        assert_eq!(
            p1000_missing_message(),
            "Could not find attached SIMAGIC P1000 Pedals"
        );
        assert_eq!(p1000_found_message(), "Found SIMAGIC P1000 Pedals...");
        assert_eq!(
            p1000_problem_message(),
            "Problem with initialization of SIMAGIC P1000 Pedals"
        );
        assert_eq!(
            p1000_sent_message(cargopit_devices::usb::P1000_LEN as i32),
            "sent 49 bytes to SIMAGIC P1000 Pedals"
        );
        assert_eq!(
            p1000_init_result_message(cargopit_devices::usb::P1000_REPORT_OK),
            "Initialization returned 0"
        );
        assert_eq!(
            p1000_bytes_message(&cargopit_devices::usb::p1000_init_report()),
            "sent bytes f1 f1 17 00 00 00 01"
        );
        assert_eq!(MSG_SIMNET_INIT, "initializing simnet pedals...");
        assert_eq!(MSG_SIMNET_MISSING, "Could not find attached Simnet Pedals");
        assert_eq!(
            simnet_found_message(SIMNET_CAPTURED_HANDLE),
            "Found Simnet Pedals handle 0..."
        );
        const SIMNET_SAMPLE_MOTOR: u32 = 1;
        const SIMNET_SAMPLE_FREQ: u32 = 40;
        const SIMNET_SAMPLE_AMP: u32 = 100;
        let sample = cargopit_devices::usb::simnet_report(
            SIMNET_SAMPLE_MOTOR,
            SIMNET_SAMPLE_FREQ,
            SIMNET_SAMPLE_AMP,
            true,
        );
        assert_eq!(
            simnet_write_message(&sample),
            "writing bytes x01x00x01x28x64x00x00x00x00"
        );
        assert_eq!(USB_INIT_LUA_FAILED, -1);
        assert_eq!(
            usb_init_error_message(USB_INIT_LUA_FAILED),
            "Did not initialize usb device due to error code -1"
        );
        let mut colors = vec![0u8; cargopit_devices::usb::c12_led_color_len()];
        let first =
            cargopit_devices::usb::c12_led_color_index(cargopit_devices::usb::C12_LED_FIRST);
        if let Some(red) = colors.get_mut(first) {
            *red = u8::MAX;
        }
        let led = cargopit_devices::usb::c12_led_reports(&colors);
        let sample = led
            .first()
            .copied()
            .unwrap_or([0; cargopit_devices::usb::C12_LED_LEN]);
        assert_eq!(
            c12_led_message(&sample),
            "writing bytes xfaxfbx02x10xffx00x00 from red 255 green 0 blue 0"
        );
        assert_eq!(
            lua_load_failed_message("cannot open missing.lua: No such file or directory"),
            "Couldn't load file: cannot open missing.lua: No such file or directory"
        );
        assert_eq!(
            lua_call_failed_message("boom"),
            "Error calling Lua script: boom"
        );
        assert_eq!(MSG_INIT_TACH, "initializing tachometer device...");
        assert_eq!(MSG_INIT_REVBURNER, "initializing revburner tachometer...");
        assert_eq!(
            MSG_REVBURNER_MISSING,
            "Could not find attached RevBurner tachometer"
        );
        assert_eq!(
            usb_init_error_message(ERROR_UNKNOWN),
            "Did not initialize usb device due to error code 1"
        );
        assert_eq!(
            could_not_initialize_message(cargopit_config::keys::CLASS_USB),
            "Could not initialize USB device"
        );
        assert_eq!(initialized_devices_message(0), "initialized 0 devices");
        assert_eq!(MSG_TACH_CONFIG_NONE, "config set to none");
        assert_eq!(
            MSG_TACH_CONFIG_REQUIRED,
            "Tachometer must have a device specific config file!"
        );
        assert_eq!(MSG_TACH_XML_INVALID, "Invalid rev burner xml");
        assert_eq!(MSG_TACH_XML_EMPTY, "Rev burner XML contains no settings");
        assert_eq!(
            MSG_TACH_GRANULARITY_INVALID,
            "No or invalid valid set for tachometer granularity, setting to 1"
        );
        assert_eq!(
            tach_config_load_message("/tmp/revburner.xml"),
            "will try to load config file at /tmp/revburner.xml"
        );
        assert_eq!(
            tach_xml_read_message("/tmp/revburner.xml"),
            "Could not read revburner xml config file /tmp/revburner.xml"
        );
        assert_eq!(
            tach_granularity_message(1),
            "Tachometer granularity set to 1"
        );
        assert_eq!(tach_settings_size_message(9), "Tach settings size 9");
        assert_eq!(tach_element_message(2), "Retrieveing element 2");
        assert_eq!(
            tach_pulses_message(43600),
            "Settings tachometer pulses to 43600"
        );
        assert_eq!(
            MSG_TACH_GETTING_PULSES,
            "Getting pulses for current tachometer revs"
        );
        assert_eq!(MSG_REVBURNER_NO_HANDLE, "no handle");
        assert_eq!(SERIAL_OPEN_ERROR, -1);
        assert_eq!(UNSUPPORTED_SIM_FEATURE, 6);
        assert_eq!(
            serial_init_error_message(SERIAL_OPEN_ERROR),
            "Did not initialize serial device due to error code -1"
        );
        assert_eq!(
            serial_subtype_message(cargopit_config::names::SUBTYPE_SERIAL_WHEEL),
            "Attempting to configure arduino device with subtype: 8"
        );
        assert_eq!(
            serial_init_port_message("/dev/ttyACM0", cargopit_config::keys::BAUD_DEFAULT),
            "initializing serial device on port /dev/ttyACM0 to 9600..."
        );
        assert_eq!(
            serial_looking_message("/dev/ttyACM0"),
            "looking to open physical serialdevice /dev/ttyACM0"
        );
        assert_eq!(
            MSG_SERIAL_NO_EXISTING,
            "no existing connections found, looking to create new"
        );
        assert_eq!(MSG_SERIAL_OPENING, "opening physical serial device...");
        assert_eq!(
            serial_looking_for_port_message("/dev/ttyACM0"),
            "Looking for port /dev/ttyACM0"
        );
        assert_eq!(MSG_SERIAL_OPEN_ERROR, "Error opening serial port");
        assert_eq!(MSG_SERIAL_PORT_OPENED, "Opening port");
        assert_eq!(
            serial_baud_message(cargopit_devices::serial::moza_r9_open_baud(
                cargopit_config::keys::BAUD_DEFAULT,
            )),
            "Setting port to 115200 8N1, no flow control"
        );
        assert_eq!(
            MSG_SERIAL_SETUP_OK,
            "Successfully setup cargopit serial device..."
        );
        assert_eq!(
            moza_opened_message("/dev/ttyACM0"),
            "Moza R9 wheel opened on /dev/ttyACM0"
        );
        assert_eq!(
            moza_arm_retry_message("/dev/ttyACM0"),
            "Moza R9 opened on /dev/ttyACM0 but telemetry arm will retry"
        );
        assert_eq!(
            haptic_effect_message(VIBRATION_GEAR),
            "Initializing haptic effect for gear shift vibrations."
        );
        assert_eq!(
            sound_effect_message(VIBRATION_LOCK),
            "Initializing sound device for tyre lock vibrations."
        );
        assert_eq!(
            haptic_summary_message(
                cargopit_config::names::EFFECT_GEAR,
                cargopit_config::names::TYRE_FRONT_LEFT,
            ),
            "Haptic effect: 1 1, tyre 0 0"
        );
        assert_eq!(haptic_duration_message(0.1), "haptic duration: 0.100000");
        assert_eq!(haptic_frequency_message(50), "haptic base frequency: 50");
        assert_eq!(haptic_amplitude_message(100), "haptic base amplitude: 100");
        assert_eq!(haptic_motor_message(1), "haptic motorposition: 1");
        assert_eq!(
            sound_subtype_message(cargopit_config::names::EFFECT_TYRE_LOCK),
            "Attempting to configure sound device with subtype: 4"
        );
        assert_eq!(
            sound_use_message("alsa_output.test"),
            "Attempting to use sound device alsa_output.test"
        );
        assert_eq!(
            MSG_SOUND_STANDALONE,
            "initializing standalone sound device..."
        );
        assert_eq!(sound_volume_message(40), "pipewire stream volume is: 40");
        assert_eq!(sound_channel_mask_message(3), "channel mask is: 3");
        assert_eq!(sound_channels_message(2), "channels is: 2");
        assert_eq!(sound_noise_message(0), "noise is: 0");
        assert_eq!(
            sound_node_message("cargopit.TyreLock.All"),
            "sound stream node name is: cargopit.TyreLock.All"
        );
        assert_eq!(
            MSG_SOUND_SKIP_HAPTICS,
            "Skipping sound effect setup because sim does not support haptic effects"
        );
        assert_eq!(
            sound_describe_error(cargopit_config::names::EFFECT_ENGINE),
            "could not describe sound stream for effect 0"
        );
        assert_eq!(
            sound_connect_error("cargopit.Gear", "alsa_output.test", "Bad state"),
            "could not connect sound stream cargopit.Gear to alsa_output.test: Bad state"
        );
        assert_eq!(
            sound_connect_error("cargopit.Gear", "", "Bad state"),
            "could not connect sound stream cargopit.Gear to the default sink: Bad state"
        );
        assert_eq!(
            unknown_haptic_message(9),
            "Initializing unknown haptic effect type 9."
        );
        assert_eq!(
            MSG_SHIFTLIGHTS_INIT,
            "Initializing arduino device for shiftlights."
        );
        assert_eq!(
            shiftlights_lit_message(1),
            "Updating arduino device lights to 1"
        );
        assert_eq!(
            arduino_copy_message(cargopit_devices::serial::SHIFT_PACKET_LEN),
            "copying 1 bytes to arduino device"
        );
        assert_eq!(
            simwind_init_message(cargopit_config::keys::FANPOWER_DEFAULT),
            "Initializing arduino devices for sim wind with fanpower: 0.600000"
        );
        const SIMWIND_SAMPLE_MPH: i32 = 50;
        const SIMWIND_SAMPLE_FAN: i32 = 153;
        assert_eq!(
            simwind_speed_message(SIMWIND_SAMPLE_MPH),
            "Updating arduino device speed to 50"
        );
        assert_eq!(
            simwind_fan_message(SIMWIND_SAMPLE_FAN, cargopit_config::keys::FANPOWER_DEFAULT),
            "Sending fanpower: 153 (from config: 0.600000)"
        );
        assert_eq!(
            MSG_SERIAL_HAPTIC_INIT,
            "Initializing arduino device for haptic effects."
        );
        assert_eq!(MSG_SIMLED_INIT, "Initializing arduino device for simled.");
        assert_eq!(
            MSG_SIMLED_CUSTOM_INIT,
            "Initializing arduino device for custom simled."
        );
        assert_eq!(
            MSG_ARDUINO_CUSTOM_INIT,
            "Initializing custom arduino device."
        );
        const ARDUINO_SAMPLE_MESSAGE: &str = "parity";
        const ARDUINO_SAMPLE_BYTES: i32 = 6;
        assert_eq!(
            arduino_custom_wrote_message(ARDUINO_SAMPLE_MESSAGE, ARDUINO_SAMPLE_BYTES),
            "custom arduino wrote message parity of 6 bytes"
        );
        assert_eq!(
            MSG_SIMLED_COUNT_ATTEMPT,
            "Attempting to retrieve num lights from port..."
        );
        assert_eq!(MSG_SIMLED_COUNT_SEND, "Sending message to get num lights");
        const SIMLED_SAMPLE_COUNT: i32 = 8;
        const SIMLED_SAMPLE_BYTES: i32 = 38;
        const SIMLED_SAMPLE_WAIT: i32 = -1;
        assert_eq!(
            simled_count_message(SIMLED_SAMPLE_COUNT),
            "numlights is 8\n"
        );
        assert_eq!(
            simled_count_invalid_message("nope"),
            "Invalid LED count received from serial device: nope"
        );
        assert_eq!(
            simled_count_wait_message(SIMLED_SAMPLE_WAIT),
            "Error getting bytes available from serial port: -1"
        );
        assert_eq!(
            MSG_SIMLED_LUA_INIT,
            "LUA config specified for this device... initializing..."
        );
        assert_eq!(MSG_SIMLED_LUA_OK, "LUA config setup successful.");
        assert_eq!(
            simled_custom_wrote_message(SIMLED_SAMPLE_BYTES),
            "custom led wrote 38 bytes"
        );
        assert_eq!(MSG_SERIAL_HAPTIC_UPDATING, "arduino haptic device updating");
        assert_eq!(MSG_SERIAL_HAPTIC_ZERO, "set zero to arduino device");
        const HAPTIC_SAMPLE_SPEED: i32 = 255;
        const HAPTIC_SAMPLE_MOTOR: i32 = 1;
        const HAPTIC_SAMPLE_PLAY: f64 = 1.0;
        assert_eq!(
            serial_haptic_channel_message(
                cargopit_config::names::EFFECT_TYRE_SLIP,
                HAPTIC_SAMPLE_SPEED,
                HAPTIC_SAMPLE_MOTOR,
                HAPTIC_SAMPLE_PLAY,
                cargopit_config::keys::AMPFACTOR_DEFAULT,
            ),
            "Updating arduino haptic device with effect type 3 speed motor speed 255 on motor 1 from original effect 1.000000 with ampfactor 1.000000"
        );
        assert_eq!(
            serial_init_error_message(UNSUPPORTED_SIM_FEATURE),
            "Did not initialize serial device due to error code 6"
        );
        assert_eq!(
            arduino_copy_message(cargopit_devices::serial::HAPTIC_PACKET_LEN),
            "copying 8 bytes to arduino device"
        );
        assert_eq!(MSG_MOZA_ARM_FAILED, "Moza R9 telemetry arm failed");
        assert_eq!(MSG_MOZA_ARMED, "Moza R9 telemetry mode armed");
        assert_eq!(MSG_MOZA_RPM_FAILED, "Moza R9 RPM bitmask write failed");
        assert_eq!(
            serial_free_message("/dev/ttyACM0"),
            "freeing physical device /dev/ttyACM0"
        );
        assert_eq!(
            starting_device_message(cargopit_config::names::DEVICE_USB, 0, 60),
            "starting device type 0 at id 0 on its own thread at 60 fps"
        );
        assert_eq!(
            device_runner_message(0, 1, 0),
            "device 0: 1 updates, 0 overruns"
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
