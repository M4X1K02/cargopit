//! Configured devices for one play session. A USB tachometer opens the RevBurner
//! and writes its pulse report on each tick. A Moza R9 serial wheel opens its
//! port and writes the new-firmware LED frames.

use std::path::{Path, PathBuf};

use cargopit_config::config::{CargopitConfig, DeviceEntry};
use cargopit_config::keys::{self, CLASS_SERIAL, CLASS_SOUND, CLASS_USB};
use cargopit_config::names;
use cargopit_config::paths;
use cargopit_config::tach::{self, ERR_TACH_XML_EMPTY};
use cargopit_devices::serial::{self, MozaNewWheel};
use cargopit_devices::telemetry::Telemetry;
use cargopit_devices::transport::{RealHid, RealSerial, ShareWarning};
use cargopit_devices::usb::{self, TachPulses};
use cargopit_devices::{tick_interval_ms, DeviceKind, SimDevice, DEFAULT_DEVICE_FPS};

use crate::games;
use crate::log::Level;
use crate::scheduler;
use crate::tyres;

pub const REVBURNER_VENDOR_ID: u16 = 0x04d8;
pub const REVBURNER_PRODUCT_ID: u16 = 0x0102;

const CONFIG_INDEX_FIRST: i32 = 0;
const USE_PULSES_DURING_PLAY: bool = false;
const DEVICE_RUNNER_OVERRUNS: u64 = 0;
const GRANULARITY_MIN: i64 = 0;
const GRANULARITY_MAX: i64 = 4;
const GRANULARITY_REJECTED: i64 = 3;
const GRANULARITY_FALLBACK: i64 = 1;
const PROBE_OPEN_NS: u64 = 0;

pub struct LoadedDevices {
    devices: Vec<SimDevice>,
    updates: Vec<u64>,
    effects: Vec<Option<i32>>,
    ports: Vec<HidPort>,
    tables: Vec<TachMap>,
    serials: Vec<SerialPort>,
    wheels: Vec<Option<MozaNewWheel>>,
}

pub struct InitNotice {
    pub level: Level,
    pub message: String,
}

pub struct ProfileLoad {
    pub devices: LoadedDevices,
    pub setup_notices: Vec<InitNotice>,
    pub notices: Vec<InitNotice>,
}

impl LoadedDevices {
    pub fn from_config(config: &CargopitConfig, requested_index: i32, disable_audio: bool) -> Self {
        let Some(index) = profile_index(config.profiles.len(), requested_index) else {
            return Self::empty();
        };
        open_profile_at(config, index, disable_audio, PROBE_OPEN_NS).devices
    }

    pub fn empty() -> Self {
        Self {
            devices: Vec::new(),
            updates: Vec::new(),
            effects: Vec::new(),
            ports: Vec::new(),
            tables: Vec::new(),
            serials: Vec::new(),
            wheels: Vec::new(),
        }
    }

    pub fn hid_handles(&self) -> usize {
        self.ports
            .iter()
            .filter(|port| matches!(port, HidPort::Live(_)))
            .count()
    }

    pub fn captured_reports(&self, index: usize) -> Option<&[Vec<u8>]> {
        match self.ports.get(index) {
            Some(HidPort::Captured(log)) => Some(log),
            _ => None,
        }
    }

    pub fn captured_serial(&self, index: usize) -> Option<&[Vec<u8>]> {
        match self.serials.get(index) {
            Some(SerialPort::Captured { frames, .. }) => Some(frames),
            _ => None,
        }
    }

    pub fn len(&self) -> usize {
        self.devices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn device(&self, index: usize) -> Option<&SimDevice> {
        self.devices.get(index)
    }

    pub fn updates(&self, index: usize) -> u64 {
        self.updates.get(index).copied().unwrap_or(0)
    }

    pub fn tick(&mut self, index: usize, frame: &Telemetry, now_ns: u64) -> Vec<InitNotice> {
        if let Some(count) = self.updates.get_mut(index) {
            *count = count.wrapping_add(1);
        }
        let mut notices = self.write_tach(index, frame);
        notices.extend(self.write_moza(index, frame, now_ns));
        notices
    }

    pub fn release(&mut self, now_ns: u64) -> Vec<InitNotice> {
        if self.devices.is_empty() {
            return Vec::new();
        }
        let mut notices = Vec::new();
        for index in 0..self.devices.len() {
            let id = i32::try_from(index).unwrap_or(i32::MAX);
            notices.push(notice(
                Level::Info,
                games::device_runner_message(id, self.updates(index), DEVICE_RUNNER_OVERRUNS),
            ));
        }
        self.write_release_zeros(&mut notices);
        self.blank_moza(now_ns, &mut notices);
        self.close_ports(&mut notices);
        notices
    }

    pub fn effect(&self, index: usize) -> Option<i32> {
        self.effects.get(index).copied().flatten()
    }

    pub fn needs_tyre_diameter(&self) -> bool {
        self.effects
            .iter()
            .flatten()
            .any(|effect| tyres::device_needs_tyre_diameter(*effect))
    }

    pub fn interval_ms(&self, index: usize) -> u64 {
        self.devices
            .get(index)
            .map(SimDevice::interval_ms)
            .unwrap_or(tick_interval_ms(DEFAULT_DEVICE_FPS))
    }

    fn write_tach(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        let Some((sample, size)) = self.tach_sample(index, frame) else {
            return Vec::new();
        };
        let mut notices = tach_trace_notices(&sample, size);
        let report = usb::revburner_report(sample.pulses());
        self.write_index(index, &report, &mut notices);
        notices
    }

    fn tach_sample(&self, index: usize, frame: &Telemetry) -> Option<(TachPulses, usize)> {
        let map = self.tables.get(index)?;
        if !map.active {
            return None;
        }
        let sample = usb::lookup_tach_pulses(
            frame.rpms(),
            frame.pulses(),
            map.granularity,
            &map.pulses,
            map.use_pulses,
        )?;
        Some((sample, map.pulses.len()))
    }

    fn write_release_zeros(&mut self, notices: &mut Vec<InitNotice>) {
        let report = usb::revburner_report(0);
        let active: Vec<usize> = self
            .tables
            .iter()
            .enumerate()
            .filter(|(_, map)| map.active)
            .map(|(index, _)| index)
            .collect();
        for index in active {
            self.write_index(index, &report, notices);
        }
    }

    fn write_index(&mut self, index: usize, report: &[u8], notices: &mut Vec<InitNotice>) {
        let Some(port) = self.ports.get_mut(index) else {
            return;
        };
        write_port(port, report, notices);
    }

    fn write_moza(&mut self, index: usize, frame: &Telemetry, now_ns: u64) -> Vec<InitNotice> {
        let step = {
            let Some(wheel) = self.wheels.get_mut(index).and_then(Option::as_mut) else {
                return Vec::new();
            };
            wheel.tick(frame, now_ns)
        };
        let mut notices = moza_step_notices(&step);
        let wrote = write_serial_frames(self.serials.get_mut(index), &step.frames);
        if step.rpm_sent && !wrote {
            disarm_wheel(&mut self.wheels, index);
            notices.push(notice(Level::Warn, games::MSG_MOZA_RPM_FAILED));
        }
        notices
    }

    fn blank_moza(&mut self, now_ns: u64, notices: &mut Vec<InitNotice>) {
        let indexes: Vec<usize> = self
            .wheels
            .iter()
            .enumerate()
            .filter_map(|(index, wheel)| wheel.as_ref().map(|_| index))
            .collect();
        for index in indexes {
            let off = Telemetry::new();
            notices.extend(self.write_moza(index, &off, now_ns));
        }
    }

    fn close_ports(&mut self, notices: &mut Vec<InitNotice>) {
        for port in &mut self.ports {
            if matches!(port, HidPort::Live(_)) {
                *port = HidPort::Closed;
            }
        }
        for port in &mut self.serials {
            let path = serial_path_of(port);
            if path.is_empty() {
                *port = SerialPort::Closed;
                continue;
            }
            notices.push(notice(Level::Debug, games::serial_free_message(&path)));
            *port = SerialPort::Closed;
        }
    }
}

fn disarm_wheel(wheels: &mut [Option<MozaNewWheel>], index: usize) {
    if let Some(wheel) = wheels.get_mut(index).and_then(Option::as_mut) {
        wheel.disarm();
    }
}

fn serial_path_of(port: &SerialPort) -> String {
    match port {
        SerialPort::Captured { path, .. } | SerialPort::Live { path, .. } => path.clone(),
        SerialPort::Closed => String::new(),
    }
}

fn moza_step_notices(step: &cargopit_devices::serial::MozaNewStep) -> Vec<InitNotice> {
    let mut notices = Vec::new();
    if step.arm_failed {
        notices.push(notice(Level::Warn, games::MSG_MOZA_ARM_FAILED));
    }
    if step.just_armed {
        notices.push(notice(Level::Info, games::MSG_MOZA_ARMED));
    }
    if step.rpm_failed {
        notices.push(notice(Level::Warn, games::MSG_MOZA_RPM_FAILED));
    }
    notices
}

fn write_serial_frames(port: Option<&mut SerialPort>, frames: &[Vec<u8>]) -> bool {
    let Some(port) = port else {
        return false;
    };
    if frames.is_empty() {
        return true;
    }
    let mut last_ok = true;
    for frame in frames {
        last_ok = write_serial_frame(port, frame);
    }
    last_ok
}

fn write_serial_frame(port: &mut SerialPort, frame: &[u8]) -> bool {
    match port {
        SerialPort::Live { port, .. } => port.write(frame).is_ok(),
        SerialPort::Captured { frames, .. } => {
            frames.push(frame.to_vec());
            true
        }
        SerialPort::Closed => false,
    }
}

pub fn profile_index(profile_count: usize, requested: i32) -> Option<usize> {
    if profile_count == 0 {
        return None;
    }
    if requested < CONFIG_INDEX_FIRST {
        return Some(0);
    }
    if requested as usize >= profile_count {
        return None;
    }
    Some(requested as usize)
}

struct PreparedDevice {
    device: SimDevice,
    effect: Option<i32>,
}

struct TachMap {
    active: bool,
    granularity: u32,
    use_pulses: bool,
    pulses: Vec<u32>,
}

enum HidPort {
    Closed,
    Live(RealHid),
    Captured(Vec<Vec<u8>>),
}

enum TachSource {
    Unset,
    NamedNone,
    File(PathBuf),
}

struct TachPrep {
    notices: Vec<InitNotice>,
    ready: bool,
    map: TachMap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceSkip {
    Disabled,
    AudioDisabled,
}

pub fn device_skip(entry: &DeviceEntry, disable_audio: bool) -> Option<DeviceSkip> {
    if entry.get_bool(keys::KEY_ENABLED) == Some(false) {
        return Some(DeviceSkip::Disabled);
    }
    if disable_audio && entry_kind(entry) == DeviceKind::Sound {
        return Some(DeviceSkip::AudioDisabled);
    }
    None
}

pub fn open_profile_at(
    config: &CargopitConfig,
    index: usize,
    disable_audio: bool,
    now_ns: u64,
) -> ProfileLoad {
    open_profile_with(
        config,
        index,
        disable_audio,
        USE_PULSES_DURING_PLAY,
        &mut HidAttempt::Live { now_ns },
    )
}

pub fn open_profile_probed<F>(
    config: &CargopitConfig,
    index: usize,
    disable_audio: bool,
    use_pulses: bool,
    probe: F,
) -> ProfileLoad
where
    F: FnMut(u16, u16) -> bool,
{
    open_profile_ports(
        config,
        index,
        disable_audio,
        use_pulses,
        PROBE_OPEN_NS,
        probe,
        |_path| true,
    )
}

pub fn open_profile_ports<H, S>(
    config: &CargopitConfig,
    index: usize,
    disable_audio: bool,
    use_pulses: bool,
    now_ns: u64,
    mut hid: H,
    mut serial: S,
) -> ProfileLoad
where
    H: FnMut(u16, u16) -> bool,
    S: FnMut(&str) -> bool,
{
    open_profile_with(
        config,
        index,
        disable_audio,
        use_pulses,
        &mut HidAttempt::Probe {
            hid: &mut hid,
            serial: &mut serial,
            now_ns,
        },
    )
}

fn open_profile_with(
    config: &CargopitConfig,
    index: usize,
    disable_audio: bool,
    use_pulses: bool,
    attempt: &mut HidAttempt<'_>,
) -> ProfileLoad {
    let Some(profile) = config.profiles.get(index) else {
        return ProfileLoad {
            devices: LoadedDevices::empty(),
            setup_notices: Vec::new(),
            notices: Vec::new(),
        };
    };
    let mut devices = Vec::new();
    let mut effects = Vec::new();
    let mut ports = Vec::new();
    let mut tables = Vec::new();
    let mut serials = Vec::new();
    let mut wheels = Vec::new();
    let mut setup_notices = Vec::new();
    let mut notices = Vec::new();
    let mut initialized = 0i32;
    for (slot, entry) in profile.devices.iter().enumerate() {
        let slot = i32::try_from(slot).unwrap_or(i32::MAX);
        let considered = consider_entry(
            entry,
            slot,
            devices.len() as i32,
            disable_audio,
            use_pulses,
            attempt,
        );
        setup_notices.extend(considered.setup_notices);
        notices.extend(considered.notices);
        let Some(prepared) = considered.prepared else {
            continue;
        };
        ports.push(considered.port);
        tables.push(considered.tach);
        serials.push(considered.serial);
        wheels.push(considered.wheel);
        devices.push(prepared.device);
        effects.push(prepared.effect);
        initialized = initialized.saturating_add(1);
    }
    notices.push(notice(
        Level::Info,
        games::initialized_devices_message(initialized),
    ));
    for device in &devices {
        notices.push(notice(
            Level::Info,
            games::starting_device_message(c_device_type(device.kind()), device.id(), device.fps()),
        ));
    }
    let updates = vec![0; devices.len()];
    ProfileLoad {
        devices: LoadedDevices {
            devices,
            updates,
            effects,
            ports,
            tables,
            serials,
            wheels,
        },
        setup_notices,
        notices,
    }
}

enum HidAttempt<'a> {
    Live {
        now_ns: u64,
    },
    Probe {
        hid: &'a mut dyn FnMut(u16, u16) -> bool,
        serial: &'a mut dyn FnMut(&str) -> bool,
        now_ns: u64,
    },
}

enum OpenedHid {
    Missing,
    Live(RealHid),
    Simulated,
}

struct Considered {
    setup_notices: Vec<InitNotice>,
    notices: Vec<InitNotice>,
    prepared: Option<PreparedDevice>,
    port: HidPort,
    tach: TachMap,
    serial: SerialPort,
    wheel: Option<MozaNewWheel>,
}

enum SerialPort {
    Closed,
    Captured { path: String, frames: Vec<Vec<u8>> },
    Live { path: String, port: RealSerial },
}

fn consider_entry(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    use_pulses: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(prep) = revburner_prep(entry, use_pulses) {
        return consider_tach(entry, slot, id, disable_audio, attempt, prep);
    }
    if moza_new_entry(entry) {
        return consider_moza(entry, slot, id, disable_audio, attempt);
    }
    consider_closed(entry, slot, disable_audio)
}

fn consider_tach(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
    prep: TachPrep,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(prep.notices, skip, slot);
    }
    if !prep.ready {
        return setup_only(prep.notices);
    }
    revburner_attempt(entry, id, attempt, prep)
}

fn consider_closed(entry: &DeviceEntry, slot: i32, disable_audio: bool) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    Considered {
        setup_notices: Vec::new(),
        notices: vec![notice(
            Level::Warn,
            games::could_not_initialize_message(class_label(entry_kind(entry))),
        )],
        prepared: None,
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
    }
}

fn skipped(setup_notices: Vec<InitNotice>, skip: DeviceSkip, slot: i32) -> Considered {
    Considered {
        setup_notices,
        notices: vec![notice(Level::Info, skip_message(skip, slot))],
        prepared: None,
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
    }
}

fn setup_only(setup_notices: Vec<InitNotice>) -> Considered {
    Considered {
        setup_notices,
        notices: Vec::new(),
        prepared: None,
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
    }
}

fn revburner_prep(entry: &DeviceEntry, use_pulses: bool) -> Option<TachPrep> {
    if !usb_uses_revburner(entry) {
        return None;
    }
    Some(prepare_tach(entry, use_pulses))
}

fn prepare_tach(entry: &DeviceEntry, use_pulses: bool) -> TachPrep {
    match tach_source(entry) {
        TachSource::Unset => TachPrep {
            notices: vec![
                notice(Level::Trace, games::MSG_TACH_CONFIG_NONE),
                notice(Level::Warn, games::MSG_TACH_CONFIG_REQUIRED),
            ],
            ready: false,
            map: inactive_tach(),
        },
        TachSource::NamedNone => TachPrep {
            notices: vec![notice(Level::Warn, games::MSG_TACH_CONFIG_REQUIRED)],
            ready: false,
            map: inactive_tach(),
        },
        TachSource::File(path) => prep_from_file(entry, use_pulses, &path),
    }
}

fn prep_from_file(entry: &DeviceEntry, use_pulses: bool, path: &Path) -> TachPrep {
    let shown = path.display().to_string();
    let mut notices = vec![notice(
        Level::Trace,
        games::tach_config_load_message(&shown),
    )];
    let (pulses, faults) = load_tach_points(path, &shown);
    notices.extend(faults);
    let (granularity, extra) = read_granularity(entry);
    notices.extend(extra);
    notices.push(notice(
        Level::Info,
        games::tach_granularity_message(granularity),
    ));
    TachPrep {
        notices,
        ready: true,
        map: TachMap {
            active: true,
            granularity: u32::try_from(granularity).unwrap_or(0),
            use_pulses,
            pulses,
        },
    }
}

fn tach_source(entry: &DeviceEntry) -> TachSource {
    match entry.get_str(keys::KEY_CONFIG) {
        None => TachSource::Unset,
        Some(value) if value.eq_ignore_ascii_case(keys::CONFIG_VALUE_NONE) => TachSource::NamedNone,
        Some(value) => TachSource::File(paths::expand_tilde(value)),
    }
}

fn load_tach_points(path: &Path, shown: &str) -> (Vec<u32>, Vec<InitNotice>) {
    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(_) => {
            return (
                Vec::new(),
                vec![notice(Level::Error, games::tach_xml_read_message(shown))],
            );
        }
    };
    match tach::parse_tach_xml(&src) {
        Ok(points) => (
            points.into_iter().map(|point| point.pulses).collect(),
            Vec::new(),
        ),
        Err(ERR_TACH_XML_EMPTY) => (
            Vec::new(),
            vec![notice(Level::Error, games::MSG_TACH_XML_EMPTY)],
        ),
        Err(_) => (
            Vec::new(),
            vec![notice(Level::Error, games::tach_xml_read_message(shown))],
        ),
    }
}

fn read_granularity(entry: &DeviceEntry) -> (i64, Vec<InitNotice>) {
    let raw = entry
        .get_i64(keys::KEY_GRANULARITY)
        .unwrap_or(GRANULARITY_MIN);
    if !(GRANULARITY_MIN..=GRANULARITY_MAX).contains(&raw) || raw == GRANULARITY_REJECTED {
        return (
            GRANULARITY_FALLBACK,
            vec![notice(Level::Debug, games::MSG_TACH_GRANULARITY_INVALID)],
        );
    }
    (raw, Vec::new())
}

fn revburner_attempt(
    entry: &DeviceEntry,
    id: i32,
    attempt: &mut HidAttempt<'_>,
    prep: TachPrep,
) -> Considered {
    let mut notices = vec![
        notice(Level::Info, games::MSG_INIT_USB),
        notice(Level::Info, games::MSG_INIT_TACH),
        notice(Level::Info, games::MSG_INIT_REVBURNER),
    ];
    match open_revburner(attempt) {
        OpenedHid::Missing => {
            notices.push(notice(Level::Error, games::MSG_REVBURNER_MISSING));
            notices.push(notice(
                Level::Warn,
                games::usb_init_error_message(games::ERROR_UNKNOWN),
            ));
            notices.push(notice(
                Level::Warn,
                games::could_not_initialize_message(CLASS_USB),
            ));
            Considered {
                setup_notices: prep.notices,
                notices,
                prepared: None,
                port: HidPort::Closed,
                tach: inactive_tach(),
                serial: SerialPort::Closed,
                wheel: None,
            }
        }
        OpenedHid::Live(hid) => Considered {
            setup_notices: prep.notices,
            notices,
            prepared: Some(build_device(entry, id)),
            port: HidPort::Live(hid),
            tach: prep.map,
            serial: SerialPort::Closed,
            wheel: None,
        },
        OpenedHid::Simulated => Considered {
            setup_notices: prep.notices,
            notices,
            prepared: Some(build_device(entry, id)),
            port: HidPort::Captured(Vec::new()),
            tach: prep.map,
            serial: SerialPort::Closed,
            wheel: None,
        },
    }
}

fn open_revburner(attempt: &mut HidAttempt<'_>) -> OpenedHid {
    match attempt {
        HidAttempt::Live { .. } => match RealHid::open(REVBURNER_VENDOR_ID, REVBURNER_PRODUCT_ID) {
            Ok(hid) => OpenedHid::Live(hid),
            Err(_) => OpenedHid::Missing,
        },
        HidAttempt::Probe { hid, .. } => {
            if hid(REVBURNER_VENDOR_ID, REVBURNER_PRODUCT_ID) {
                return OpenedHid::Simulated;
            }
            OpenedHid::Missing
        }
    }
}

fn consider_moza(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    open_moza_new(entry, id, attempt)
}

fn moza_new_entry(entry: &DeviceEntry) -> bool {
    if entry_kind(entry) != DeviceKind::Serial {
        return false;
    }
    let kind = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    if names::lookup(names::SERIAL_TYPES, kind) != Some(names::SUBTYPE_SERIAL_WHEEL) {
        return false;
    }
    let hardware = entry.get_str(keys::KEY_SUBTYPE).unwrap_or("");
    names::lookup(names::HARDWARE, hardware) == Some(names::HARDWARE_MOZA_NEW)
}

fn open_moza_new(entry: &DeviceEntry, id: i32, attempt: &mut HidAttempt<'_>) -> Considered {
    let path = device_port(entry);
    let configured = entry.get_i64(keys::KEY_BAUD).unwrap_or(keys::BAUD_DEFAULT);
    let baud = serial::moza_r9_open_baud(configured);
    let mut notices = moza_lookup_notices(&path, configured);
    match open_serial(attempt, &path, baud) {
        OpenedSerial::Missing => moza_open_failed(notices),
        OpenedSerial::Ready(port) => {
            notices.extend(moza_ready_notices(baud));
            finish_moza(entry, id, path, port, notices, attempt_now(attempt))
        }
    }
}

fn device_port(entry: &DeviceEntry) -> String {
    entry
        .get_str(keys::KEY_DEVID)
        .or_else(|| entry.get_str(keys::KEY_DEVPATH))
        .unwrap_or("")
        .to_string()
}

fn moza_lookup_notices(path: &str, configured: i64) -> Vec<InitNotice> {
    vec![
        notice(
            Level::Trace,
            games::serial_subtype_message(names::SUBTYPE_SERIAL_WHEEL),
        ),
        notice(Level::Info, games::MSG_MOZA_NEW_INIT),
        notice(Level::Info, games::MSG_SERIAL_START),
        notice(
            Level::Info,
            games::serial_init_port_message(path, configured),
        ),
        notice(Level::Info, games::serial_looking_message(path)),
        notice(Level::Debug, games::MSG_SERIAL_NO_EXISTING),
        notice(Level::Info, games::MSG_SERIAL_OPENING),
        notice(Level::Debug, games::serial_looking_for_port_message(path)),
    ]
}

fn moza_ready_notices(baud: u32) -> Vec<InitNotice> {
    vec![
        notice(Level::Debug, games::MSG_SERIAL_PORT_OPENED),
        notice(Level::Debug, games::serial_baud_message(baud)),
        notice(Level::Debug, games::MSG_SERIAL_SETUP_OK),
    ]
}

fn moza_open_failed(mut notices: Vec<InitNotice>) -> Considered {
    notices.push(notice(Level::Error, games::MSG_SERIAL_OPEN_ERROR));
    notices.push(notice(
        Level::Warn,
        games::serial_init_error_message(games::SERIAL_OPEN_ERROR),
    ));
    notices.push(notice(
        Level::Warn,
        games::could_not_initialize_message(CLASS_SERIAL),
    ));
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: None,
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
    }
}

enum OpenedSerial {
    Missing,
    Ready(SerialPort),
}

fn open_serial(attempt: &mut HidAttempt<'_>, path: &str, baud: u32) -> OpenedSerial {
    if path.is_empty() {
        return OpenedSerial::Missing;
    }
    match attempt {
        HidAttempt::Live { .. } => match RealSerial::open(path, baud) {
            Ok(port) => OpenedSerial::Ready(SerialPort::Live {
                path: path.to_string(),
                port,
            }),
            Err(_) => OpenedSerial::Missing,
        },
        HidAttempt::Probe { serial, .. } => {
            if serial(path) {
                return OpenedSerial::Ready(SerialPort::Captured {
                    path: path.to_string(),
                    frames: Vec::new(),
                });
            }
            OpenedSerial::Missing
        }
    }
}

fn attempt_now(attempt: &HidAttempt<'_>) -> u64 {
    match attempt {
        HidAttempt::Live { now_ns } | HidAttempt::Probe { now_ns, .. } => *now_ns,
    }
}

fn finish_moza(
    entry: &DeviceEntry,
    id: i32,
    path: String,
    mut port: SerialPort,
    mut notices: Vec<InitNotice>,
    now_ns: u64,
) -> Considered {
    notices.extend(share_notices(&mut port));
    let mut wheel = MozaNewWheel::new();
    let step = wheel.prepare(now_ns);
    let wrote = write_serial_frames(Some(&mut port), &step.frames);
    if wheel.armed() && wrote {
        notices.push(notice(Level::Info, games::moza_opened_message(&path)));
    } else {
        wheel.disarm();
        notices.push(notice(Level::Warn, games::moza_arm_retry_message(&path)));
    }
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: port,
        wheel: Some(wheel),
    }
}

fn share_notices(port: &mut SerialPort) -> Vec<InitNotice> {
    let SerialPort::Live { port, .. } = port else {
        return Vec::new();
    };
    port.share().into_iter().map(share_notice).collect()
}

fn share_notice(warning: ShareWarning) -> InitNotice {
    let message = match warning {
        ShareWarning::NativeHandle => games::MSG_SHARE_HANDLE,
        ShareWarning::Exclusive => games::MSG_SHARE_EXCLUSIVE,
        ShareWarning::Hupcl => games::MSG_SHARE_HUPCL,
    };
    notice(Level::Warn, message)
}

fn usb_uses_revburner(entry: &DeviceEntry) -> bool {
    if entry_kind(entry) != DeviceKind::Usb {
        return false;
    }
    let subtype = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    !matches!(
        names::lookup(names::USB_TYPES, subtype),
        Some(names::SUBTYPE_USB_WHEEL | names::SUBTYPE_USB_HAPTIC)
    )
}

fn class_label(kind: DeviceKind) -> &'static str {
    match kind {
        DeviceKind::Usb => CLASS_USB,
        DeviceKind::Sound => CLASS_SOUND,
        DeviceKind::Serial => CLASS_SERIAL,
        DeviceKind::Unknown => games::DEVICE_NAME_MISSING,
    }
}

fn c_device_type(kind: DeviceKind) -> i32 {
    match kind {
        DeviceKind::Usb => names::DEVICE_USB,
        DeviceKind::Sound => names::DEVICE_SOUND,
        DeviceKind::Serial => names::DEVICE_SERIAL,
        DeviceKind::Unknown => names::DEVICE_UNKNOWN,
    }
}

fn skip_message(skip: DeviceSkip, index: i32) -> String {
    match skip {
        DeviceSkip::Disabled => games::skipping_disabled_message(index),
        DeviceSkip::AudioDisabled => games::MSG_SKIP_AUDIO.to_string(),
    }
}

fn build_device(entry: &DeviceEntry, id: i32) -> PreparedDevice {
    let kind = entry_kind(entry);
    let fps = scheduler::clamp_fps(entry.get_i64(keys::KEY_FPS).unwrap_or(0) as i32);
    let mut device = SimDevice::new(id, fps, kind);
    device.set_initialized(true);
    device.set_config_file(entry.get_str(keys::KEY_CONFIG).map(str::to_string));
    PreparedDevice {
        device,
        effect: device_effect(entry),
    }
}

fn inactive_tach() -> TachMap {
    TachMap {
        active: false,
        granularity: 0,
        use_pulses: false,
        pulses: Vec::new(),
    }
}

fn notice(level: Level, message: impl Into<String>) -> InitNotice {
    InitNotice {
        level,
        message: message.into(),
    }
}

fn write_port(port: &mut HidPort, report: &[u8], notices: &mut Vec<InitNotice>) {
    match port {
        HidPort::Live(hid) => {
            let _ = hid.write(report);
        }
        HidPort::Captured(log) => log.push(report.to_vec()),
        HidPort::Closed => notices.push(notice(Level::Debug, games::MSG_REVBURNER_NO_HANDLE)),
    }
}

fn tach_trace_notices(sample: &TachPulses, size: usize) -> Vec<InitNotice> {
    let mut notices = Vec::new();
    match sample {
        TachPulses::Frame(_) => {}
        TachPulses::Idle(_) => {
            notices.push(notice(Level::Trace, games::MSG_TACH_GETTING_PULSES));
        }
        TachPulses::Indexed { element, .. } => {
            notices.push(notice(Level::Trace, games::MSG_TACH_GETTING_PULSES));
            let size = i32::try_from(size).unwrap_or(i32::MAX);
            notices.push(notice(
                Level::Trace,
                games::tach_settings_size_message(size),
            ));
            let element = i32::try_from(*element).unwrap_or(i32::MAX);
            notices.push(notice(Level::Trace, games::tach_element_message(element)));
        }
    }
    notices.push(notice(
        Level::Trace,
        games::tach_pulses_message(sample.pulses()),
    ));
    notices
}

pub fn device_effect(entry: &DeviceEntry) -> Option<i32> {
    let name = entry.get_str(keys::KEY_EFFECT)?;
    names::lookup(names::EFFECTS, name)
}

pub fn entry_kind(entry: &DeviceEntry) -> DeviceKind {
    let class = entry
        .get_str(keys::KEY_DEVICE)
        .or_else(|| entry.get_str(keys::KEY_TYPE))
        .unwrap_or("");
    device_kind(class)
}

pub fn device_kind(name: &str) -> DeviceKind {
    if name.eq_ignore_ascii_case(CLASS_USB) {
        return DeviceKind::Usb;
    }
    if name.eq_ignore_ascii_case(CLASS_SERIAL) {
        return DeviceKind::Serial;
    }
    if name.eq_ignore_ascii_case(CLASS_SOUND) {
        return DeviceKind::Sound;
    }
    DeviceKind::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games;
    use cargopit_config::config::{DeviceEntry, SimProfile};
    use cargopit_config::keys::CLASS_SERIAL;
    use cargopit_devices::telemetry::Telemetry;

    const PLAY_USES_PULSES: bool = false;
    const FRAME_PULSES: u32 = 7;
    const RPM_TABLE: u32 = 1000;
    const ELEMENT_AT_1000: i32 = 1;
    const GRANULARITY_ONE: i64 = 1;
    const GRANULARITY_REJECTED_VALUE: i64 = 3;

    fn entry(kind: &str, fps: i64, enabled: bool) -> DeviceEntry {
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_TYPE, kind);
        device.set_int(keys::KEY_FPS, fps);
        device.set_bool(keys::KEY_ENABLED, enabled);
        device
    }

    fn sample_xml() -> String {
        paths::source_root()
            .join(keys::CONF_DIRNAME)
            .join(keys::REVBURNER_XML_NAME)
            .display()
            .to_string()
    }

    fn sample_points() -> Vec<tach::TachPoint> {
        let src = std::fs::read_to_string(sample_xml()).expect("sample xml");
        tach::parse_tach_xml(&src).expect("tach")
    }

    fn tach_entry(fps: i64, enabled: bool) -> DeviceEntry {
        let mut device = entry("usb", fps, enabled);
        device.set_str(keys::KEY_CONFIG, sample_xml());
        device.set_int(keys::KEY_GRANULARITY, GRANULARITY_ONE);
        device
    }

    #[test]
    fn profile_load_skips_disabled_and_muted_sound() {
        let config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![
                    tach_entry(60, true),
                    entry("serial", 0, true),
                    entry("sound", 60, true),
                    entry("wheel", 144, false),
                ],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let loaded =
            open_profile_probed(&config, 0, true, PLAY_USES_PULSES, |_vendor, _product| true);
        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(loaded.devices.device(0).unwrap().kind(), DeviceKind::Usb);
        assert_eq!(loaded.devices.device(0).unwrap().interval_ms(), 16);
        assert_eq!(loaded.devices.hid_handles(), 0);
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::initialized_devices_message(1)));
        assert!(loaded.notices.iter().any(|notice| {
            notice.message == games::starting_device_message(names::DEVICE_USB, 0, 60)
        }));
        assert!(loaded
            .setup_notices
            .iter()
            .any(|notice| notice.message == games::tach_granularity_message(GRANULARITY_ONE)));
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::could_not_initialize_message(CLASS_SERIAL)));
        let missing =
            open_profile_probed(&config, 0, true, PLAY_USES_PULSES, |_vendor, _product| {
                false
            });
        assert!(missing.devices.is_empty());
        assert!(missing
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_REVBURNER_MISSING));
        assert!(missing
            .notices
            .iter()
            .any(|notice| { notice.message == games::initialized_devices_message(0) }));
        assert!(profile_index(1, 4).is_none());
        let disabled = entry("sound", 60, false);
        assert_eq!(device_skip(&disabled, true), Some(DeviceSkip::Disabled));
        let muted = entry("sound", 60, true);
        assert_eq!(device_skip(&muted, true), Some(DeviceSkip::AudioDisabled));
        assert_eq!(device_skip(&muted, false), None);
        loaded_tick_counts();
    }

    fn loaded_tick_counts() {
        let config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![tach_entry(60, true)],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let mut loaded =
            open_profile_probed(&config, 0, false, PLAY_USES_PULSES, |_vendor, _product| {
                true
            })
            .devices;
        let points = sample_points();
        let mut frame = Telemetry::new();
        let notices = loaded.tick(0, &frame, PROBE_OPEN_NS);
        assert_eq!(loaded.updates(0), 1);
        assert!(notices
            .iter()
            .any(|notice| notice.message == games::MSG_TACH_GETTING_PULSES));
        assert_eq!(
            loaded.captured_reports(0).map(|reports| reports[0].clone()),
            Some(usb::revburner_report(points[0].pulses).to_vec())
        );
        frame.set_rpms(RPM_TABLE);
        let indexed = loaded.tick(0, &frame, PROBE_OPEN_NS);
        assert!(indexed
            .iter()
            .any(|notice| notice.message == games::tach_element_message(ELEMENT_AT_1000)));
        let reports = loaded.captured_reports(0).expect("reports");
        assert_eq!(
            reports[1],
            usb::revburner_report(points[ELEMENT_AT_1000 as usize].pulses).to_vec()
        );
        let released = loaded.release(PROBE_OPEN_NS);
        assert!(released.iter().any(|notice| {
            notice.message == games::device_runner_message(0, loaded.updates(0), 0)
        }));
        let reports = loaded.captured_reports(0).expect("reports");
        assert_eq!(
            reports.last().map(Vec::as_slice),
            Some(usb::revburner_report(0).as_slice())
        );
    }

    #[test]
    fn frame_pulses_skip_the_table_when_requested() {
        let config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![tach_entry(60, true)],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let mut loaded =
            open_profile_probed(&config, 0, false, true, |_vendor, _product| true).devices;
        let mut frame = Telemetry::new();
        frame.set_rpms(RPM_TABLE);
        frame.set_pulses(FRAME_PULSES);
        let notices = loaded.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notices
            .iter()
            .all(|notice| notice.message != games::MSG_TACH_GETTING_PULSES));
        assert_eq!(
            loaded.captured_reports(0).map(|reports| reports[0].clone()),
            Some(usb::revburner_report(FRAME_PULSES).to_vec())
        );
    }

    #[test]
    fn tachometer_without_xml_does_not_open() {
        let config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![entry("usb", 60, true)],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let missing =
            open_profile_probed(&config, 0, false, PLAY_USES_PULSES, |_vendor, _product| {
                panic!("hid open")
            });
        assert!(missing.devices.is_empty());
        assert!(missing
            .setup_notices
            .iter()
            .any(|notice| notice.message == games::MSG_TACH_CONFIG_NONE));
        assert!(missing
            .setup_notices
            .iter()
            .any(|notice| notice.message == games::MSG_TACH_CONFIG_REQUIRED));
        let mut named = entry("usb", 60, true);
        named.set_str(keys::KEY_CONFIG, keys::CONFIG_VALUE_NONE);
        let named_config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![named],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let named_none = open_profile_probed(
            &named_config,
            0,
            false,
            PLAY_USES_PULSES,
            |_vendor, _product| panic!("hid open"),
        );
        assert!(named_none
            .setup_notices
            .iter()
            .any(|notice| notice.message == games::MSG_TACH_CONFIG_REQUIRED));
        assert!(named_none
            .setup_notices
            .iter()
            .all(|notice| notice.message != games::MSG_TACH_CONFIG_NONE));
        let mut rejected = tach_entry(60, true);
        rejected.set_int(keys::KEY_GRANULARITY, GRANULARITY_REJECTED_VALUE);
        let rejected_config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![rejected],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let loaded = open_profile_probed(
            &rejected_config,
            0,
            false,
            PLAY_USES_PULSES,
            |_vendor, _product| true,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert!(loaded
            .setup_notices
            .iter()
            .any(|notice| notice.message == games::MSG_TACH_GRANULARITY_INVALID));
        assert!(loaded
            .setup_notices
            .iter()
            .any(|notice| notice.message == games::tach_granularity_message(GRANULARITY_FALLBACK)));
    }

    #[test]
    fn slip_alias_marks_the_profile_for_tyre_diameter() {
        let mut slip = tach_entry(60, true);
        slip.set_str(keys::KEY_EFFECT, "Slip");
        let mut engine = entry("serial", 60, true);
        engine.set_str(keys::KEY_EFFECT, "Engine");
        let config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![slip, engine],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let loaded =
            open_profile_probed(&config, 0, false, PLAY_USES_PULSES, |_vendor, _product| {
                true
            })
            .devices;
        assert_eq!(loaded.effect(0), Some(names::EFFECT_TYRE_SLIP));
        assert!(loaded.needs_tyre_diameter());
    }

    fn moza_config(path: &str, baud: i64) -> CargopitConfig {
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_DEVICE, keys::CLASS_SERIAL);
        device.set_str(keys::KEY_TYPE, keys::TYPE_WHEEL);
        device.set_str(keys::KEY_SUBTYPE, keys::SUBTYPE_MOZA_R9);
        device.set_str(keys::KEY_DEVPATH, path);
        device.set_int(keys::KEY_BAUD, baud);
        device.set_bool(keys::KEY_ENABLED, true);
        device.set_int(keys::KEY_FPS, 60);
        CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![device],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        }
    }

    #[test]
    fn moza_r9_arms_from_the_configured_port() {
        const PORT: &str = "/dev/ttyMOZA-TEST";
        let config = moza_config(PORT, keys::BAUD_DEFAULT);
        let baud = serial::moza_r9_open_baud(keys::BAUD_DEFAULT);
        let missing = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |_path| false,
        );
        assert!(missing.devices.is_empty());
        assert!(missing
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_SERIAL_OPEN_ERROR));
        assert!(missing.notices.iter().any(|notice| {
            notice.message == games::serial_init_error_message(games::SERIAL_OPEN_ERROR)
        }));
        assert!(missing
            .notices
            .iter()
            .any(|notice| { notice.message == games::could_not_initialize_message(CLASS_SERIAL) }));
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |_path| true,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(loaded.devices.device(0).unwrap().kind(), DeviceKind::Serial);
        assert!(loaded.notices.iter().any(|notice| {
            notice.message == games::serial_init_port_message(PORT, keys::BAUD_DEFAULT)
        }));
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::serial_baud_message(baud)));
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::moza_opened_message(PORT)));
        let mut reference = MozaNewWheel::new();
        let expected = reference.prepare(PROBE_OPEN_NS).frames;
        assert_eq!(
            loaded
                .devices
                .captured_serial(0)
                .map(|frames| frames.to_vec()),
            Some(expected)
        );
        let mut devices = loaded.devices;
        let before = devices.captured_serial(0).unwrap().len();
        let _ = devices.tick(0, &Telemetry::new(), PROBE_OPEN_NS);
        assert!(devices.captured_serial(0).unwrap().len() > before);
        let released = devices.release(PROBE_OPEN_NS);
        assert!(released
            .iter()
            .any(|notice| notice.message == games::serial_free_message(PORT)));
    }
}
