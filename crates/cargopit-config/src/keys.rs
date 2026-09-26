//! Config file names, keys, and XDG path segments shared by the host and the TUI.

pub const CONFIG_FILE_NAME: &str = "cargopit.config";
pub const CONFIG_DIR_NAME: &str = "cargopit";
pub const SIMD_CONFIG_DIR_NAME: &str = "simd";
pub const SIMD_CONFIG_FILE_NAME: &str = "simd.config";
pub const DIAMETERS_FILE_NAME: &str = "diameters.config";
pub const TUI_STATE_FILE_NAME: &str = "tui-state.json";
pub const CACHE_DIR_NAME: &str = "cargopit";

pub const ENV_XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
pub const ENV_XDG_CACHE_HOME: &str = "XDG_CACHE_HOME";
pub const ENV_XDG_DATA_HOME: &str = "XDG_DATA_HOME";
pub const ENV_XDG_STATE_HOME: &str = "XDG_STATE_HOME";
pub const ENV_HOME: &str = "HOME";
pub const ENV_PATH: &str = "PATH";
pub const XDG_CONFIG_FALLBACK: &str = ".config";
pub const XDG_CACHE_FALLBACK: &str = ".cache";
pub const XDG_DATA_FALLBACK: &str = ".local/share";
pub const XDG_STATE_FALLBACK: &str = ".local/state";
pub const PATH_SEPARATOR: &str = ":";

pub const LOCAL_BIN_DIRNAME: &str = ".local/bin";
pub const BUILD_DIRNAME: &str = "build";
pub const CONF_DIRNAME: &str = "conf";
pub const SHARE_DIRNAME: &str = "share";
pub const USR_PREFIX: &str = "/usr";

pub const CLASS_USB: &str = "USB";
pub const CLASS_SOUND: &str = "Sound";
pub const CLASS_SERIAL: &str = "Serial";
pub const DEVICE_CLASS_COUNT: usize = 3;

pub const TYPE_TACHOMETER: &str = "Tachometer";
pub const TYPE_HAPTIC: &str = "Haptic";
pub const TYPE_WHEEL: &str = "Wheel";
pub const TYPE_SIMLEDS: &str = "Simleds";

pub const KEY_DEVICE: &str = "device";
pub const KEY_TYPE: &str = "type";
pub const KEY_SUBTYPE: &str = "subtype";
pub const KEY_DEVID: &str = "devid";
pub const KEY_DEVPATH: &str = "devpath";
pub const KEY_ENABLED: &str = "enabled";
pub const KEY_FPS: &str = "fps";
pub const KEY_CONFIG: &str = "config";
pub const KEY_GRANULARITY: &str = "granularity";
pub const CONFIG_VALUE_NONE: &str = "none";
pub const REVBURNER_XML_NAME: &str = "revburner.xml";
pub const KEY_EFFECT: &str = "effect";
pub const KEY_TYRE: &str = "tyre";
pub const KEY_THRESHOLD: &str = "threshold";
pub const KEY_BAUD: &str = "baud";
pub const KEY_AMPFACTOR: &str = "ampfactor";
pub const KEY_NUMLEDS: &str = "numleds";
pub const KEY_STARTLED: &str = "startled";
pub const KEY_ENDLED: &str = "endled";
pub const KEY_SIM: &str = "sim";
pub const KEY_CAR: &str = "car";
pub const KEY_CARS: &str = "cars";
pub const KEY_TYRE0: &str = "tyre0";
pub const KEY_TYRE1: &str = "tyre1";
pub const KEY_TYRE2: &str = "tyre2";
pub const KEY_TYRE3: &str = "tyre3";
pub const KEY_API: &str = "api";
pub const KEY_PROFILE_NAME: &str = "name";
pub const KEY_DEVICES: &str = "devices";
pub const KEY_CONFIGS: &str = "configs";
pub const KEY_SIMS: &str = "sims";

pub const SUBTYPE_MOZA_R9: &str = "MozaR9";
pub const DEFAULT_SIM: &str = "default";
pub const DEFAULT_CAR: &str = "default";
pub const DEFAULT_ENABLED: bool = true;
pub const ATOMIC_SAVE_SUFFIX: &str = ".tmp";
pub const SETTINGS_GAME_IDLE: u64 = 0;

pub const SUMMARY_UNKNOWN_CLASS: &str = "?";
pub const SUMMARY_UNKNOWN_TYPE: &str = "-";

pub const FPS_DEFAULT: i32 = 60;
pub const FPS_MIN: i32 = 1;
pub const FPS_MAX: i32 = 1000;
pub const CONFIG_INDEX_UNSET: i32 = -1;
pub const CONFIG_INDEX_FIRST: i32 = 0;

pub const SIMD_FIELD_NAME: &str = "name";
pub const SIMD_FIELD_GAMEID: &str = "gameid";
pub const SIMD_FIELD_LAUNCHEXE: &str = "launchexe";
pub const SIMD_FIELD_LIVEEXE: &str = "liveexe";
pub const SIMD_FIELD_BRIDGEDELAY: &str = "bridgedelay";
pub const SIMD_FIELD_SIMAPI: &str = "simapi";
pub const SIMD_FIELD_TELEMETRY: &str = "telemetry";
pub const SIMD_FIELD_USEUDP: &str = "useudp";
pub const SIMD_TELEMETRY_AUTO: &str = "auto";
pub const SIMD_TELEMETRY_SHM: &str = "shm";
pub const SIMD_TELEMETRY_UDP: &str = "udp";
pub const SIMD_TELEMETRY_SOURCE_COUNT: usize = 3;
pub const SIMD_TELEMETRY_SOURCES: [&str; SIMD_TELEMETRY_SOURCE_COUNT] =
    [SIMD_TELEMETRY_AUTO, SIMD_TELEMETRY_SHM, SIMD_TELEMETRY_UDP];
pub const SIMD_FIELD_COUNT: usize = 7;
pub const SIMD_TABLE_COLUMNS: [&str; SIMD_FIELD_COUNT] = [
    SIMD_FIELD_NAME,
    SIMD_FIELD_GAMEID,
    SIMD_FIELD_LAUNCHEXE,
    SIMD_FIELD_LIVEEXE,
    SIMD_FIELD_BRIDGEDELAY,
    SIMD_FIELD_SIMAPI,
    SIMD_FIELD_TELEMETRY,
];
pub const SIMD_VALUE_COMPLEX: &str = "?";
pub const SIMD_UNNAMED: &str = "(unnamed)";
pub const SIMD_COL_NAME_MIN: u16 = 16;
pub const SIMD_COL_GAMEID: u16 = 7;
pub const SIMD_COL_EXE_MIN: u16 = 10;
pub const SIMD_COL_BRIDGE: u16 = 11;
pub const SIMD_COL_SIMAPI: u16 = 6;
pub const SIMD_COL_TELEMETRY: u16 = 9;
pub const SIMD_COL_EXTRA_MIN: u16 = 8;
pub const SIMD_COL_SPACING: u16 = 1;

pub const SIMD_FIELD_HELP: &[(&str, &str)] = &[
    (SIMD_FIELD_NAME, "Simulator title as simd matches it"),
    (
        SIMD_FIELD_GAMEID,
        "Steam app id for shm compatibility matching",
    ),
    (
        SIMD_FIELD_LAUNCHEXE,
        "Windows exe name when the title launches",
    ),
    (
        SIMD_FIELD_LIVEEXE,
        "Windows exe name while the session is live",
    ),
    (
        SIMD_FIELD_BRIDGEDELAY,
        "Seconds to wait for the shm bridge (default 5)",
    ),
    (SIMD_FIELD_SIMAPI, "SimulatorAPI enum value from simapi.h"),
    (
        SIMD_FIELD_TELEMETRY,
        "How play reads this title: auto, shm (POSIX /dev/shm), or udp. ACR shm needs the Proton helper to mirror Local\\acpmf_physics.",
    ),
];

pub const TACH_ELEMENT_SETTINGS_ITEM: &str = "SettingsItem";
pub const TACH_ELEMENT_VALUE: &str = "Value";
pub const TACH_ELEMENT_TIME_VALUE: &str = "TimeValue";
