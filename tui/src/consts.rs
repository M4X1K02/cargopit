pub const APP_TITLE: &str = "Cargopit";
pub const BINARY_NAME: &str = "cargopit-tui";
pub const PROGRAM_NAME: &str = "monocoque";

pub const MIN_TERMINAL_WIDTH: u16 = 60;
pub const MIN_TERMINAL_HEIGHT: u16 = 20;

pub const TAB_DASHBOARD: usize = 0;
pub const TAB_DEVICES: usize = 1;
pub const TAB_LOGS: usize = 2;
pub const TAB_COUNT: usize = 3;
pub const TAB_LABELS: [&str; TAB_COUNT] = ["Dashboard", "Devices", "Logs"];

pub const SIMD_PROCESS_NAME: &str = "simd";
pub const MONOCOQUE_PROCESS_NAME: &str = "monocoque";
pub const SIMD_PID_FILE: &str = "/tmp/simd.pid";
pub const MONOCOQUE_PID_FILE: &str = "/tmp/monocoque.pid";
pub const SIMAPI_SHM_PATH: &str = "/dev/shm/SIMAPI.DAT";

pub const CONFIG_FILE_NAME: &str = "monocoque.config";
pub const LOG_FILE_EXTENSION: &str = "log";

pub const PROCESS_STOP_WAIT_MS: u64 = 1000;
pub const TICK_INTERVAL_MS: u64 = 250;
pub const MAX_LOG_LINES: usize = 1000;

pub const DEFAULT_FPS: u32 = 60;
pub const DEFAULT_BAUD: u32 = 115200;
pub const DEFAULT_VOLUME: u32 = 100;
pub const DEFAULT_CHANNELS: u32 = 2;
pub const DEFAULT_PAN: u32 = 0;
pub const DEFAULT_NOISE: u32 = 0;
pub const DEFAULT_FREQUENCY: u32 = 32;
pub const DEFAULT_AMPLITUDE: u32 = 100;
pub const DEFAULT_FREQUENCY_MAX: u32 = 0;
pub const DEFAULT_THRESHOLD: f64 = 0.0;
pub const DEFAULT_DURATION: f64 = 0.0;
pub const DEFAULT_FANPOWER: f64 = 0.6;
pub const DEFAULT_AMPFACTOR: f64 = 1.0;
pub const DEFAULT_NUMLEDS: u32 = 6;
pub const DEFAULT_STARTLED: u32 = 0;
pub const DEFAULT_ENDLED: u32 = 0;
pub const DEFAULT_GRANULARITY: u32 = 1;

pub const KEY_QUIT: char = 'q';
pub const KEY_TAB_DASHBOARD: char = '1';
pub const KEY_TAB_DEVICES: char = '2';
pub const KEY_TAB_LOGS: char = '3';
pub const KEY_UP: char = 'k';
pub const KEY_DOWN: char = 'j';
pub const KEY_LEFT: char = 'h';
pub const KEY_RIGHT: char = 'l';
pub const KEY_ADD: char = 'a';
pub const KEY_EDIT: char = 'e';
pub const KEY_DELETE: char = 'd';
pub const KEY_SAVE: char = 's';
pub const KEY_TEST: char = 't';
pub const KEY_CONFIRM_YES: char = 'y';
pub const KEY_CONFIRM_NO: char = 'n';

pub const ACTION_START: usize = 0;
pub const ACTION_TEST: usize = 1;
pub const ACTION_RESTART: usize = 2;
pub const ACTION_STOP: usize = 3;
pub const DASHBOARD_ACTION_COUNT: usize = 4;
pub const DASHBOARD_ACTION_LABELS: [&str; DASHBOARD_ACTION_COUNT] = [
    "Start",
    "Test configuration",
    "Restart services",
    "Stop all",
];

pub const CONFIGS_KEY: &str = "configs";
pub const DEVICES_KEY: &str = "devices";
pub const SIM_KEY: &str = "sim";
pub const CAR_KEY: &str = "car";
pub const API_KEY: &str = "api";
pub const DEVICE_CLASS_KEY: &str = "device";
pub const DEVICE_TYPE_KEY: &str = "type";
pub const DEVICE_SUBTYPE_KEY: &str = "subtype";
pub const DEVID_KEY: &str = "devid";
pub const DEVPATH_KEY: &str = "devpath";
pub const ENABLED_KEY: &str = "enabled";
pub const FPS_KEY: &str = "fps";
pub const CONFIG_PATH_KEY: &str = "config";
pub const EFFECT_KEY: &str = "effect";
pub const TYRE_KEY: &str = "tyre";
pub const MODULATION_KEY: &str = "modulation";
pub const FREQUENCY_KEY: &str = "frequency";
pub const FREQUENCY_MAX_KEY: &str = "frequencyMax";
pub const AMPLITUDE_KEY: &str = "amplitude";
pub const AMPLITUDE_MAX_KEY: &str = "amplitudeMax";
pub const THRESHOLD_KEY: &str = "threshold";
pub const DURATION_KEY: &str = "duration";
pub const BAUD_KEY: &str = "baud";
pub const VOLUME_KEY: &str = "volume";
pub const STREAM_VOLUME_KEY: &str = "streamVolume";
pub const PAN_KEY: &str = "pan";
pub const CHANNELS_KEY: &str = "channels";
pub const NOISE_KEY: &str = "noise";
pub const NUMLEDS_KEY: &str = "numleds";
pub const STARTLED_KEY: &str = "startled";
pub const ENDLED_KEY: &str = "endled";
pub const FANPOWER_KEY: &str = "fanpower";
pub const AMPFACTOR_KEY: &str = "ampfactor";
pub const GRANULARITY_KEY: &str = "granularity";
pub const MOTORS_KEY: &str = "motors";

pub const CLASS_USB: &str = "USB";
pub const CLASS_SOUND: &str = "Sound";
pub const CLASS_SERIAL: &str = "Serial";

pub const TYPE_TACHOMETER: &str = "Tachometer";
pub const TYPE_HAPTIC: &str = "Haptic";
pub const TYPE_WHEEL: &str = "Wheel";
pub const TYPE_SHIFT_LIGHTS: &str = "ShiftLights";
pub const TYPE_SIM_WIND: &str = "SimWind";
pub const TYPE_SIM_LED: &str = "Simleds";
pub const TYPE_CUSTOM: &str = "Custom";
pub const TYPE_SOUND_HAPTIC: &str = "SoundHaptic";

pub const USB_TYPES: [&str; 3] = [TYPE_TACHOMETER, TYPE_HAPTIC, TYPE_WHEEL];
pub const SERIAL_TYPES: [&str; 6] = [
    TYPE_SHIFT_LIGHTS,
    TYPE_SIM_WIND,
    TYPE_HAPTIC,
    TYPE_WHEEL,
    TYPE_SIM_LED,
    TYPE_CUSTOM,
];
pub const SOUND_TYPES: [&str; 2] = [TYPE_HAPTIC, TYPE_SOUND_HAPTIC];

pub const USB_HARDWARE_SUBTYPES: [&str; 11] = [
    "CammusC5",
    "CammusC12",
    "MozaR5",
    "MozaR3",
    "MozaR8",
    "MozaNew",
    "LogitechG29",
    "MozaKSProWheel",
    "CSLELITEV3PEDALS",
    "SIMNETPEDALS",
    "SIMAGICP1000PEDALS",
];
pub const USB_TACH_SUBTYPES: [&str; 1] = ["Revburner"];

pub const EFFECT_NAMES: [&str; 6] = [
    "Engine",
    "Gear",
    "ABS",
    "TyreSlip",
    "TyreLock",
    "Suspension",
];
pub const MODULATION_NAMES: [&str; 3] = ["None", "Frequency", "Amplitude"];
pub const TYRE_NAMES: [&str; 7] = [
    "FrontLeft",
    "FrontRight",
    "RearLeft",
    "RearRight",
    "Fronts",
    "Rears",
    "All",
];

pub const DEFAULT_SIM: &str = "default";
pub const DEFAULT_CAR: &str = "default";
pub const DEFAULT_API: &str = "";
pub const DEFAULT_VALUE: &str = "default";
pub const ALL_VALUE: &str = "all";

pub const PACTL_BIN: &str = "pactl";
pub const PACTL_LIST_ARGS: [&str; 3] = ["list", "short", "sinks"];
pub const SERIAL_DEV_DIR: &str = "/dev";
pub const SERIAL_PREFIXES: [&str; 3] = ["ttyUSB", "ttyACM", "ttyS"];

pub const MONOCOQUE_PLAY_ARG: &str = "play";
pub const MONOCOQUE_TEST_ARG: &str = "test";
pub const START_MONOCOQUE_WRAPPER: &str = "start-monocoque";
pub const TEST_MONOCOQUE_WRAPPER: &str = "test-monocoque";

pub const STATUS_RUNNING: &str = "RUNNING";
pub const STATUS_STOPPED: &str = "STOPPED";
pub const STATUS_PRESENT: &str = "present";
pub const STATUS_ABSENT: &str = "absent";

pub const TUNING_STUB_MESSAGE: &str =
    "Live per-device tuning is not implemented yet. This page is the slot later device pages will use.";

pub const FOOTER_TABS: &str = "1/2/3 or Tab: pages | q: quit";
pub const FOOTER_DASHBOARD: &str = "j/k: select | Enter: run | q: quit";
pub const FOOTER_DEVICES: &str =
    "j/k: device | [/]: config | a: add | e: edit | d: delete | Enter: tune | q: quit";
pub const FOOTER_EDIT: &str =
    "j/k: field | h/l: cycle | type to edit text | s: save | t: test | Esc: back";
pub const FOOTER_TUNE: &str = "Esc/Backspace: back to devices";
pub const FOOTER_LOGS: &str = "j/k: scroll | G: bottom | q: quit";
pub const FOOTER_CONFIRM: &str = "y: confirm delete | n/Esc: cancel";
pub const FOOTER_TOO_SMALL: &str = "Resize the terminal to continue";
