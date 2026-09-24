//! Case-insensitive config names. The first entry for a value is the one written back.

use crate::keys;

pub const DEVICE_USB: i32 = 0;
pub const DEVICE_SOUND: i32 = 1;
pub const DEVICE_SERIAL: i32 = 2;

pub const SUBTYPE_UNKNOWN: i32 = 0;
pub const SUBTYPE_TACHOMETER: i32 = 2;
pub const SUBTYPE_USB_HAPTIC: i32 = 3;
pub const SUBTYPE_USB_WHEEL: i32 = 4;
pub const SUBTYPE_SHIFT_LIGHTS: i32 = 5;
pub const SUBTYPE_SIM_WIND: i32 = 6;
pub const SUBTYPE_SERIAL_HAPTIC: i32 = 7;
pub const SUBTYPE_SERIAL_WHEEL: i32 = 8;
pub const SUBTYPE_SIMLED: i32 = 9;
pub const SUBTYPE_ARDUINO_CUSTOM: i32 = 10;
pub const SUBTYPE_SOUND_HAPTIC: i32 = 1;

pub const HARDWARE_UNKNOWN: i32 = 0;
pub const HARDWARE_CAMMUS_C5: i32 = 1;
pub const HARDWARE_CAMMUS_C12: i32 = 2;
pub const HARDWARE_MOZA_R5: i32 = 3;
pub const HARDWARE_CSL_ELITE: i32 = 4;
pub const HARDWARE_SIMAGIC_P1000: i32 = 5;
pub const HARDWARE_SIMAGIC_GT_NEO: i32 = 6;
pub const HARDWARE_MOZA_NEW: i32 = 7;
pub const HARDWARE_LOGITECH_G29: i32 = 8;
pub const HARDWARE_MOZA_KS_PRO: i32 = 9;
pub const HARDWARE_SIMNET: i32 = 10;
pub const HARDWARE_REVBURNER: i32 = 11;

pub const EFFECT_ENGINE: i32 = 0;
pub const EFFECT_GEAR: i32 = 1;
pub const EFFECT_ABS: i32 = 2;
pub const EFFECT_TYRE_SLIP: i32 = 3;
pub const EFFECT_TYRE_LOCK: i32 = 4;
pub const EFFECT_SUSPENSION: i32 = 5;

pub const TYRE_FRONT_LEFT: i32 = 0;
pub const TYRE_FRONT_RIGHT: i32 = 1;
pub const TYRE_REAR_LEFT: i32 = 2;
pub const TYRE_REAR_RIGHT: i32 = 3;
pub const TYRE_FRONTS: i32 = 4;
pub const TYRE_REARS: i32 = 5;
pub const TYRE_ALL_FOUR: i32 = 6;

pub const MODULATION_NONE: i32 = 0;
pub const MODULATION_FREQUENCY: i32 = 1;
pub const MODULATION_AMPLIFY: i32 = 2;

pub const ERROR_NONE: i32 = 0;
pub const ERROR_INVALID_DEV: i32 = 3;

#[derive(Clone, Copy)]
pub struct NameEntry {
    pub name: &'static str,
    pub value: i32,
}

pub const DEVICE_CLASSES: &[NameEntry] = &[
    NameEntry {
        name: keys::CLASS_USB,
        value: DEVICE_USB,
    },
    NameEntry {
        name: keys::CLASS_SOUND,
        value: DEVICE_SOUND,
    },
    NameEntry {
        name: keys::CLASS_SERIAL,
        value: DEVICE_SERIAL,
    },
];

pub const USB_TYPES: &[NameEntry] = &[
    NameEntry {
        name: keys::TYPE_TACHOMETER,
        value: SUBTYPE_TACHOMETER,
    },
    NameEntry {
        name: "UsbWheel",
        value: SUBTYPE_USB_WHEEL,
    },
    NameEntry {
        name: keys::TYPE_WHEEL,
        value: SUBTYPE_USB_WHEEL,
    },
    NameEntry {
        name: "UsbHaptic",
        value: SUBTYPE_USB_HAPTIC,
    },
    NameEntry {
        name: keys::TYPE_HAPTIC,
        value: SUBTYPE_USB_HAPTIC,
    },
];

pub const SERIAL_TYPES: &[NameEntry] = &[
    NameEntry {
        name: "ShiftLights",
        value: SUBTYPE_SHIFT_LIGHTS,
    },
    NameEntry {
        name: keys::TYPE_SIMLEDS,
        value: SUBTYPE_SIMLED,
    },
    NameEntry {
        name: "ArduinoCustom",
        value: SUBTYPE_ARDUINO_CUSTOM,
    },
    NameEntry {
        name: "Custom",
        value: SUBTYPE_ARDUINO_CUSTOM,
    },
    NameEntry {
        name: "SimWind",
        value: SUBTYPE_SIM_WIND,
    },
    NameEntry {
        name: "SerialHaptic",
        value: SUBTYPE_SERIAL_HAPTIC,
    },
    NameEntry {
        name: keys::TYPE_HAPTIC,
        value: SUBTYPE_SERIAL_HAPTIC,
    },
    NameEntry {
        name: keys::TYPE_WHEEL,
        value: SUBTYPE_SERIAL_WHEEL,
    },
];

pub const SOUND_TYPES: &[NameEntry] = &[NameEntry {
    name: keys::TYPE_HAPTIC,
    value: SUBTYPE_SOUND_HAPTIC,
}];

pub const HARDWARE: &[NameEntry] = &[
    NameEntry {
        name: "CammusC5",
        value: HARDWARE_CAMMUS_C5,
    },
    NameEntry {
        name: "CammusC12",
        value: HARDWARE_CAMMUS_C12,
    },
    NameEntry {
        name: "MozaR5",
        value: HARDWARE_MOZA_R5,
    },
    NameEntry {
        name: "MozaR8",
        value: HARDWARE_MOZA_R5,
    },
    NameEntry {
        name: "MozaR3",
        value: HARDWARE_MOZA_R5,
    },
    NameEntry {
        name: "MozaNew",
        value: HARDWARE_MOZA_NEW,
    },
    NameEntry {
        name: keys::SUBTYPE_MOZA_R9,
        value: HARDWARE_MOZA_NEW,
    },
    NameEntry {
        name: "MozaKSProWheel",
        value: HARDWARE_MOZA_KS_PRO,
    },
    NameEntry {
        name: "LogitechG29",
        value: HARDWARE_LOGITECH_G29,
    },
    NameEntry {
        name: "CSLELITEV3PEDALS",
        value: HARDWARE_CSL_ELITE,
    },
    NameEntry {
        name: "SIMNETPEDALS",
        value: HARDWARE_SIMNET,
    },
    NameEntry {
        name: "SIMAGICP1000PEDALS",
        value: HARDWARE_SIMAGIC_P1000,
    },
    NameEntry {
        name: "SIMAGICGTNEO",
        value: HARDWARE_SIMAGIC_GT_NEO,
    },
    NameEntry {
        name: "Revburner",
        value: HARDWARE_REVBURNER,
    },
];

pub const EFFECTS: &[NameEntry] = &[
    NameEntry {
        name: "Engine",
        value: EFFECT_ENGINE,
    },
    NameEntry {
        name: "Gear",
        value: EFFECT_GEAR,
    },
    NameEntry {
        name: "ABS",
        value: EFFECT_ABS,
    },
    NameEntry {
        name: "TyreSlip",
        value: EFFECT_TYRE_SLIP,
    },
    NameEntry {
        name: "Slip",
        value: EFFECT_TYRE_SLIP,
    },
    NameEntry {
        name: "TireSlip",
        value: EFFECT_TYRE_SLIP,
    },
    NameEntry {
        name: "TyreLock",
        value: EFFECT_TYRE_LOCK,
    },
    NameEntry {
        name: "Lock",
        value: EFFECT_TYRE_LOCK,
    },
    NameEntry {
        name: "TireLock",
        value: EFFECT_TYRE_LOCK,
    },
    NameEntry {
        name: "Suspension",
        value: EFFECT_SUSPENSION,
    },
];

pub const TYRES: &[NameEntry] = &[
    NameEntry {
        name: "FrontLeft",
        value: TYRE_FRONT_LEFT,
    },
    NameEntry {
        name: "FrontRight",
        value: TYRE_FRONT_RIGHT,
    },
    NameEntry {
        name: "RearLeft",
        value: TYRE_REAR_LEFT,
    },
    NameEntry {
        name: "RearRight",
        value: TYRE_REAR_RIGHT,
    },
    NameEntry {
        name: "Fronts",
        value: TYRE_FRONTS,
    },
    NameEntry {
        name: "Front",
        value: TYRE_FRONTS,
    },
    NameEntry {
        name: "Rears",
        value: TYRE_REARS,
    },
    NameEntry {
        name: "Rear",
        value: TYRE_REARS,
    },
    NameEntry {
        name: "All",
        value: TYRE_ALL_FOUR,
    },
];

pub const MODULATIONS: &[NameEntry] = &[
    NameEntry {
        name: "None",
        value: MODULATION_NONE,
    },
    NameEntry {
        name: "Frequency",
        value: MODULATION_FREQUENCY,
    },
    NameEntry {
        name: "Amplitude",
        value: MODULATION_AMPLIFY,
    },
    NameEntry {
        name: "Amplify",
        value: MODULATION_AMPLIFY,
    },
];

pub fn lookup(table: &[NameEntry], name: &str) -> Option<i32> {
    if name.is_empty() {
        return None;
    }
    table
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
        .map(|entry| entry.value)
}

pub fn name_for(table: &[NameEntry], value: i32) -> Option<&'static str> {
    table
        .iter()
        .find(|entry| entry.value == value)
        .map(|entry| entry.name)
}

pub fn tyre_or_all_four(name: &str) -> i32 {
    lookup(TYRES, name).unwrap_or(TYRE_ALL_FOUR)
}

pub fn map_device(class_name: &str, type_name: &str) -> Result<i32, i32> {
    let class = lookup(DEVICE_CLASSES, class_name).ok_or(ERROR_INVALID_DEV)?;
    let table = match class {
        DEVICE_USB => USB_TYPES,
        DEVICE_SERIAL => SERIAL_TYPES,
        DEVICE_SOUND => SOUND_TYPES,
        _ => return Err(ERROR_INVALID_DEV),
    };
    if class == DEVICE_SOUND {
        return Ok(SUBTYPE_SOUND_HAPTIC);
    }
    lookup(table, type_name).ok_or(ERROR_INVALID_DEV)
}

pub fn clamp_fps(fps: i32) -> i32 {
    if fps < keys::FPS_MIN {
        return keys::FPS_MIN;
    }
    if fps > keys::FPS_MAX {
        return keys::FPS_MAX;
    }
    fps
}

pub fn resolve_profile_index(profile_count: usize, requested: i32) -> i32 {
    if profile_count == 0 {
        return keys::CONFIG_INDEX_UNSET;
    }
    if requested < 0 {
        return keys::CONFIG_INDEX_FIRST;
    }
    if requested as usize >= profile_count {
        return keys::CONFIG_INDEX_UNSET;
    }
    requested
}
