use crate::config::DeviceEntry;
use crate::consts;
use crate::libconfig::Value;
use crate::schema::{self, DeviceClass};

#[derive(Debug, Clone, Copy)]
pub struct Template {
    pub name: &'static str,
    pub devices: fn() -> Vec<DeviceEntry>,
}

pub fn all() -> &'static [Template] {
    &TEMPLATES
}

const TEMPLATES: [Template; 5] = [
    Template {
        name: "Sound: Engine + Gear",
        devices: sound_engine_gear,
    },
    Template {
        name: "Sound: four-corner TyreSlip",
        devices: sound_tyre_slip,
    },
    Template {
        name: "Serial haptic: slip / lock / ABS",
        devices: serial_haptic,
    },
    Template {
        name: "USB CSL Elite V3: ABS / Slip / Lock",
        devices: usb_csl,
    },
    Template {
        name: "Serial Simleds (basic_rpms.lua)",
        devices: serial_simleds,
    },
];

fn sound_base(effect: &str, tyre: Option<&str>) -> DeviceEntry {
    let mut device = DeviceEntry::new();
    schema::apply_defaults(&mut device, DeviceClass::Sound, consts::TYPE_HAPTIC);
    device.set_str(consts::KEY_EFFECT, effect);
    if let Some(tyre) = tyre {
        device.set_str(consts::KEY_TYRE, tyre);
    }
    device
}

fn sound_engine_gear() -> Vec<DeviceEntry> {
    vec![
        sound_base(consts::EFFECT_ENGINE, None),
        sound_base(consts::EFFECT_GEAR, None),
    ]
}

fn sound_tyre_slip() -> Vec<DeviceEntry> {
    vec![
        sound_base(consts::EFFECT_TYRE_SLIP, Some(consts::TYRE_FRONT_LEFT)),
        sound_base(consts::EFFECT_TYRE_SLIP, Some(consts::TYRE_FRONT_RIGHT)),
        sound_base(consts::EFFECT_TYRE_SLIP, Some(consts::TYRE_REAR_LEFT)),
        sound_base(consts::EFFECT_TYRE_SLIP, Some(consts::TYRE_REAR_RIGHT)),
    ]
}

fn serial_device(effect: &str, motors: i64) -> DeviceEntry {
    let mut device = DeviceEntry::new();
    schema::apply_defaults(&mut device, DeviceClass::Serial, consts::TYPE_HAPTIC);
    device.set_str(consts::KEY_EFFECT, effect);
    device.set_str(consts::KEY_TYRE, consts::TYRE_ALL);
    device.set(consts::KEY_MOTORS, Value::Int(motors));
    device
}

fn serial_haptic() -> Vec<DeviceEntry> {
    vec![
        serial_device(consts::EFFECT_TYRE_SLIP, 0),
        serial_device(consts::EFFECT_TYRE_LOCK, 2),
        serial_device(consts::EFFECT_ABS, 8),
    ]
}

fn usb_csl_device(effect: &str) -> DeviceEntry {
    let mut device = DeviceEntry::new();
    schema::apply_defaults(&mut device, DeviceClass::Usb, consts::TYPE_HAPTIC);
    device.set_str(consts::KEY_SUBTYPE, consts::SUBTYPE_CSL_ELITE_V3);
    device.set_str(consts::KEY_EFFECT, effect);
    device.set_str(consts::KEY_TYRE, consts::TYRE_ALL);
    device
}

fn usb_csl() -> Vec<DeviceEntry> {
    vec![
        usb_csl_device(consts::EFFECT_ABS),
        usb_csl_device(consts::EFFECT_TYRE_SLIP),
        usb_csl_device(consts::EFFECT_TYRE_LOCK),
    ]
}

fn serial_simleds() -> Vec<DeviceEntry> {
    let mut device = DeviceEntry::new();
    schema::apply_defaults(&mut device, DeviceClass::Serial, consts::TYPE_SIMLEDS);
    if let Some(path) = crate::paths::find_bundled_file(consts::BUNDLED_LUA[0]) {
        device.set_str(consts::KEY_CONFIG, path.to_string_lossy().into_owned());
    } else {
        device.set_str(consts::KEY_CONFIG, consts::BUNDLED_LUA[0]);
    }
    vec![device]
}
