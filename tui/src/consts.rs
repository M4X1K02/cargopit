//! Named constants for the TUI. No magic strings or layout numbers elsewhere.

use crossterm::event::KeyCode;

pub const APP_TITLE: &str = "Cargopit TUI";
pub const BINARY_CARGOPIT: &str = "cargopit";
pub const BINARY_SIMD: &str = "simd";
pub const BINARY_TUI: &str = "cargopit-tui";

pub const CONFIG_FILE_NAME: &str = "cargopit.config";
pub const CONFIG_DIR_NAME: &str = "cargopit";
pub const SIMD_CONFIG_DIR_NAME: &str = "simd";
pub const SIMD_CONFIG_FILE_NAME: &str = "simd.config";
pub const DIAMETERS_FILE_NAME: &str = "diameters.config";
pub const TUI_STATE_FILE_NAME: &str = "tui-state.json";
pub const CACHE_DIR_NAME: &str = "cargopit";
pub const LOG_GLOB_SUFFIX: &str = ".log";

pub const PID_FILE_SIMD: &str = "/tmp/simd.pid";
pub const PID_FILE_CARGOPIT: &str = "/tmp/cargopit.pid";

pub const SIMAPI_DAT_PATH: &str = "/dev/shm/SIMAPI.DAT";

pub const UDEV_RULES_PATH: &str = "/etc/udev/rules.d/69-cargopit.rules";
pub const UDEV_GROUP_INPUT: &str = "input";
pub const UDEV_GROUP_DIALOUT: &str = "dialout";
pub const UDEV_GROUP_UUCP: &str = "uucp";

pub const ENV_XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
pub const ENV_XDG_CACHE_HOME: &str = "XDG_CACHE_HOME";
pub const ENV_XDG_DATA_HOME: &str = "XDG_DATA_HOME";
pub const ENV_XDG_STATE_HOME: &str = "XDG_STATE_HOME";
pub const ENV_HOME: &str = "HOME";
pub const ENV_PATH: &str = "PATH";
pub const ENV_TERM: &str = "TERM";
pub const TERM_DUMB: &str = "dumb";

pub const LOCAL_BIN_DIRNAME: &str = ".local/bin";
pub const BUILD_DIRNAME: &str = "build";
pub const CONF_DIRNAME: &str = "conf";
pub const SHARE_DIRNAME: &str = "share";

pub const MIN_TERMINAL_WIDTH: u16 = 60;
pub const MIN_TERMINAL_HEIGHT: u16 = 20;

pub const STATUS_REFRESH_MS: u64 = 1000;
pub const EVENT_POLL_MS: u64 = 200;
pub const PROCESS_STOP_WAIT_MS: u64 = 1000;
pub const PROCESS_KILL_PAUSE_MS: u64 = 100;
pub const LOG_TAIL_MAX_LINES: usize = 500;
pub const LOG_READ_CHUNK: usize = 8192;
pub const CHILD_OUTPUT_MAX_LINES: usize = 400;

pub const DEFAULT_FPS: i64 = 60;
pub const DEFAULT_BAUD: i64 = 9600;
pub const DEFAULT_AMPFACTOR: f64 = 1.0;
pub const DEFAULT_FANPOWER: f64 = 0.6;
pub const DEFAULT_NUMLIGHTS: i64 = 6;
pub const DEFAULT_NUMLEDS: i64 = 6;
pub const DEFAULT_STARTLED: i64 = 1;
pub const DEFAULT_ENDLED: i64 = 1;
pub const DEFAULT_GRANULARITY: i64 = 1;
pub const DEFAULT_VOLUME: i64 = 70;
pub const DEFAULT_PAN: i64 = 0;
pub const DEFAULT_CHANNELS: i64 = 2;
pub const DEFAULT_NOISE: i64 = 0;
pub const DEFAULT_FREQUENCY: i64 = 17;
pub const DEFAULT_FREQUENCY_MAX: i64 = 37;
pub const DEFAULT_AMPLITUDE: i64 = 100;
pub const DEFAULT_AMPLITUDE_MAX: i64 = 100;
pub const DEFAULT_THRESHOLD: f64 = 0.2;
pub const DEFAULT_DURATION: f64 = 0.125;
pub const DEFAULT_ENABLED: bool = true;
pub const DEFAULT_VERBOSITY: u8 = 0;
pub const DEFAULT_TACH_MAX_REVS: i64 = 8000;
pub const VOLUME_MIN: i64 = 0;
pub const VOLUME_MAX: i64 = 100;
pub const GRANULARITY_ALLOWED: [i64; 3] = [1, 2, 4];

pub const NUDGE_INT_SMALL: i64 = 1;
pub const NUDGE_INT_LARGE: i64 = 5;
pub const NUDGE_FLOAT_SMALL: f64 = 0.05;
pub const NUDGE_FLOAT_LARGE: f64 = 0.2;

pub const TAB_DASHBOARD: usize = 0;
pub const TAB_DEVICES: usize = 1;
pub const TAB_SETTINGS: usize = 2;
pub const TAB_LOGS: usize = 3;
pub const TAB_COUNT: usize = 4;
pub const TAB_TITLES: [&str; TAB_COUNT] = ["Dashboard", "Devices", "Settings", "Logs"];

pub const KEY_QUIT: KeyCode = KeyCode::Char('q');
pub const KEY_QUIT_UPPER: KeyCode = KeyCode::Char('Q');
pub const KEY_ESC: KeyCode = KeyCode::Esc;
pub const KEY_TAB: KeyCode = KeyCode::Tab;
pub const KEY_BACK_TAB: KeyCode = KeyCode::BackTab;
pub const KEY_ENTER: KeyCode = KeyCode::Enter;
pub const KEY_UP: KeyCode = KeyCode::Up;
pub const KEY_DOWN: KeyCode = KeyCode::Down;
pub const KEY_LEFT: KeyCode = KeyCode::Left;
pub const KEY_RIGHT: KeyCode = KeyCode::Right;
pub const KEY_J: KeyCode = KeyCode::Char('j');
pub const KEY_K: KeyCode = KeyCode::Char('k');
pub const KEY_H: KeyCode = KeyCode::Char('h');
pub const KEY_L: KeyCode = KeyCode::Char('l');
pub const KEY_G_UPPER: KeyCode = KeyCode::Char('J');
pub const KEY_K_UPPER: KeyCode = KeyCode::Char('K');
pub const KEY_SPACE: KeyCode = KeyCode::Char(' ');
pub const KEY_ADD: KeyCode = KeyCode::Char('a');
pub const KEY_EDIT: KeyCode = KeyCode::Char('e');
pub const KEY_DELETE: KeyCode = KeyCode::Char('d');
pub const KEY_DELETE_UPPER: KeyCode = KeyCode::Char('D');
pub const KEY_DUPLICATE: KeyCode = KeyCode::Char('y');
pub const KEY_SAVE: KeyCode = KeyCode::Char('s');
pub const KEY_TEST: KeyCode = KeyCode::Char('t');
pub const KEY_TEMPLATE: KeyCode = KeyCode::Char('T');
pub const KEY_APPLY: KeyCode = KeyCode::Char('A');
pub const KEY_PLUS: KeyCode = KeyCode::Char('+');
pub const KEY_MINUS: KeyCode = KeyCode::Char('-');
pub const KEY_EQUALS: KeyCode = KeyCode::Char('=');
pub const KEY_PREV_PROFILE: KeyCode = KeyCode::Char('[');
pub const KEY_NEXT_PROFILE: KeyCode = KeyCode::Char(']');
pub const KEY_START: KeyCode = KeyCode::Char('1');
pub const KEY_TAB2: KeyCode = KeyCode::Char('2');
pub const KEY_TAB3: KeyCode = KeyCode::Char('3');
pub const KEY_TAB4: KeyCode = KeyCode::Char('4');
pub const KEY_BACKSPACE: KeyCode = KeyCode::Backspace;
pub const KEY_CONFIRM_YES: KeyCode = KeyCode::Char('y');
pub const KEY_CONFIRM_NO: KeyCode = KeyCode::Char('n');

pub const CLASS_USB: &str = "USB";
pub const CLASS_SOUND: &str = "Sound";
pub const CLASS_SERIAL: &str = "Serial";

pub const TYPE_TACHOMETER: &str = "Tachometer";
pub const TYPE_HAPTIC: &str = "Haptic";
pub const TYPE_WHEEL: &str = "Wheel";
pub const TYPE_SHIFT_LIGHTS: &str = "ShiftLights";
pub const TYPE_SIM_WIND: &str = "SimWind";
pub const TYPE_SIMLEDS: &str = "Simleds";
pub const TYPE_CUSTOM: &str = "Custom";
pub const TYPE_ARDUINO_CUSTOM: &str = "ArduinoCustom";
pub const TYPE_USB_HAPTIC: &str = "UsbHaptic";
pub const TYPE_USB_WHEEL: &str = "UsbWheel";
pub const TYPE_SERIAL_HAPTIC: &str = "SerialHaptic";

pub const KEY_DEVICE: &str = "device";
pub const KEY_TYPE: &str = "type";
pub const KEY_SUBTYPE: &str = "subtype";
pub const KEY_DEVID: &str = "devid";
pub const KEY_DEVPATH: &str = "devpath";
pub const KEY_ENABLED: &str = "enabled";
pub const KEY_FPS: &str = "fps";
pub const KEY_CONFIG: &str = "config";
pub const KEY_EFFECT: &str = "effect";
pub const KEY_TYRE: &str = "tyre";
pub const KEY_MODULATION: &str = "modulation";
pub const KEY_FREQUENCY: &str = "frequency";
pub const KEY_FREQUENCY_MAX: &str = "frequencyMax";
pub const KEY_AMPLITUDE: &str = "amplitude";
pub const KEY_AMPLITUDE_MAX: &str = "amplitudeMax";
pub const KEY_THRESHOLD: &str = "threshold";
pub const KEY_DURATION: &str = "duration";
pub const KEY_MOTORS: &str = "motors";
pub const KEY_BAUD: &str = "baud";
pub const KEY_FANPOWER: &str = "fanpower";
pub const KEY_AMPFACTOR: &str = "ampfactor";
pub const KEY_NUMLEDS: &str = "numleds";
pub const KEY_STARTLED: &str = "startled";
pub const KEY_ENDLED: &str = "endled";
pub const KEY_NUMLIGHTS: &str = "numlights";
pub const KEY_GRANULARITY: &str = "granularity";
pub const KEY_VOLUME: &str = "volume";
pub const KEY_STREAM_VOLUME: &str = "streamVolume";
pub const KEY_PAN: &str = "pan";
pub const KEY_CHANNELS: &str = "channels";
pub const KEY_NOISE: &str = "noise";
pub const KEY_SIM: &str = "sim";
pub const KEY_CAR: &str = "car";
pub const KEY_API: &str = "api";
pub const KEY_DEVICES: &str = "devices";
pub const KEY_CONFIGS: &str = "configs";
pub const KEY_CARS: &str = "cars";
pub const KEY_TYRE0: &str = "tyre0";
pub const KEY_TYRE1: &str = "tyre1";
pub const KEY_TYRE2: &str = "tyre2";
pub const KEY_TYRE3: &str = "tyre3";
pub const KEY_SIMS: &str = "sims";

pub const SUBTYPE_CAMMUS_C5: &str = "CammusC5";
pub const SUBTYPE_CAMMUS_C12: &str = "CammusC12";
pub const SUBTYPE_MOZA_R5: &str = "MozaR5";
pub const SUBTYPE_MOZA_R8: &str = "MozaR8";
pub const SUBTYPE_MOZA_R3: &str = "MozaR3";
pub const SUBTYPE_MOZA_R9: &str = "MozaR9";
pub const SUBTYPE_MOZA_NEW: &str = "MozaNew";
pub const SUBTYPE_CSL_ELITE_V3: &str = "CSLELITEV3PEDALS";
pub const SUBTYPE_SIMAGIC_P1000: &str = "SIMAGICP1000PEDALS";
pub const SUBTYPE_SIMAGIC_GT_NEO: &str = "SIMAGICGTNEO";
pub const SUBTYPE_LOGITECH_G29: &str = "LogitechG29";
pub const SUBTYPE_MOZA_KS_PRO: &str = "MozaKSProWheel";
pub const SUBTYPE_SIMNET_PEDALS: &str = "SIMNETPEDALS";
pub const SUBTYPE_REVBURNER: &str = "Revburner";

pub const USB_HARDWARE_SUBTYPES: &[&str] = &[
    SUBTYPE_CAMMUS_C5,
    SUBTYPE_CAMMUS_C12,
    SUBTYPE_MOZA_R5,
    SUBTYPE_MOZA_R8,
    SUBTYPE_MOZA_R3,
    SUBTYPE_MOZA_R9,
    SUBTYPE_MOZA_NEW,
    SUBTYPE_CSL_ELITE_V3,
    SUBTYPE_SIMAGIC_P1000,
    SUBTYPE_SIMAGIC_GT_NEO,
    SUBTYPE_LOGITECH_G29,
    SUBTYPE_MOZA_KS_PRO,
    SUBTYPE_SIMNET_PEDALS,
];

pub const TACHOMETER_SUBTYPES: &[&str] = &[SUBTYPE_REVBURNER];

pub const USB_TYPES: &[&str] = &[TYPE_TACHOMETER, TYPE_HAPTIC, TYPE_WHEEL];
pub const SOUND_TYPES: &[&str] = &[TYPE_HAPTIC];
pub const SERIAL_TYPES: &[&str] = &[
    TYPE_SHIFT_LIGHTS,
    TYPE_SIM_WIND,
    TYPE_HAPTIC,
    TYPE_WHEEL,
    TYPE_SIMLEDS,
    TYPE_CUSTOM,
];

pub const EFFECT_ENGINE: &str = "Engine";
pub const EFFECT_GEAR: &str = "Gear";
pub const EFFECT_ABS: &str = "ABS";
pub const EFFECT_TYRE_SLIP: &str = "TyreSlip";
pub const EFFECT_TYRE_LOCK: &str = "TyreLock";
pub const EFFECT_SUSPENSION: &str = "Suspension";
pub const EFFECTS: &[&str] = &[
    EFFECT_ENGINE,
    EFFECT_GEAR,
    EFFECT_ABS,
    EFFECT_TYRE_SLIP,
    EFFECT_TYRE_LOCK,
    EFFECT_SUSPENSION,
];

pub const MODULATION_NONE: &str = "None";
pub const MODULATION_FREQUENCY: &str = "Frequency";
pub const MODULATION_AMPLITUDE: &str = "Amplitude";
pub const MODULATIONS: &[&str] = &[MODULATION_NONE, MODULATION_FREQUENCY, MODULATION_AMPLITUDE];
pub const MODULATION_FREQUENCY_ALT: &str = "frequency";

pub const TYRE_FRONT_LEFT: &str = "FrontLeft";
pub const TYRE_FRONT_RIGHT: &str = "FrontRight";
pub const TYRE_REAR_LEFT: &str = "RearLeft";
pub const TYRE_REAR_RIGHT: &str = "RearRight";
pub const TYRE_FRONTS: &str = "Fronts";
pub const TYRE_REARS: &str = "Rears";
pub const TYRE_ALL: &str = "All";
pub const TYRES: &[&str] = &[
    TYRE_FRONT_LEFT,
    TYRE_FRONT_RIGHT,
    TYRE_REAR_LEFT,
    TYRE_REAR_RIGHT,
    TYRE_FRONTS,
    TYRE_REARS,
    TYRE_ALL,
];

pub const MOTOR_LABELS: &[&str] = &[
    "M1",
    "M2",
    "M3",
    "M4",
    "M1+M4",
    "M2+M4",
    "M3+M4",
    "M1+M2",
    "M1+M3",
    "M2+M3",
    "M1+M2+M3+M4",
    "M1+M2+M3",
    "M2+M3+M4",
    "M1+M2+M4",
    "M1+M3+M4",
];
pub const MOTOR_COUNT: i64 = 15;

pub const DEFAULT_SIM: &str = "default";
pub const DEFAULT_CAR: &str = "default";
pub const PROFILE_SIMS: &[&str] = &[
    "default", "ac", "acc", "ace", "ams2", "et", "at", "rf2", "all",
];

pub const SERIAL_DEV_PREFIXES: &[&str] = &["ttyUSB", "ttyACM", "ttyS"];
pub const SERIAL_UDEV_GLOB: &str = "/dev/simdev*";
pub const DEV_DIR: &str = "/dev";
pub const SYSFS_HID_DEVICES: &str = "/sys/bus/usb/devices";
pub const SYSFS_HIDRAW: &str = "/sys/class/hidraw";
pub const CSL_RUMBLE_GLOB: &str = "/sys/module/hid_fanatec/drivers/hid:f*/0003:0EB7:183B.*/rumble";

pub const PACTL_BIN: &str = "pactl";
pub const PACTL_LIST_SINKS: &[&str] = &["list", "sinks"];
pub const PGREP_BIN: &str = "pgrep";
pub const PGREP_EXACT: &str = "-x";
pub const PKILL_BIN: &str = "pkill";
pub const PKILL_SIGNAL_TERM: &str = "-TERM";
pub const PKILL_SIGNAL_KILL: &str = "-9";
pub const PS_BIN: &str = "ps";
pub const PS_LIST_ARGS: &[&str] = &["axo", "pid,comm,args"];
pub const PS_COMM_ARGS: &[&str] = &["-o", "comm="];
pub const PS_PID_FLAG: &str = "-p";
pub const KILL_BIN: &str = "kill";
pub const KILL_TERM: &str = "-TERM";
pub const ID_BIN: &str = "id";
pub const ID_GROUPS: &str = "-nG";
pub const WHICH_CARGOPIT_PLAY: &str = "play";
pub const WHICH_CARGOPIT_TEST: &str = "test";
pub const CLI_FLAG_VERBOSE: &str = "-v";
pub const CLI_FLAG_VERY_VERBOSE: &str = "-vv";
pub const CLI_FLAG_DISABLE_AUDIO: &str = "--disable_audio";
pub const CLI_FLAG_UDP: &str = "--udp";
pub const CLI_FLAG_FPS: &str = "--fps";
pub const CLI_FLAG_CONFIG_FILE: &str = "--config-file";
pub const CLI_FLAG_LOG: &str = "--log";
pub const CLI_CONFIG_TACHOMETER: &str = "config";
pub const CLI_TACHOMETER: &str = "tachometer";
pub const CLI_FLAG_MAX_REVS: &str = "-m";
pub const CLI_FLAG_GRANULARITY: &str = "-g";
pub const CLI_FLAG_SAVEFILE: &str = "-s";

pub const STATUS_RUNNING: &str = "RUNNING";
pub const STATUS_STOPPED: &str = "STOPPED";
pub const PRESENCE_CONNECTED: &str = "connected";
pub const PRESENCE_MISSING: &str = "missing";
pub const PRESENCE_UNKNOWN: &str = "unknown";

pub const ACTION_START: &str = "Start";
pub const ACTION_TEST: &str = "Test";
pub const ACTION_RESTART: &str = "Restart";
pub const ACTION_STOP: &str = "Stop";

pub const MSG_STARTED_PLAY: &str = "Started cargopit play";
pub const MSG_STARTED_TEST: &str = "Started cargopit test";
pub const MSG_PLAY_ALREADY_RUNNING: &str = "cargopit play is already running";
pub const MSG_NO_SERVICES: &str = "No running services found to stop";

pub const PROFILE_FIELD_SIM: usize = 0;
pub const PROFILE_FIELD_CAR: usize = 1;
pub const PROFILE_FIELD_COUNT: usize = 2;

pub const TOO_SMALL_TITLE: &str = "Terminal too small";
pub const TUNING_OFFLINE_HINT: &str =
    "Offline tune: save writes disk. Apply restarts play if it was running. Live IPC is not implemented.";
pub const RAW_VIEW_HINT: &str = "Read-only view of the last on-disk file (comments are lost on save).";

pub const LAYOUT_HEADER_HEIGHT: u16 = 3;
pub const LAYOUT_FOOTER_HEIGHT: u16 = 2;
pub const LAYOUT_TAB_HEIGHT: u16 = 3;
pub const LAYOUT_STATUS_HEIGHT: u16 = 1;
pub const LAYOUT_BORDER_LINES: u16 = 2;
pub const LAYOUT_DASHBOARD_INFO_MIN: u16 = 6;
pub const LIST_HIGHLIGHT_SYMBOL: &str = "> ";

pub const ATOMIC_SAVE_SUFFIX: &str = ".tmp";

pub const BUNDLED_LUA: &[&str] = &[
    "basic_rpms.lua",
    "rpms_and_flags.lua",
    "rpms_and_radar.lua",
    "car_radar.lua",
    "custom.lua",
    "neo.lua",
];

pub const SIMD_FIELD_NAME: &str = "name";
pub const SIMD_FIELD_GAMEID: &str = "gameid";
pub const SIMD_FIELD_LAUNCHEXE: &str = "launchexe";
pub const SIMD_FIELD_LIVEEXE: &str = "liveexe";
pub const SIMD_FIELD_BRIDGEDELAY: &str = "bridgedelay";
pub const SIMD_FIELD_SIMAPI: &str = "simapi";
pub const SIMD_FIELD_USEUDP: &str = "useudp";
pub const SIMD_FIELD_UDP_FORWARD: &str = "udp_forward";

pub const SIMD_FIELD_HELP: &[(&str, &str)] = &[
    (SIMD_FIELD_NAME, "Simulator title as simd matches it"),
    (SIMD_FIELD_GAMEID, "Steam app id for shm compatibility matching"),
    (SIMD_FIELD_LAUNCHEXE, "Windows exe name when the title launches"),
    (SIMD_FIELD_LIVEEXE, "Windows exe name while the session is live"),
    (SIMD_FIELD_BRIDGEDELAY, "Seconds to wait for the shm bridge (default 5)"),
    (SIMD_FIELD_SIMAPI, "SimulatorAPI enum value from simapi.h"),
    (SIMD_FIELD_USEUDP, "Force UDP telemetry for this title when supported"),
    (SIMD_FIELD_UDP_FORWARD, "Extra UDP copies of packets simd receives (ip/port list)"),
];

pub const SETTINGS_ITEMS: &[&str] = &[
    "Play / test flags",
    "simd.config",
    "Lua scripts",
    "Tachometer calibration",
    "Tyre diameters",
    "Diagnostics",
    "Raw cargopit.config (read-only)",
];

pub const DASHBOARD_ACTIONS: &[&str] = &[ACTION_START, ACTION_TEST, ACTION_RESTART, ACTION_STOP];
