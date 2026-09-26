//! Configured devices for one play session. A USB tachometer opens the RevBurner
//! and writes its pulse report on each tick. A Logitech G29, a Cammus C5, and a
//! Cammus C12 open and write their LED reports on each tick. A C12 with a Lua file
//! writes one packet per shift light. A Simagic GT Neo with a Lua file sends
//! feature reports for all 73 LEDs. A Fanatec CSL Elite V3 opens its sysfs rumble
//! file and writes the haptic value when that value changes. A Simagic P1000 opens
//! over HID and sends feature reports when that play value changes. A SimNet pedal
//! opens over HID and writes its motor report when that play value changes. A Moza R9 serial wheel opens its port and
//! writes the new-firmware LED frames. A Moza R5, R8, or R3 opens its serial port and
//! writes the shift-light mask on each tick. Shift lights open their serial port and write
//! one lit-count byte on each tick. SimWind opens its serial port and writes speed
//! and fan power on each tick. Serial haptic opens its port when the sim supports
//! haptics and writes an eight-byte motor report on each tick. SimLED opens its
//! serial port and writes a shift-light packet on each tick. A custom SimLED queries the LED
//! count, loads its Lua file, and writes a script-painted packet on each tick. A custom Arduino
//! loads its Lua file and writes the script Message on each tick. A sound device logs the C init sequence,
//! connects its Pulse playback stream, and renders haptic samples on each tick.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use cargopit_config::config::{CargopitConfig, DeviceEntry};
use cargopit_config::keys::{self, CLASS_SERIAL, CLASS_SOUND, CLASS_USB};
use cargopit_config::names;
use cargopit_config::paths;
use cargopit_config::tach::{self, ERR_TACH_XML_EMPTY};
use cargopit_devices::clock::{SystemClock, VirtualClock};
use cargopit_devices::haptic::{HapticEffect, HapticSettings, TyreId, VibrationEffect};
use cargopit_devices::lua_host::{lua_detail, LuaHost, LuaLedMode};
use cargopit_devices::serial::{self, MozaNewWheel};
use cargopit_devices::sound::SharedShaker;
use cargopit_devices::telemetry::Telemetry;
use cargopit_devices::transport::{PulseSession, RealHid, RealSerial, ShakerRequest, ShareWarning};
use cargopit_devices::usb::{self, TachPulses};
use cargopit_devices::{tick_interval_ms, DeviceKind, SimDevice, DEFAULT_DEVICE_FPS};

use crate::games;
use crate::log::Level;
use crate::scheduler;
use crate::sound_host;
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
const G29_RELEASE_RPM: u32 = 0;
const C5_RELEASE_RPM: u32 = 0;
const C5_RELEASE_GEAR: u32 = 0;
const C5_RELEASE_VELOCITY: u32 = 0;
const ASSUME_SIM_SUPPORTS_HAPTICS: bool = true;
const SIMLED_COUNT_UNSET: i32 = 0;
const SIMLED_INPUT_WAIT_ERROR: i32 = -1;
const ARDUINO_CUSTOM_LED_TOTAL: i64 = 0;
const HAPTIC_AMPLITUDE_UNITY: i64 = 100;
const HAPTIC_MOTOR_DEFAULT: i64 = 1;
const HAPTIC_FREQUENCY_DEFAULT: i64 = 0;
const HAPTIC_DURATION_UNSET: f64 = 0.0;
const GEAR_DURATION_DEFAULT_S: f64 = 0.125;
const HAPTIC_THRESHOLD_DEFAULT: f64 = 0.0;
const HAPTIC_STATE_IDLE: f64 = 0.0;
const CSL_PROBE_CAPTURED: i32 = 0;
const CSL_PROBE_MISSING: i32 = 1;
const CSL_PROBE_PERMISSION: i32 = 2;
const CSL_PROBE_OPEN_FAILED: i32 = 3;

pub struct LoadedDevices {
    devices: Vec<SimDevice>,
    updates: Vec<u64>,
    effects: Vec<Option<i32>>,
    ports: Vec<HidPort>,
    tables: Vec<TachMap>,
    serials: Vec<SerialPort>,
    wheels: Vec<Option<MozaNewWheel>>,
    sounds: Vec<Option<ShakerRequest>>,
    voices: Vec<Option<SharedShaker>>,
    g29: Vec<bool>,
    c5: Vec<bool>,
    c12: Vec<bool>,
    gt: Vec<bool>,
    arduino_custom: Vec<bool>,
    moza_r5: Vec<bool>,
    csl: Vec<Option<CslPedal>>,
    p1000: Vec<Option<P1000Pedal>>,
    simnet: Vec<Option<SimNetPedal>>,
    shift_lights: Vec<Option<i32>>,
    simwind: Vec<Option<f64>>,
    serial_haptic: Vec<Option<SerialHaptic>>,
    simled: Vec<Option<SimLed>>,
    custom_leds: Vec<Option<i32>>,
    luas: Vec<Option<LuaHost>>,
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
        open_profile_at(
            config,
            index,
            disable_audio,
            PROBE_OPEN_NS,
            ASSUME_SIM_SUPPORTS_HAPTICS,
            None,
        )
        .devices
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
            sounds: Vec::new(),
            voices: Vec::new(),
            g29: Vec::new(),
            c5: Vec::new(),
            c12: Vec::new(),
            gt: Vec::new(),
            arduino_custom: Vec::new(),
            moza_r5: Vec::new(),
            csl: Vec::new(),
            p1000: Vec::new(),
            simnet: Vec::new(),
            shift_lights: Vec::new(),
            simwind: Vec::new(),
            serial_haptic: Vec::new(),
            simled: Vec::new(),
            custom_leds: Vec::new(),
            luas: Vec::new(),
        }
    }

    pub fn captured_sound(&self, index: usize) -> Option<&ShakerRequest> {
        self.sounds.get(index).and_then(Option::as_ref)
    }

    pub fn rendered_sound(&self, index: usize, nbytes: usize) -> Option<Vec<u8>> {
        let voice = self.voices.get(index)?.as_ref()?;
        let mut voice = voice.lock().unwrap_or_else(|poison| poison.into_inner());
        Some(voice.render(nbytes))
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

    pub fn captured_sysfs(&self, index: usize) -> Option<&[Vec<u8>]> {
        let pedal = self.csl.get(index)?.as_ref()?;
        match &pedal.file {
            CslFile::Captured { frames } => Some(frames),
            CslFile::Live { .. } | CslFile::Closed => None,
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
        notices.extend(self.write_moza_r5(index, frame));
        notices.extend(self.write_shiftlights(index, frame));
        notices.extend(self.write_simwind(index, frame));
        notices.extend(self.write_serial_haptic(index, frame, now_ns));
        notices.extend(self.write_simled(index, frame));
        notices.extend(self.write_simled_custom(index, frame));
        notices.extend(self.write_arduino_custom(index, frame));
        notices.extend(self.write_g29(index, frame.rpms(), frame.maxrpm()));
        notices.extend(self.write_c5(index, frame));
        notices.extend(self.write_c12(index, frame));
        notices.extend(self.write_gt(index, frame));
        notices.extend(self.write_csl(index, frame, now_ns));
        notices.extend(self.write_p1000(index, frame, now_ns));
        notices.extend(self.write_simnet(index, frame, now_ns));
        self.update_sound(index, frame, now_ns);
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
        self.blank_g29(&mut notices);
        self.blank_c5(&mut notices);
        self.blank_moza(now_ns, &mut notices);
        self.blank_serial_haptic(&mut notices);
        self.blank_simled();
        self.close_ports(&mut notices);
        self.close_c12_lua(&mut notices);
        self.close_csl();
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

    fn update_sound(&mut self, index: usize, frame: &Telemetry, now_ns: u64) {
        let Some(Some(voice)) = self.voices.get(index) else {
            return;
        };
        let mut voice = voice.lock().unwrap_or_else(|poison| poison.into_inner());
        voice.update(frame, &VirtualClock::from_monotonic_ns(now_ns));
    }

    fn write_g29(&mut self, index: usize, rpm: u32, maxrpm: u32) -> Vec<InitNotice> {
        if !self.g29.get(index).copied().unwrap_or(false) {
            return Vec::new();
        }
        let report = usb::g29_report(rpm, maxrpm);
        let rpm_i = i32::try_from(rpm).unwrap_or(i32::MAX);
        let mut notices = vec![notice(
            Level::Trace,
            games::g29_write_message(&report, rpm_i),
        )];
        self.write_index(index, &report, &mut notices);
        notices
    }

    fn write_c5(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        if !self.c5.get(index).copied().unwrap_or(false) {
            return Vec::new();
        }
        let report = usb::c5_report(frame.rpms(), frame.maxrpm(), frame.gear(), frame.velocity());
        let mut notices = vec![notice(
            Level::Trace,
            games::c5_write_message(
                &report,
                telemetry_i32(frame.rpms()),
                telemetry_i32(frame.velocity()),
                telemetry_i32(frame.gear()),
            ),
        )];
        self.write_index(index, &report, &mut notices);
        notices
    }

    fn write_c12(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        if !self.c12.get(index).copied().unwrap_or(false) {
            return Vec::new();
        }
        if self.luas.get(index).and_then(Option::as_ref).is_some() {
            return self.write_c12_lua(index, frame);
        }
        let report = usb::c12_report(frame.rpms(), frame.maxrpm(), frame.gear(), frame.velocity());
        let mut notices = vec![notice(
            Level::Trace,
            games::c12_write_message(
                &report,
                telemetry_i32(frame.rpms()),
                telemetry_i32(frame.velocity()),
                telemetry_i32(frame.gear()),
            ),
        )];
        self.write_index(index, &report, &mut notices);
        notices
    }

    fn write_c12_lua(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        let painted = self.script_colors(index, frame, usb::C12_LED_TOTAL);
        if let Some(detail) = painted.failure {
            eprintln!("{}", games::lua_call_failed_message(&detail));
        }
        let mut notices = Vec::new();
        for report in usb::c12_led_reports(&painted.leds) {
            notices.push(notice(Level::Trace, games::c12_led_message(&report)));
            self.write_index(index, &report, &mut notices);
        }
        notices
    }

    fn write_csl(&mut self, index: usize, frame: &Telemetry, now_ns: u64) -> Vec<InitNotice> {
        let Some(pedal) = self.csl.get_mut(index).and_then(Option::as_mut) else {
            return Vec::new();
        };
        let clock = VirtualClock::from_monotonic_ns(now_ns);
        let Some(play) = csl_play(pedal, frame, &clock) else {
            return Vec::new();
        };
        if play == pedal.state {
            return Vec::new();
        }
        let Some(kind) = pedal.effect.as_ref().map(HapticEffect::effect) else {
            return Vec::new();
        };
        let text = usb::csl_rumble_text(kind, play);
        write_csl_bytes(&mut pedal.file, text.as_bytes());
        pedal.state = play;
        Vec::new()
    }

    fn write_p1000(&mut self, index: usize, frame: &Telemetry, now_ns: u64) -> Vec<InitNotice> {
        let Some(reports) = self.p1000_changes(index, frame, now_ns) else {
            return Vec::new();
        };
        if reports.is_empty() {
            return Vec::new();
        }
        let mut notices = Vec::new();
        let Some(port) = self.ports.get_mut(index) else {
            return notices;
        };
        for report in &reports {
            let _ = record_p1000(port, report, &mut notices);
        }
        notices
    }

    fn p1000_changes(
        &mut self,
        index: usize,
        frame: &Telemetry,
        now_ns: u64,
    ) -> Option<Vec<[u8; usb::P1000_LEN]>> {
        let pedal = self.p1000.get_mut(index)?.as_mut()?;
        let clock = VirtualClock::from_monotonic_ns(now_ns);
        let play = haptic_play(&mut pedal.effect, frame, &clock)?;
        if play == pedal.state {
            return None;
        }
        let kind = pedal.effect.as_ref()?.effect();
        let reports = usb::p1000_reports(kind, play);
        pedal.state = play;
        Some(reports)
    }

    fn write_simnet(&mut self, index: usize, frame: &Telemetry, now_ns: u64) -> Vec<InitNotice> {
        let Some(report) = self.simnet_changes(index, frame, now_ns) else {
            return Vec::new();
        };
        let mut notices = Vec::new();
        let Some(port) = self.ports.get_mut(index) else {
            return notices;
        };
        if matches!(port, HidPort::Closed) {
            notices.push(notice(Level::Debug, games::MSG_REVBURNER_NO_HANDLE));
            return notices;
        }
        notices.push(notice(Level::Trace, games::simnet_write_message(&report)));
        write_port(port, &report, &mut notices);
        notices
    }

    fn simnet_changes(
        &mut self,
        index: usize,
        frame: &Telemetry,
        now_ns: u64,
    ) -> Option<[u8; usb::SIMNET_LEN]> {
        let pedal = self.simnet.get_mut(index)?.as_mut()?;
        let clock = VirtualClock::from_monotonic_ns(now_ns);
        let play = haptic_play(&mut pedal.effect, frame, &clock)?;
        if play == pedal.state {
            return None;
        }
        let effect = pedal.effect.as_ref()?;
        let report = usb::simnet_report(
            effect.motor_position(),
            effect.frequency(),
            effect.amplitude(),
            play > 0.0,
        );
        pedal.state = play;
        Some(report)
    }

    fn close_csl(&mut self) {
        for slot in &mut self.csl {
            let Some(pedal) = slot else {
                continue;
            };
            close_csl_file(&mut pedal.file);
        }
    }

    fn write_gt(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        if !self.gt.get(index).copied().unwrap_or(false) {
            return Vec::new();
        }
        let painted = self.script_colors(index, frame, usb::GT_NEO_LEDS);
        if let Some(detail) = painted.failure {
            eprintln!("{}", games::lua_call_failed_message(&detail));
        }
        self.send_gt_features(index, &painted.leds)
    }

    fn send_gt_features(&mut self, index: usize, colors: &[u8]) -> Vec<InitNotice> {
        let mut notices = Vec::new();
        let mut first = true;
        for report in usb::gt_neo_reports(colors) {
            if self.feature_index(index, &report, &mut notices) {
                first = false;
                continue;
            }
            if first {
                eprintln!("{}", games::MSG_GT_FEATURE_FAILED);
                first = false;
                continue;
            }
            eprintln!("{}", games::MSG_GT_FEATURE_CHUNK_FAILED);
            break;
        }
        notices
    }

    fn feature_index(
        &mut self,
        index: usize,
        report: &[u8],
        notices: &mut Vec<InitNotice>,
    ) -> bool {
        let Some(port) = self.ports.get_mut(index) else {
            return false;
        };
        feature_port(port, report, notices)
    }

    fn script_colors(&mut self, index: usize, frame: &Telemetry, total: i64) -> LuaPaint {
        let Some(host) = self.luas.get_mut(index).and_then(Option::as_mut) else {
            return LuaPaint::blank();
        };
        let clock = SystemClock::new();
        let mut sim = frame.clone_buf();
        let script = host.call_script(&mut sim, total, &clock);
        let Ok((tick, failure)) = script else {
            return LuaPaint::blank();
        };
        LuaPaint {
            leds: tick.leds,
            message: tick.message,
            failure: failure.map(|err| lua_detail(&err)),
        }
    }

    fn close_c12_lua(&mut self, notices: &mut Vec<InitNotice>) {
        for (index, slot) in self.luas.iter_mut().enumerate() {
            if slot.is_none() {
                continue;
            }
            if self.c12.get(index).copied().unwrap_or(false) {
                notices.push(notice(Level::Trace, games::MSG_C12_LUA_CLOSE));
            }
            slot.take();
        }
    }

    fn blank_c5(&mut self, notices: &mut Vec<InitNotice>) {
        let indexes: Vec<usize> = self
            .c5
            .iter()
            .enumerate()
            .filter(|(_, armed)| **armed)
            .map(|(index, _)| index)
            .collect();
        let mut blank = Telemetry::new();
        blank.set_rpms(C5_RELEASE_RPM);
        blank.set_maxrpm(C5_RELEASE_RPM);
        blank.set_gear(C5_RELEASE_GEAR);
        blank.set_velocity(C5_RELEASE_VELOCITY);
        for index in indexes {
            notices.extend(self.write_c5(index, &blank));
        }
    }

    fn blank_g29(&mut self, notices: &mut Vec<InitNotice>) {
        let indexes: Vec<usize> = self
            .g29
            .iter()
            .enumerate()
            .filter(|(_, armed)| **armed)
            .map(|(index, _)| index)
            .collect();
        for index in indexes {
            notices.extend(self.write_g29(index, G29_RELEASE_RPM, G29_RELEASE_RPM));
        }
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

    fn write_shiftlights(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        let Some(lights) = self.shift_lights.get(index).copied().flatten() else {
            return Vec::new();
        };
        let lit = serial::shiftlights_byte(frame.rpms(), frame.maxrpm(), lights);
        let notices = vec![
            notice(Level::Trace, games::shiftlights_lit_message(i32::from(lit))),
            notice(
                Level::Trace,
                games::arduino_copy_message(serial::SHIFT_PACKET_LEN),
            ),
        ];
        if let Some(port) = self.serials.get_mut(index) {
            let _ = write_serial_frame(port, &[lit]);
        }
        notices
    }

    fn write_simwind(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        let Some(fanpower) = self.simwind.get(index).copied().flatten() else {
            return Vec::new();
        };
        let report = serial::simwind_report(frame.velocity(), fanpower);
        let mph = i32::from(report[serial::SIMWIND_BYTE_SPEED]);
        let fan = i32::from(report[serial::SIMWIND_BYTE_FAN]);
        let notices = vec![
            notice(Level::Trace, games::simwind_speed_message(mph)),
            notice(Level::Trace, games::simwind_fan_message(fan, fanpower)),
            notice(
                Level::Trace,
                games::arduino_copy_message(serial::SIMWIND_PACKET_LEN),
            ),
        ];
        if let Some(port) = self.serials.get_mut(index) {
            let _ = write_serial_frame(port, &report);
        }
        notices
    }

    fn write_serial_haptic(
        &mut self,
        index: usize,
        frame: &Telemetry,
        now_ns: u64,
    ) -> Vec<InitNotice> {
        let Some(tick) = self.haptic_tick(index, frame, now_ns) else {
            return Vec::new();
        };
        let notices = serial_haptic_tick_notices(&tick);
        write_haptic_packet(self.serials.get_mut(index), &tick.step.packet);
        notices
    }

    fn write_simled(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        let Some(settings) = self.simled_settings(index) else {
            return Vec::new();
        };
        let Some(report) = serial::simled_report(
            frame.rpms(),
            frame.maxrpm(),
            settings.total,
            settings.startled,
            settings.endled,
        ) else {
            return Vec::new();
        };
        let notices = vec![notice(
            Level::Trace,
            games::shiftlights_lit_message(report.lit),
        )];
        if let Some(port) = self.serials.get_mut(index) {
            let _ = write_serial_frame(port, &report.bytes);
        }
        notices
    }

    fn simled_settings(&self, index: usize) -> Option<SimLed> {
        self.simled.get(index).and_then(Option::as_ref).copied()
    }

    fn blank_simled(&mut self) {
        for index in simled_indexes(&self.simled) {
            self.write_simled_blank(index);
        }
    }

    fn write_simled_blank(&mut self, index: usize) {
        let Some(settings) = self.simled_settings(index) else {
            return;
        };
        let Some(packet) = serial::simled_blank(settings.total) else {
            return;
        };
        let Some(port) = self.serials.get_mut(index) else {
            return;
        };
        let _ = write_serial_frame(port, &packet);
    }

    fn write_simled_custom(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        let Some(total) = self.custom_led_count(index) else {
            return Vec::new();
        };
        if total < serial::LED_FIRST {
            return Vec::new();
        }
        let Some(count) = usize::try_from(total).ok() else {
            return Vec::new();
        };
        self.write_custom_leds(index, frame, total, count)
    }

    fn custom_led_count(&self, index: usize) -> Option<i32> {
        self.custom_leds.get(index).copied().flatten()
    }

    fn write_custom_leds(
        &mut self,
        index: usize,
        frame: &Telemetry,
        total: i32,
        count: usize,
    ) -> Vec<InitNotice> {
        let painted = self.script_colors(index, frame, i64::from(total));
        if let Some(detail) = painted.failure.as_deref() {
            eprintln!("{}", games::lua_call_failed_message(detail));
        }
        let packet = serial::simled_packet(count, &painted.leds);
        let copied = i32::try_from(packet.len()).unwrap_or(i32::MAX);
        if let Some(port) = self.serials.get_mut(index) {
            let _ = write_serial_frame(port, &packet);
        }
        vec![
            notice(Level::Trace, games::arduino_copy_message(copied)),
            notice(Level::Trace, games::simled_custom_wrote_message(copied)),
        ]
    }

    fn write_arduino_custom(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        if !self.arduino_custom.get(index).copied().unwrap_or(false) {
            return Vec::new();
        }
        let painted = self.script_colors(index, frame, ARDUINO_CUSTOM_LED_TOTAL);
        if let Some(detail) = painted.failure.as_deref() {
            eprintln!("{}", games::lua_call_failed_message(detail));
        }
        let Some(message) = painted.message else {
            return Vec::new();
        };
        write_arduino_message(self.serials.get_mut(index), &message)
    }

    fn haptic_tick(
        &mut self,
        index: usize,
        frame: &Telemetry,
        now_ns: u64,
    ) -> Option<SerialHapticTick> {
        let haptic = self.serial_haptic.get_mut(index)?.as_mut()?;
        Some(haptic.tick(frame, now_ns))
    }

    fn blank_serial_haptic(&mut self, notices: &mut Vec<InitNotice>) {
        for index in haptic_indexes(&self.serial_haptic) {
            write_haptic_packet(self.serials.get_mut(index), &serial::haptic_stop_packet());
            notices.push(notice(Level::Info, games::MSG_SERIAL_HAPTIC_ZERO));
        }
    }

    fn write_moza_r5(&mut self, index: usize, frame: &Telemetry) -> Vec<InitNotice> {
        if !self.moza_r5.get(index).copied().unwrap_or(false) {
            return Vec::new();
        }
        let report = serial::moza_r5_report(frame.rpms(), frame.maxrpm());
        if let Some(port) = self.serials.get_mut(index) {
            let _ = write_serial_frame(port, &report.bytes);
        }
        vec![
            notice(
                Level::Debug,
                games::moza_r5_copy_message(serial::MOZA_R5_PACKET_LEN),
            ),
            notice(
                Level::Trace,
                games::moza_r5_write_message(&report.bytes, report.rpm, report.maxrpm),
            ),
        ]
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

fn write_arduino_message(port: Option<&mut SerialPort>, message: &str) -> Vec<InitNotice> {
    let copied = i32::try_from(message.len()).unwrap_or(i32::MAX);
    if let Some(port) = port {
        let _ = write_serial_frame(port, message.as_bytes());
    }
    vec![
        notice(Level::Trace, games::arduino_copy_message(copied)),
        notice(
            Level::Trace,
            games::arduino_custom_wrote_message(message, copied),
        ),
    ]
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

enum ConfigSource {
    Unset,
    NamedNone,
    File(PathBuf),
}

struct LuaPaint {
    leds: Vec<u8>,
    message: Option<String>,
    failure: Option<String>,
}

impl LuaPaint {
    fn blank() -> Self {
        Self {
            leds: Vec::new(),
            message: None,
            failure: None,
        }
    }
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
    supports_haptics: bool,
    pulse: Option<&mut PulseSession>,
) -> ProfileLoad {
    open_profile_with(
        config,
        index,
        disable_audio,
        USE_PULSES_DURING_PLAY,
        supports_haptics,
        pulse,
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
        ASSUME_SIM_SUPPORTS_HAPTICS,
        None,
        &mut HidAttempt::Probe {
            hid: &mut hid,
            serial: &mut serial,
            sysfs: None,
            now_ns,
            led_reply: None,
        },
    )
}

fn open_profile_with(
    config: &CargopitConfig,
    index: usize,
    disable_audio: bool,
    use_pulses: bool,
    supports_haptics: bool,
    mut pulse: Option<&mut PulseSession>,
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
    let mut sounds = Vec::new();
    let mut voices = Vec::new();
    let mut g29 = Vec::new();
    let mut c5 = Vec::new();
    let mut c12 = Vec::new();
    let mut gt = Vec::new();
    let mut arduino_custom = Vec::new();
    let mut moza_r5 = Vec::new();
    let mut csl = Vec::new();
    let mut p1000 = Vec::new();
    let mut simnet = Vec::new();
    let mut shift_lights = Vec::new();
    let mut simwind = Vec::new();
    let mut serial_haptic = Vec::new();
    let mut simled = Vec::new();
    let mut custom_leds = Vec::new();
    let mut luas = Vec::new();
    let mut setup_notices = Vec::new();
    let mut notices = Vec::new();
    let mut initialized = 0i32;
    for (slot, entry) in profile.devices.iter().enumerate() {
        let slot = i32::try_from(slot).unwrap_or(i32::MAX);
        let mut considered = consider_entry(
            entry,
            slot,
            devices.len() as i32,
            disable_audio,
            use_pulses,
            supports_haptics,
            attempt,
        );
        if !link_prepared_sound(&mut pulse, &mut considered) {
            considered.prepared = None;
        }
        setup_notices.extend(considered.setup_notices);
        notices.extend(considered.notices);
        let Some(prepared) = considered.prepared else {
            continue;
        };
        ports.push(considered.port);
        tables.push(considered.tach);
        serials.push(considered.serial);
        wheels.push(considered.wheel);
        sounds.push(considered.sound);
        voices.push(considered.voice);
        g29.push(considered.g29);
        c5.push(considered.c5);
        c12.push(considered.c12);
        gt.push(considered.gt);
        arduino_custom.push(considered.arduino_custom);
        moza_r5.push(considered.moza_r5);
        csl.push(considered.csl);
        p1000.push(considered.p1000);
        simnet.push(considered.simnet);
        shift_lights.push(considered.shift_lights);
        simwind.push(considered.simwind);
        serial_haptic.push(considered.serial_haptic);
        simled.push(considered.simled);
        custom_leds.push(considered.custom_leds);
        luas.push(considered.lua);
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
            sounds,
            voices,
            g29,
            c5,
            c12,
            gt,
            arduino_custom,
            moza_r5,
            csl,
            p1000,
            simnet,
            shift_lights,
            simwind,
            serial_haptic,
            simled,
            custom_leds,
            luas,
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
        sysfs: Option<&'a mut dyn FnMut(&str) -> i32>,
        now_ns: u64,
        led_reply: Option<&'a [u8]>,
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
    sound: Option<ShakerRequest>,
    voice: Option<SharedShaker>,
    g29: bool,
    c5: bool,
    c12: bool,
    lua: Option<LuaHost>,
    gt: bool,
    arduino_custom: bool,
    moza_r5: bool,
    csl: Option<CslPedal>,
    p1000: Option<P1000Pedal>,
    simnet: Option<SimNetPedal>,
    shift_lights: Option<i32>,
    simwind: Option<f64>,
    serial_haptic: Option<SerialHaptic>,
    simled: Option<SimLed>,
    custom_leds: Option<i32>,
}

enum SerialPort {
    Closed,
    Captured {
        path: String,
        frames: Vec<Vec<u8>>,
        led_reply: Option<Vec<u8>>,
    },
    Live {
        path: String,
        port: RealSerial,
    },
}

fn consider_entry(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    use_pulses: bool,
    supports_haptics: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(prep) = revburner_prep(entry, use_pulses) {
        return consider_tach(entry, slot, id, disable_audio, attempt, prep);
    }
    if shiftlights_entry(entry) {
        return consider_shiftlights(entry, slot, id, disable_audio, attempt);
    }
    if simwind_entry(entry) {
        return consider_simwind(entry, slot, id, disable_audio, attempt);
    }
    if serial_haptic_entry(entry) {
        return consider_serial_haptic(entry, slot, id, disable_audio, supports_haptics, attempt);
    }
    if simled_custom_entry(entry) {
        return consider_simled_custom(entry, slot, id, disable_audio, attempt);
    }
    if simled_entry(entry) {
        return consider_simled(entry, slot, id, disable_audio, attempt);
    }
    if arduino_custom_entry(entry) {
        return consider_arduino_custom(entry, slot, id, disable_audio, attempt);
    }
    if moza_new_entry(entry) {
        return consider_moza(entry, slot, id, disable_audio, attempt);
    }
    if moza_r5_entry(entry) {
        return consider_moza_r5(entry, slot, id, disable_audio, attempt);
    }
    if sound_entry(entry) {
        return consider_sound(entry, slot, id, disable_audio, supports_haptics);
    }
    if g29_entry(entry) {
        return consider_g29(entry, slot, id, disable_audio, attempt);
    }
    if c5_entry(entry) {
        return consider_c5(entry, slot, id, disable_audio, attempt);
    }
    if c12_entry(entry) {
        return consider_c12(entry, slot, id, disable_audio, attempt);
    }
    if gt_entry(entry) {
        return consider_gt(entry, slot, id, disable_audio, attempt);
    }
    if csl_entry(entry) {
        return consider_csl(entry, slot, id, disable_audio, supports_haptics, attempt);
    }
    if p1000_entry(entry) {
        return consider_p1000(entry, slot, id, disable_audio, supports_haptics, attempt);
    }
    if simnet_entry(entry) {
        return consider_simnet(entry, slot, id, disable_audio, supports_haptics, attempt);
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
    unopened(vec![notice(
        Level::Warn,
        games::could_not_initialize_message(class_label(entry_kind(entry))),
    )])
}

fn consider_sound(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    supports_haptics: bool,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let Some(effect) = device_effect(entry) else {
        return consider_closed(entry, slot, disable_audio);
    };
    finish_sound(entry, id, effect, supports_haptics)
}

fn finish_sound(entry: &DeviceEntry, id: i32, effect: i32, supports_haptics: bool) -> Considered {
    let opened = sound_host::open_sound(entry, effect, &device_port(entry), supports_haptics);
    let ready = opened.ready;
    let request = opened.request.clone();
    let voice = opened.voice.clone();
    let mut notices = sound_notices(opened);
    if !ready {
        notices.push(notice(
            Level::Warn,
            games::could_not_initialize_message(CLASS_SOUND),
        ));
        return unopened(notices);
    }
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
        sound: request,
        voice,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn link_prepared_sound(pulse: &mut Option<&mut PulseSession>, considered: &mut Considered) -> bool {
    if considered.prepared.is_none() {
        return true;
    }
    let Some(session) = pulse.as_mut() else {
        return true;
    };
    let Some(request) = considered.sound.as_ref() else {
        return true;
    };
    if let Err(err) = session.connect_shaker(request, considered.voice.clone()) {
        considered.notices.push(notice(
            Level::Error,
            games::sound_connect_error(&request.node, &request.sink, &err),
        ));
        considered.notices.push(notice(
            Level::Warn,
            games::could_not_initialize_message(CLASS_SOUND),
        ));
        considered.sound = None;
        considered.voice = None;
        return false;
    }
    true
}

fn sound_entry(entry: &DeviceEntry) -> bool {
    entry_kind(entry) == DeviceKind::Sound && device_effect(entry).is_some()
}

fn sound_notices(opened: sound_host::SoundOpen) -> Vec<InitNotice> {
    opened
        .notices
        .into_iter()
        .map(|(level, message)| notice(level, message))
        .collect()
}

fn unopened(notices: Vec<InitNotice>) -> Considered {
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: None,
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
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
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
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
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn revburner_prep(entry: &DeviceEntry, use_pulses: bool) -> Option<TachPrep> {
    if !usb_uses_revburner(entry) {
        return None;
    }
    Some(prepare_tach(entry, use_pulses))
}

fn prepare_tach(entry: &DeviceEntry, use_pulses: bool) -> TachPrep {
    match config_source(entry) {
        ConfigSource::Unset => TachPrep {
            notices: vec![
                notice(Level::Trace, games::MSG_TACH_CONFIG_NONE),
                notice(Level::Warn, games::MSG_TACH_CONFIG_REQUIRED),
            ],
            ready: false,
            map: inactive_tach(),
        },
        ConfigSource::NamedNone => TachPrep {
            notices: vec![notice(Level::Warn, games::MSG_TACH_CONFIG_REQUIRED)],
            ready: false,
            map: inactive_tach(),
        },
        ConfigSource::File(path) => prep_from_file(entry, use_pulses, &path),
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

fn config_source(entry: &DeviceEntry) -> ConfigSource {
    match entry.get_str(keys::KEY_CONFIG) {
        None => ConfigSource::Unset,
        Some(value) if value.eq_ignore_ascii_case(keys::CONFIG_VALUE_NONE) => {
            ConfigSource::NamedNone
        }
        Some(value) => ConfigSource::File(paths::expand_tilde(value)),
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
                sound: None,
                voice: None,
                g29: false,
                c5: false,
                c12: false,
                lua: None,
                gt: false,
                arduino_custom: false,
                moza_r5: false,
                csl: None,
                p1000: None,
                simnet: None,
                shift_lights: None,
                simwind: None,
                serial_haptic: None,
                simled: None,
                custom_leds: None,
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
            sound: None,
            voice: None,
            g29: false,
            c5: false,
            c12: false,
            lua: None,
            gt: false,
            arduino_custom: false,
            moza_r5: false,
            csl: None,
            p1000: None,
            simnet: None,
            shift_lights: None,
            simwind: None,
            serial_haptic: None,
            simled: None,
            custom_leds: None,
        },
        OpenedHid::Simulated => Considered {
            setup_notices: prep.notices,
            notices,
            prepared: Some(build_device(entry, id)),
            port: HidPort::Captured(Vec::new()),
            tach: prep.map,
            serial: SerialPort::Closed,
            wheel: None,
            sound: None,
            voice: None,
            g29: false,
            c5: false,
            c12: false,
            lua: None,
            gt: false,
            arduino_custom: false,
            moza_r5: false,
            csl: None,
            p1000: None,
            simnet: None,
            shift_lights: None,
            simwind: None,
            serial_haptic: None,
            simled: None,
            custom_leds: None,
        },
    }
}

fn open_revburner(attempt: &mut HidAttempt<'_>) -> OpenedHid {
    open_hid(attempt, REVBURNER_VENDOR_ID, REVBURNER_PRODUCT_ID)
}

fn open_hid(attempt: &mut HidAttempt<'_>, vendor: u16, product: u16) -> OpenedHid {
    match attempt {
        HidAttempt::Live { .. } => match RealHid::open(vendor, product) {
            Ok(hid) => OpenedHid::Live(hid),
            Err(_) => OpenedHid::Missing,
        },
        HidAttempt::Probe { hid, .. } => {
            if hid(vendor, product) {
                return OpenedHid::Simulated;
            }
            OpenedHid::Missing
        }
    }
}

fn g29_entry(entry: &DeviceEntry) -> bool {
    usb_wheel_hardware(entry, names::HARDWARE_LOGITECH_G29)
}

fn c5_entry(entry: &DeviceEntry) -> bool {
    usb_wheel_hardware(entry, names::HARDWARE_CAMMUS_C5)
}

fn c12_entry(entry: &DeviceEntry) -> bool {
    usb_wheel_hardware(entry, names::HARDWARE_CAMMUS_C12)
}

fn gt_entry(entry: &DeviceEntry) -> bool {
    usb_wheel_hardware(entry, names::HARDWARE_SIMAGIC_GT_NEO)
}

fn csl_entry(entry: &DeviceEntry) -> bool {
    usb_wheel_hardware(entry, names::HARDWARE_CSL_ELITE)
}

fn p1000_entry(entry: &DeviceEntry) -> bool {
    usb_wheel_hardware(entry, names::HARDWARE_SIMAGIC_P1000)
}

fn consider_p1000(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    supports_haptics: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let effect = device_effect(entry);
    let mut notices = Vec::new();
    if !supports_haptics {
        notices.push(notice(Level::Info, games::MSG_USB_NO_HAPTICS));
    }
    let arm = supports_haptics && effect.is_some();
    if let Some(effect_id) = effect.filter(|_| arm) {
        notices.extend(csl_haptic_notices(entry, effect_id));
    }
    notices.extend([
        notice(Level::Info, games::MSG_INIT_USB),
        notice(Level::Info, games::MSG_INIT_WHEEL),
        notice(Level::Info, games::p1000_init_message()),
    ]);
    let armed = effect
        .filter(|_| arm)
        .and_then(|effect_id| csl_effect(entry, effect_id));
    match open_hid(attempt, usb::P1000_VID, usb::P1000_PID) {
        OpenedHid::Missing => {
            notices.push(notice(Level::Error, games::p1000_missing_message()));
            usb_not_initialized(notices, games::ERROR_UNKNOWN)
        }
        OpenedHid::Live(hid) => finish_p1000(entry, id, notices, HidPort::Live(hid), armed),
        OpenedHid::Simulated => {
            finish_p1000(entry, id, notices, HidPort::Captured(Vec::new()), armed)
        }
    }
}

fn finish_p1000(
    entry: &DeviceEntry,
    id: i32,
    mut notices: Vec<InitNotice>,
    mut port: HidPort,
    effect: Option<HapticEffect>,
) -> Considered {
    let init = usb::p1000_init_report();
    let code = record_p1000(&mut port, &init, &mut notices);
    notices.push(notice(Level::Debug, games::p1000_init_result_message(code)));
    if code != usb::P1000_REPORT_OK {
        notices.push(notice(Level::Warn, games::p1000_problem_message()));
        notices.push(notice(Level::Debug, games::p1000_found_message()));
        drop(port);
        return usb_not_initialized(notices, games::ERROR_UNKNOWN);
    }
    notices.push(notice(Level::Debug, games::p1000_found_message()));
    p1000_ready(entry, id, notices, port, effect)
}

fn p1000_ready(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: HidPort,
    effect: Option<HapticEffect>,
) -> Considered {
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: Some(P1000Pedal {
            effect,
            state: HAPTIC_STATE_IDLE,
        }),
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

enum CslAttach {
    Captured,
    Missing,
    Permission,
    OpenFailed,
}

enum CslOpen {
    Ready(CslFile),
    Missing,
    Permission,
    OpenFailed,
}

enum CslFile {
    Closed,
    Captured { frames: Vec<Vec<u8>> },
    Live { file: std::fs::File },
}

struct CslPedal {
    file: CslFile,
    effect: Option<HapticEffect>,
    state: f64,
}

struct P1000Pedal {
    effect: Option<HapticEffect>,
    state: f64,
}

struct SimNetPedal {
    effect: Option<HapticEffect>,
    state: f64,
}

fn simnet_entry(entry: &DeviceEntry) -> bool {
    usb_wheel_hardware(entry, names::HARDWARE_SIMNET)
}

fn consider_simnet(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    supports_haptics: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let effect = device_effect(entry);
    let mut notices = Vec::new();
    if !supports_haptics {
        notices.push(notice(Level::Info, games::MSG_USB_NO_HAPTICS));
    }
    let arm = supports_haptics && effect.is_some();
    if let Some(effect_id) = effect.filter(|_| arm) {
        notices.extend(csl_haptic_notices(entry, effect_id));
    }
    notices.extend([
        notice(Level::Info, games::MSG_INIT_USB),
        notice(Level::Info, games::MSG_INIT_WHEEL),
        notice(Level::Info, games::MSG_SIMNET_INIT),
    ]);
    let armed = effect
        .filter(|_| arm)
        .and_then(|effect_id| csl_effect(entry, effect_id));
    match open_hid(attempt, usb::SIMNET_VID, usb::SIMNET_PID) {
        OpenedHid::Missing => {
            notices.push(notice(Level::Error, games::MSG_SIMNET_MISSING));
            usb_not_initialized(notices, games::ERROR_UNKNOWN)
        }
        OpenedHid::Live(hid) => finish_simnet(entry, id, notices, HidPort::Live(hid), armed),
        OpenedHid::Simulated => {
            finish_simnet(entry, id, notices, HidPort::Captured(Vec::new()), armed)
        }
    }
}

fn finish_simnet(
    entry: &DeviceEntry,
    id: i32,
    mut notices: Vec<InitNotice>,
    port: HidPort,
    effect: Option<HapticEffect>,
) -> Considered {
    notices.push(notice(
        Level::Debug,
        games::simnet_found_message(simnet_handle(&port)),
    ));
    simnet_ready(entry, id, notices, port, effect)
}

fn simnet_handle(port: &HidPort) -> i32 {
    match port {
        HidPort::Live(hid) => hid.handle_code(),
        HidPort::Captured(_) | HidPort::Closed => games::SIMNET_CAPTURED_HANDLE,
    }
}

fn simnet_ready(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: HidPort,
    effect: Option<HapticEffect>,
) -> Considered {
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: Some(SimNetPedal {
            effect,
            state: HAPTIC_STATE_IDLE,
        }),
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn consider_csl(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    supports_haptics: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let effect = device_effect(entry);
    let mut notices = Vec::new();
    if !supports_haptics {
        notices.push(notice(Level::Info, games::MSG_USB_NO_HAPTICS));
    }
    let arm = supports_haptics && effect.is_some();
    if arm {
        if let Some(effect_id) = effect {
            notices.extend(csl_haptic_notices(entry, effect_id));
        }
    }
    notices.extend([
        notice(Level::Info, games::MSG_INIT_USB),
        notice(Level::Info, games::MSG_INIT_WHEEL),
        notice(Level::Info, games::MSG_CSL_ATTEMPT),
        notice(Level::Info, games::MSG_CSL_INIT),
    ]);
    let armed = effect
        .filter(|_| arm)
        .and_then(|effect_id| csl_effect(entry, effect_id));
    match attach_csl(attempt) {
        CslOpen::Missing => csl_failed(notices, games::MSG_CSL_MISSING, games::ERROR_UNKNOWN),
        CslOpen::Permission => csl_failed(
            notices,
            games::MSG_CSL_PERMISSION,
            games::USB_INIT_CSL_PERMISSION,
        ),
        CslOpen::OpenFailed => csl_failed(notices, games::MSG_CSL_OPEN, games::ERROR_UNKNOWN),
        CslOpen::Ready(file) => csl_ready(entry, id, notices, file, armed),
    }
}

fn csl_failed(mut notices: Vec<InitNotice>, message: &str, code: i32) -> Considered {
    notices.push(notice(Level::Error, message));
    usb_not_initialized(notices, code)
}

fn csl_ready(
    entry: &DeviceEntry,
    id: i32,
    mut notices: Vec<InitNotice>,
    file: CslFile,
    effect: Option<HapticEffect>,
) -> Considered {
    notices.push(notice(Level::Debug, games::MSG_CSL_FOUND));
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: Some(CslPedal {
            file,
            effect,
            state: HAPTIC_STATE_IDLE,
        }),
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn attach_csl(attempt: &mut HidAttempt<'_>) -> CslOpen {
    match attempt {
        HidAttempt::Live { .. } => match usb::locate_csl_pedals() {
            usb::CslLocate::Missing => CslOpen::Missing,
            usb::CslLocate::Permission => CslOpen::Permission,
            usb::CslLocate::Found(path) => open_csl_file(&path),
        },
        HidAttempt::Probe { sysfs, .. } => {
            let Some(probe) = sysfs else {
                return CslOpen::Missing;
            };
            match csl_attach(probe(usb::CSL_SYSFS_GLOB)) {
                CslAttach::Captured => CslOpen::Ready(CslFile::Captured { frames: Vec::new() }),
                CslAttach::Missing => CslOpen::Missing,
                CslAttach::Permission => CslOpen::Permission,
                CslAttach::OpenFailed => CslOpen::OpenFailed,
            }
        }
    }
}

fn csl_attach(code: i32) -> CslAttach {
    match code {
        CSL_PROBE_CAPTURED => CslAttach::Captured,
        CSL_PROBE_PERMISSION => CslAttach::Permission,
        CSL_PROBE_OPEN_FAILED => CslAttach::OpenFailed,
        CSL_PROBE_MISSING => CslAttach::Missing,
        _ => CslAttach::Missing,
    }
}

fn open_csl_file(path: &str) -> CslOpen {
    if path.is_empty() {
        return CslOpen::Missing;
    }
    match OpenOptions::new().write(true).open(path) {
        Ok(file) => CslOpen::Ready(CslFile::Live { file }),
        Err(_) => CslOpen::OpenFailed,
    }
}

fn csl_play(pedal: &mut CslPedal, frame: &Telemetry, clock: &VirtualClock) -> Option<f64> {
    haptic_play(&mut pedal.effect, frame, clock)
}

fn haptic_play(
    effect: &mut Option<HapticEffect>,
    frame: &Telemetry,
    clock: &VirtualClock,
) -> Option<f64> {
    Some(effect.as_mut()?.play_with_clock(frame, clock))
}

fn write_csl_bytes(file: &mut CslFile, bytes: &[u8]) {
    match file {
        CslFile::Captured { frames } => frames.push(bytes.to_vec()),
        CslFile::Live { file } => {
            let _ = file.write_all(bytes);
            let _ = file.flush();
        }
        CslFile::Closed => {}
    }
}

fn close_csl_file(file: &mut CslFile) {
    if let CslFile::Live { file } = file {
        let _ = file.flush();
    }
    if matches!(file, CslFile::Live { .. }) {
        *file = CslFile::Closed;
    }
}

fn csl_haptic_notices(entry: &DeviceEntry, effect: i32) -> Vec<InitNotice> {
    let tyre = csl_tyre(entry, effect);
    let duration = csl_duration(entry, effect);
    let frequency = entry
        .get_i64(keys::KEY_FREQUENCY)
        .unwrap_or(HAPTIC_FREQUENCY_DEFAULT);
    let amplitude = entry
        .get_i64(keys::KEY_AMPLITUDE)
        .unwrap_or(HAPTIC_AMPLITUDE_UNITY);
    let motor = configured_motor_i64(entry);
    let mut notices = Vec::new();
    match csl_vibration_phrase(effect) {
        Some(phrase) => notices.push(notice(Level::Info, games::haptic_effect_message(phrase))),
        None => notices.push(notice(Level::Warn, games::unknown_haptic_message(effect))),
    }
    notices.push(notice(
        Level::Info,
        games::haptic_summary_message(effect, tyre),
    ));
    notices.push(notice(
        Level::Trace,
        games::haptic_duration_message(duration),
    ));
    notices.push(notice(
        Level::Trace,
        games::haptic_frequency_message(frequency),
    ));
    notices.push(notice(
        Level::Trace,
        games::haptic_amplitude_message(amplitude),
    ));
    notices.push(notice(Level::Trace, games::haptic_motor_message(motor)));
    notices
}

fn csl_effect(entry: &DeviceEntry, effect: i32) -> Option<HapticEffect> {
    let kind = VibrationEffect::from_id(effect)?;
    Some(HapticEffect::new(&HapticSettings {
        effect: kind,
        tyre: TyreId::from_id(csl_tyre(entry, effect)),
        threshold: entry
            .get_f64(keys::KEY_THRESHOLD)
            .unwrap_or(HAPTIC_THRESHOLD_DEFAULT),
        frequency: u32_from_i64(
            entry
                .get_i64(keys::KEY_FREQUENCY)
                .unwrap_or(HAPTIC_FREQUENCY_DEFAULT),
        ),
        amplitude: u32_from_i64(
            entry
                .get_i64(keys::KEY_AMPLITUDE)
                .unwrap_or(HAPTIC_AMPLITUDE_UNITY),
        ),
        duration: csl_duration(entry, effect),
        motor_position: configured_motor(entry),
        ..HapticSettings::default()
    }))
}

fn csl_tyre(entry: &DeviceEntry, effect: i32) -> i32 {
    if !csl_effect_uses_tyre(effect) {
        return names::TYRE_FRONT_LEFT;
    }
    let Some(name) = entry.get_str(keys::KEY_TYRE) else {
        return names::TYRE_FRONT_LEFT;
    };
    names::lookup(names::TYRES, name).unwrap_or(names::TYRE_ALL_FOUR)
}

fn csl_effect_uses_tyre(effect: i32) -> bool {
    matches!(
        effect,
        names::EFFECT_TYRE_SLIP
            | names::EFFECT_TYRE_LOCK
            | names::EFFECT_ABS
            | names::EFFECT_SUSPENSION
    )
}

fn csl_duration(entry: &DeviceEntry, effect: i32) -> f64 {
    if let Some(value) = entry.get_f64(keys::KEY_DURATION) {
        return value;
    }
    if effect == names::EFFECT_GEAR {
        return GEAR_DURATION_DEFAULT_S;
    }
    HAPTIC_DURATION_UNSET
}

fn csl_vibration_phrase(effect: i32) -> Option<&'static str> {
    match effect {
        names::EFFECT_ENGINE => Some(games::VIBRATION_ENGINE),
        names::EFFECT_GEAR => Some(games::VIBRATION_GEAR),
        names::EFFECT_TYRE_SLIP => Some(games::VIBRATION_SLIP),
        names::EFFECT_TYRE_LOCK => Some(games::VIBRATION_LOCK),
        names::EFFECT_ABS => Some(games::VIBRATION_ABS),
        names::EFFECT_SUSPENSION => Some(games::VIBRATION_SUSPENSION),
        _ => None,
    }
}

fn u32_from_i64(value: i64) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

fn configured_motor_i64(entry: &DeviceEntry) -> i64 {
    entry
        .get_i64(keys::KEY_MOTORS)
        .unwrap_or(HAPTIC_MOTOR_DEFAULT)
}

fn configured_motor(entry: &DeviceEntry) -> u32 {
    u32_from_i64(configured_motor_i64(entry))
}

fn usb_wheel_hardware(entry: &DeviceEntry, hardware: i32) -> bool {
    if entry_kind(entry) != DeviceKind::Usb {
        return false;
    }
    let kind = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    if names::lookup(names::USB_TYPES, kind) != Some(names::SUBTYPE_USB_WHEEL) {
        return false;
    }
    let name = entry.get_str(keys::KEY_SUBTYPE).unwrap_or("");
    names::lookup(names::HARDWARE, name) == Some(hardware)
}

fn consider_g29(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let notices = vec![
        notice(Level::Info, games::MSG_INIT_USB),
        notice(Level::Info, games::MSG_INIT_WHEEL),
        notice(Level::Info, games::MSG_G29_ATTEMPT),
        notice(Level::Info, games::MSG_G29_INIT),
    ];
    match open_hid(attempt, usb::G29_VID, usb::G29_PID) {
        OpenedHid::Missing => wheel_missing(notices, games::MSG_G29_MISSING),
        OpenedHid::Live(hid) => g29_ready(entry, id, notices, HidPort::Live(hid)),
        OpenedHid::Simulated => g29_ready(entry, id, notices, HidPort::Captured(Vec::new())),
    }
}

fn consider_c5(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let notices = vec![
        notice(Level::Info, games::MSG_INIT_USB),
        notice(Level::Info, games::MSG_INIT_WHEEL),
        notice(Level::Info, games::MSG_C5_ATTEMPT),
        notice(Level::Info, games::MSG_C5_INIT),
    ];
    match open_hid(attempt, usb::C5_VID, usb::C5_PID) {
        OpenedHid::Missing => wheel_missing(notices, games::MSG_C5_MISSING),
        OpenedHid::Live(hid) => c5_ready(entry, id, notices, HidPort::Live(hid)),
        OpenedHid::Simulated => c5_ready(entry, id, notices, HidPort::Captured(Vec::new())),
    }
}

fn consider_c12(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    let (setup, lua_path) = lua_config(entry);
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(setup, skip, slot);
    }
    let notices = vec![
        notice(Level::Info, games::MSG_INIT_USB),
        notice(Level::Info, games::MSG_INIT_WHEEL),
        notice(Level::Info, games::MSG_C12_ATTEMPT),
        notice(Level::Info, games::MSG_C12_INIT),
    ];
    let opened = match open_hid(attempt, usb::C12_VID, usb::C12_PID) {
        OpenedHid::Missing => wheel_missing(notices, games::MSG_C12_MISSING),
        OpenedHid::Live(hid) => finish_c12(entry, id, notices, HidPort::Live(hid), lua_path),
        OpenedHid::Simulated => {
            finish_c12(entry, id, notices, HidPort::Captured(Vec::new()), lua_path)
        }
    };
    attach_setup(opened, setup)
}

fn lua_config(entry: &DeviceEntry) -> (Vec<InitNotice>, Option<PathBuf>) {
    match config_source(entry) {
        ConfigSource::Unset => (
            vec![notice(Level::Trace, games::MSG_TACH_CONFIG_NONE)],
            None,
        ),
        ConfigSource::NamedNone => (Vec::new(), None),
        ConfigSource::File(path) => {
            let shown = path.display().to_string();
            (
                vec![notice(
                    Level::Trace,
                    games::tach_config_load_message(&shown),
                )],
                Some(path),
            )
        }
    }
}

fn consider_gt(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    let (setup, lua_path) = lua_config(entry);
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(setup, skip, slot);
    }
    let mut notices = vec![
        notice(Level::Info, games::MSG_INIT_USB),
        notice(Level::Info, games::MSG_INIT_WHEEL),
        notice(Level::Info, games::MSG_GT_ATTEMPT),
    ];
    let Some(path) = lua_path else {
        notices.push(notice(Level::Error, games::MSG_GT_NEEDS_CONFIG));
        return attach_setup(
            usb_not_initialized(notices, games::USB_INIT_LUA_FAILED),
            setup,
        );
    };
    notices.push(notice(Level::Info, games::MSG_GT_INIT));
    let opened = match open_hid(attempt, usb::GT_NEO_VID, usb::GT_NEO_PID) {
        OpenedHid::Missing => wheel_missing(notices, games::MSG_GT_MISSING),
        OpenedHid::Live(hid) => finish_gt(entry, id, notices, HidPort::Live(hid), path),
        OpenedHid::Simulated => finish_gt(entry, id, notices, HidPort::Captured(Vec::new()), path),
    };
    attach_setup(opened, setup)
}

fn finish_gt(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: HidPort,
    path: PathBuf,
) -> Considered {
    let mut ready = gt_ready(entry, id, notices, port);
    ready.notices.push(notice(Level::Trace, games::MSG_GT_LUA));
    match LuaHost::load_file(&path, LuaLedMode::Usb) {
        Ok(host) => {
            ready.lua = Some(host);
            ready
        }
        Err(detail) => {
            ready
                .notices
                .push(notice(Level::Error, games::MSG_C12_LUA_ISSUE));
            eprintln!("{}", games::lua_load_failed_message(&detail));
            let notices = std::mem::take(&mut ready.notices);
            drop(ready);
            usb_not_initialized(notices, games::USB_INIT_LUA_FAILED)
        }
    }
}

fn gt_ready(
    entry: &DeviceEntry,
    id: i32,
    mut notices: Vec<InitNotice>,
    port: HidPort,
) -> Considered {
    notices.push(notice(Level::Debug, games::MSG_GT_FOUND));
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: true,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn finish_c12(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: HidPort,
    lua_path: Option<PathBuf>,
) -> Considered {
    let mut ready = c12_ready(entry, id, notices, port);
    let Some(path) = lua_path else {
        return ready;
    };
    ready.notices.push(notice(Level::Trace, games::MSG_C12_LUA));
    match LuaHost::load_file(&path, LuaLedMode::Usb) {
        Ok(host) => {
            ready.lua = Some(host);
            ready
        }
        Err(detail) => {
            ready
                .notices
                .push(notice(Level::Error, games::MSG_C12_LUA_ISSUE));
            eprintln!("{}", games::lua_load_failed_message(&detail));
            let notices = std::mem::take(&mut ready.notices);
            drop(ready);
            usb_not_initialized(notices, games::USB_INIT_LUA_FAILED)
        }
    }
}

fn attach_setup(mut considered: Considered, setup: Vec<InitNotice>) -> Considered {
    considered.setup_notices = setup;
    considered
}

fn usb_not_initialized(mut notices: Vec<InitNotice>, code: i32) -> Considered {
    notices.push(notice(Level::Warn, games::usb_init_error_message(code)));
    notices.push(notice(
        Level::Warn,
        games::could_not_initialize_message(CLASS_USB),
    ));
    unopened(notices)
}

fn wheel_missing(mut notices: Vec<InitNotice>, missing: &str) -> Considered {
    notices.push(notice(Level::Error, missing));
    usb_not_initialized(notices, games::ERROR_UNKNOWN)
}

fn g29_ready(
    entry: &DeviceEntry,
    id: i32,
    mut notices: Vec<InitNotice>,
    port: HidPort,
) -> Considered {
    notices.push(notice(Level::Debug, games::MSG_G29_FOUND));
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
        sound: None,
        voice: None,
        g29: true,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn c5_ready(
    entry: &DeviceEntry,
    id: i32,
    mut notices: Vec<InitNotice>,
    port: HidPort,
) -> Considered {
    notices.push(notice(Level::Debug, games::MSG_C5_FOUND));
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: true,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn c12_ready(
    entry: &DeviceEntry,
    id: i32,
    mut notices: Vec<InitNotice>,
    port: HidPort,
) -> Considered {
    notices.push(notice(Level::Debug, games::MSG_C12_FOUND));
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port,
        tach: inactive_tach(),
        serial: SerialPort::Closed,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: true,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn shiftlights_entry(entry: &DeviceEntry) -> bool {
    if entry_kind(entry) != DeviceKind::Serial {
        return false;
    }
    let kind = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    names::lookup(names::SERIAL_TYPES, kind) == Some(names::SUBTYPE_SHIFT_LIGHTS)
}

fn consider_shiftlights(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let path = device_port(entry);
    let configured = entry.get_i64(keys::KEY_BAUD).unwrap_or(keys::BAUD_DEFAULT);
    let lights = config_i32(
        entry
            .get_i64(keys::KEY_NUMLIGHTS)
            .unwrap_or(keys::NUMLIGHTS_DEFAULT),
    );
    let mut notices = serial_lookup_notices(
        &path,
        configured,
        names::SUBTYPE_SHIFT_LIGHTS,
        games::MSG_SHIFTLIGHTS_INIT.to_string(),
    );
    let baud = serial_baud(configured);
    match open_serial(attempt, &path, baud) {
        OpenedSerial::Missing => serial_open_failed(notices),
        OpenedSerial::Ready(port) => {
            notices.extend(moza_ready_notices(baud));
            shiftlights_ready(entry, id, notices, port, lights)
        }
    }
}

fn serial_lookup_notices(
    path: &str,
    configured: i64,
    subtype: i32,
    init: String,
) -> Vec<InitNotice> {
    let mut notices = serial_preamble(subtype, init);
    notices.extend(serial_open_attempt_notices(path, configured));
    notices
}

fn serial_preamble(subtype: i32, init: String) -> Vec<InitNotice> {
    vec![
        notice(Level::Trace, games::serial_subtype_message(subtype)),
        notice(Level::Info, init),
    ]
}

fn serial_open_attempt_notices(path: &str, configured: i64) -> Vec<InitNotice> {
    vec![
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

fn serial_baud(configured: i64) -> u32 {
    u32::try_from(configured).unwrap_or(0)
}

fn config_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

fn serial_open_failed(mut notices: Vec<InitNotice>) -> Considered {
    notices.push(notice(Level::Error, games::MSG_SERIAL_OPEN_ERROR));
    serial_rejected(notices, games::SERIAL_OPEN_ERROR)
}

fn serial_rejected(mut notices: Vec<InitNotice>, code: i32) -> Considered {
    notices.push(notice(Level::Warn, games::serial_init_error_message(code)));
    notices.push(notice(
        Level::Warn,
        games::could_not_initialize_message(CLASS_SERIAL),
    ));
    unopened(notices)
}

fn shiftlights_ready(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: SerialPort,
    lights: i32,
) -> Considered {
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: port,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: Some(lights),
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn simwind_entry(entry: &DeviceEntry) -> bool {
    if entry_kind(entry) != DeviceKind::Serial {
        return false;
    }
    let kind = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    names::lookup(names::SERIAL_TYPES, kind) == Some(names::SUBTYPE_SIM_WIND)
}

fn consider_simwind(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let path = device_port(entry);
    let configured = entry.get_i64(keys::KEY_BAUD).unwrap_or(keys::BAUD_DEFAULT);
    let fanpower = entry
        .get_f64(keys::KEY_FANPOWER)
        .unwrap_or(keys::FANPOWER_DEFAULT);
    let mut notices = serial_lookup_notices(
        &path,
        configured,
        names::SUBTYPE_SIM_WIND,
        games::simwind_init_message(fanpower),
    );
    let baud = serial_baud(configured);
    match open_serial(attempt, &path, baud) {
        OpenedSerial::Missing => serial_open_failed(notices),
        OpenedSerial::Ready(port) => {
            notices.extend(moza_ready_notices(baud));
            simwind_ready(entry, id, notices, port, fanpower)
        }
    }
}

fn simwind_ready(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: SerialPort,
    fanpower: f64,
) -> Considered {
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: port,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: Some(fanpower),
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn serial_haptic_entry(entry: &DeviceEntry) -> bool {
    if entry_kind(entry) != DeviceKind::Serial {
        return false;
    }
    let kind = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    names::lookup(names::SERIAL_TYPES, kind) == Some(names::SUBTYPE_SERIAL_HAPTIC)
}

fn consider_serial_haptic(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    supports_haptics: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let mut notices = serial_preamble(
        names::SUBTYPE_SERIAL_HAPTIC,
        games::MSG_SERIAL_HAPTIC_INIT.to_string(),
    );
    if !supports_haptics {
        return serial_rejected(notices, games::UNSUPPORTED_SIM_FEATURE);
    }
    let effect_id = device_effect(entry).unwrap_or(names::EFFECT_ENGINE);
    notices.extend(csl_haptic_notices(entry, effect_id));
    let path = device_port(entry);
    let configured = entry.get_i64(keys::KEY_BAUD).unwrap_or(keys::BAUD_DEFAULT);
    notices.extend(serial_open_attempt_notices(&path, configured));
    let baud = serial_baud(configured);
    let haptic = serial_haptic_from(entry, effect_id);
    match open_serial(attempt, &path, baud) {
        OpenedSerial::Missing => serial_open_failed(notices),
        OpenedSerial::Ready(port) => {
            notices.extend(moza_ready_notices(baud));
            serial_haptic_ready(entry, id, notices, port, haptic)
        }
    }
}

fn serial_haptic_from(entry: &DeviceEntry, effect_id: i32) -> SerialHaptic {
    SerialHaptic {
        effect: csl_effect(entry, effect_id),
        effect_id,
        ampfactor: entry
            .get_f64(keys::KEY_AMPFACTOR)
            .unwrap_or(keys::AMPFACTOR_DEFAULT),
        motor: configured_motor(entry),
        state: serial::SerialHapticState::new(),
    }
}

fn serial_haptic_ready(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: SerialPort,
    haptic: SerialHaptic,
) -> Considered {
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: port,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: Some(haptic),
        simled: None,
        custom_leds: None,
    }
}

struct SerialHaptic {
    effect: Option<HapticEffect>,
    effect_id: i32,
    ampfactor: f64,
    motor: u32,
    state: serial::SerialHapticState,
}

struct SerialHapticTick {
    step: serial::SerialHapticStep,
    effect_id: i32,
    raw: f64,
    ampfactor: f64,
}

impl SerialHaptic {
    fn tick(&mut self, frame: &Telemetry, now_ns: u64) -> SerialHapticTick {
        let raw = self.raw_play(frame, now_ns);
        let step = self.state.tick(raw, self.ampfactor, self.motor);
        SerialHapticTick {
            step,
            effect_id: self.effect_id,
            raw,
            ampfactor: self.ampfactor,
        }
    }

    fn raw_play(&mut self, frame: &Telemetry, now_ns: u64) -> f64 {
        let Some(effect) = self.effect.as_mut() else {
            return HAPTIC_STATE_IDLE;
        };
        let clock = VirtualClock::from_monotonic_ns(now_ns);
        effect.play_with_clock(frame, &clock)
    }
}

fn serial_haptic_tick_notices(tick: &SerialHapticTick) -> Vec<InitNotice> {
    let mut notices = vec![notice(
        Level::Trace,
        games::MSG_SERIAL_HAPTIC_UPDATING.to_string(),
    )];
    for channel in tick.step.channels.into_iter().flatten() {
        notices.push(notice(
            Level::Trace,
            games::serial_haptic_channel_message(
                tick.effect_id,
                i32::from(channel.speed),
                channel.motor,
                tick.raw,
                tick.ampfactor,
            ),
        ));
    }
    notices.push(notice(
        Level::Trace,
        games::arduino_copy_message(serial::HAPTIC_PACKET_LEN),
    ));
    notices
}

fn write_haptic_packet(port: Option<&mut SerialPort>, packet: &[u8]) {
    let Some(port) = port else {
        return;
    };
    let _ = write_serial_frame(port, packet);
}

fn haptic_indexes(haptics: &[Option<SerialHaptic>]) -> Vec<usize> {
    haptics
        .iter()
        .enumerate()
        .filter_map(|(index, haptic)| haptic.as_ref().map(|_| index))
        .collect()
}

fn arduino_custom_entry(entry: &DeviceEntry) -> bool {
    if entry_kind(entry) != DeviceKind::Serial {
        return false;
    }
    let kind = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    names::lookup(names::SERIAL_TYPES, kind) == Some(names::SUBTYPE_ARDUINO_CUSTOM)
}

fn consider_arduino_custom(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let path = device_port(entry);
    let configured = entry.get_i64(keys::KEY_BAUD).unwrap_or(keys::BAUD_DEFAULT);
    let Some(script) = device_script_path(entry) else {
        return arduino_custom_without_script(&path, configured);
    };
    let mut notices = serial_lookup_notices(
        &path,
        configured,
        names::SUBTYPE_ARDUINO_CUSTOM,
        games::MSG_ARDUINO_CUSTOM_INIT.to_string(),
    );
    let baud = serial_baud(configured);
    let port = match open_serial(attempt, &path, baud) {
        OpenedSerial::Missing => return serial_open_failed(notices),
        OpenedSerial::Ready(port) => port,
    };
    notices.extend(moza_ready_notices(baud));
    load_arduino_script(entry, id, notices, port, &path, script)
}

fn arduino_custom_without_script(path: &str, configured: i64) -> Considered {
    let mut notices = serial_preamble(
        names::SUBTYPE_ARDUINO_CUSTOM,
        games::MSG_ARDUINO_CUSTOM_INIT.to_string(),
    );
    notices.push(notice(Level::Info, games::MSG_SERIAL_START));
    notices.push(notice(
        Level::Info,
        games::serial_init_port_message(path, configured),
    ));
    serial_rejected(notices, games::SERIAL_OPEN_ERROR)
}

fn load_arduino_script(
    entry: &DeviceEntry,
    id: i32,
    mut notices: Vec<InitNotice>,
    port: SerialPort,
    path: &str,
    script: PathBuf,
) -> Considered {
    notices.push(notice(Level::Info, games::MSG_SIMLED_LUA_INIT));
    match LuaHost::load_file(&script, LuaLedMode::Serial) {
        Ok(host) => {
            notices.push(notice(Level::Info, games::MSG_SIMLED_LUA_OK));
            arduino_custom_ready(entry, id, notices, port, host)
        }
        Err(detail) => {
            eprintln!("{}", games::lua_load_failed_message(&detail));
            drop(port);
            reject_opened_serial(path, notices)
        }
    }
}

fn arduino_custom_ready(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: SerialPort,
    host: LuaHost,
) -> Considered {
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: port,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: Some(host),
        gt: false,
        arduino_custom: true,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn simled_custom_entry(entry: &DeviceEntry) -> bool {
    if entry_kind(entry) != DeviceKind::Serial || !simled_script(entry) {
        return false;
    }
    let kind = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    names::lookup(names::SERIAL_TYPES, kind) == Some(names::SUBTYPE_SIMLED)
}

fn consider_simled_custom(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let path = device_port(entry);
    let configured = entry.get_i64(keys::KEY_BAUD).unwrap_or(keys::BAUD_DEFAULT);
    let mut notices = serial_lookup_notices(
        &path,
        configured,
        names::SUBTYPE_SIMLED,
        games::MSG_SIMLED_CUSTOM_INIT.to_string(),
    );
    let baud = serial_baud(configured);
    let mut port = match open_serial(attempt, &path, baud) {
        OpenedSerial::Missing => return serial_open_failed(notices),
        OpenedSerial::Ready(port) => port,
    };
    notices.extend(moza_ready_notices(baud));
    let query = query_simled_count(&mut port);
    notices.extend(query.notices);
    let Some(count) = query.count else {
        return reject_simled_count(&path, notices);
    };
    notices.push(notice(Level::Info, games::simled_count_message(count)));
    load_simled_script(entry, id, notices, port, count, &path)
}

fn reject_simled_count(path: &str, mut notices: Vec<InitNotice>) -> Considered {
    notices.push(notice(
        Level::Info,
        games::simled_count_message(SIMLED_COUNT_UNSET),
    ));
    reject_opened_serial(path, notices)
}

fn reject_opened_serial(path: &str, mut notices: Vec<InitNotice>) -> Considered {
    notices.push(notice(Level::Debug, games::serial_free_message(path)));
    serial_rejected(notices, games::SERIAL_OPEN_ERROR)
}

fn load_simled_script(
    entry: &DeviceEntry,
    id: i32,
    mut notices: Vec<InitNotice>,
    port: SerialPort,
    count: i32,
    path: &str,
) -> Considered {
    let Some(script) = device_script_path(entry) else {
        return reject_opened_serial(path, notices);
    };
    notices.push(notice(Level::Info, games::MSG_SIMLED_LUA_INIT));
    match LuaHost::load_file(&script, LuaLedMode::Serial) {
        Ok(host) => {
            notices.push(notice(Level::Info, games::MSG_SIMLED_LUA_OK));
            simled_custom_ready(entry, id, notices, port, count, host)
        }
        Err(detail) => {
            eprintln!("{}", games::lua_load_failed_message(&detail));
            drop(port);
            reject_opened_serial(path, notices)
        }
    }
}

fn device_script_path(entry: &DeviceEntry) -> Option<PathBuf> {
    match config_source(entry) {
        ConfigSource::File(path) => Some(path),
        ConfigSource::Unset | ConfigSource::NamedNone => None,
    }
}

fn simled_custom_ready(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: SerialPort,
    count: i32,
    host: LuaHost,
) -> Considered {
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: port,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: Some(host),
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: Some(count),
    }
}

struct SimLedQuery {
    notices: Vec<InitNotice>,
    count: Option<i32>,
}

impl SimLedQuery {
    fn ready(notices: Vec<InitNotice>, count: i32) -> Self {
        Self {
            notices,
            count: Some(count),
        }
    }

    fn rejected(notices: Vec<InitNotice>) -> Self {
        Self {
            notices,
            count: None,
        }
    }
}

fn query_simled_count(port: &mut SerialPort) -> SimLedQuery {
    if matches!(port, SerialPort::Live { .. }) {
        return query_live_simled(port);
    }
    query_captured_simled(port)
}

fn query_captured_simled(port: &mut SerialPort) -> SimLedQuery {
    let reply = captured_led_reply(port);
    let mut notices = Vec::new();
    for _attempt in 0..serial::SIMLED_QUERY_ATTEMPTS {
        record_count_attempt(&mut notices);
        let _ = write_serial_frame(port, &serial::simled_count_query());
        if let Some(done) = count_from_reply(&reply, &mut notices) {
            return done;
        }
    }
    SimLedQuery::ready(notices, SIMLED_COUNT_UNSET)
}

fn query_live_simled(port: &mut SerialPort) -> SimLedQuery {
    let mut notices = Vec::new();
    for _attempt in 0..serial::SIMLED_QUERY_ATTEMPTS {
        record_count_attempt(&mut notices);
        let _ = write_serial_frame(port, &serial::simled_count_query());
        if let Some(done) = live_count_step(port, &mut notices) {
            return done;
        }
    }
    SimLedQuery::ready(notices, SIMLED_COUNT_UNSET)
}

fn live_count_step(port: &mut SerialPort, notices: &mut Vec<InitNotice>) -> Option<SimLedQuery> {
    match read_live_reply(port) {
        LiveReply::Waiting => None,
        LiveReply::Failed => {
            notices.push(notice(
                Level::Error,
                games::simled_count_wait_message(SIMLED_INPUT_WAIT_ERROR),
            ));
            Some(SimLedQuery::rejected(std::mem::take(notices)))
        }
        LiveReply::Bytes(bytes) => count_from_reply(&bytes, notices),
    }
}

fn count_from_reply(reply: &[u8], notices: &mut Vec<InitNotice>) -> Option<SimLedQuery> {
    match serial::parse_simled_count(reply) {
        serial::SimLedCount::Waiting => None,
        serial::SimLedCount::Invalid => {
            notices.push(notice(
                Level::Error,
                games::simled_count_invalid_message(&serial::simled_count_text(reply)),
            ));
            Some(SimLedQuery::rejected(std::mem::take(notices)))
        }
        serial::SimLedCount::Count(count) => {
            Some(SimLedQuery::ready(std::mem::take(notices), count))
        }
    }
}

fn record_count_attempt(notices: &mut Vec<InitNotice>) {
    notices.push(notice(Level::Info, games::MSG_SIMLED_COUNT_ATTEMPT));
    notices.push(notice(Level::Debug, games::MSG_SIMLED_COUNT_SEND));
}

fn captured_led_reply(port: &SerialPort) -> Vec<u8> {
    match port {
        SerialPort::Captured { led_reply, .. } => led_reply
            .clone()
            .unwrap_or_else(|| serial::SIMLED_COUNT_REPLY.to_vec()),
        SerialPort::Live { .. } | SerialPort::Closed => Vec::new(),
    }
}

enum LiveReply {
    Waiting,
    Failed,
    Bytes(Vec<u8>),
}

fn read_live_reply(port: &mut SerialPort) -> LiveReply {
    let SerialPort::Live { port, .. } = port else {
        return LiveReply::Failed;
    };
    read_count_bytes(port)
}

fn read_count_bytes(port: &mut RealSerial) -> LiveReply {
    let mut buf = vec![0; serial::SIMLED_READ_CAP];
    let first = match port.read_with_timeout(&mut buf, u64::from(serial::SIMLED_QUERY_WAIT_MS)) {
        Ok(read) => read,
        Err(_) => return LiveReply::Failed,
    };
    if first == 0 {
        return LiveReply::Waiting;
    }
    let mut got = buf[..first].to_vec();
    if got.len() >= serial::SIMLED_READ_CAP {
        return LiveReply::Bytes(got);
    }
    let mut more = vec![0; serial::SIMLED_READ_CAP - got.len()];
    let extra = match port.read_with_timeout(&mut more, serial::SIMLED_READ_TIMEOUT_MS) {
        Ok(read) => read,
        Err(_) => return LiveReply::Failed,
    };
    got.extend_from_slice(&more[..extra]);
    LiveReply::Bytes(got)
}

fn simled_entry(entry: &DeviceEntry) -> bool {
    if entry_kind(entry) != DeviceKind::Serial || simled_script(entry) {
        return false;
    }
    let kind = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    names::lookup(names::SERIAL_TYPES, kind) == Some(names::SUBTYPE_SIMLED)
}

fn simled_script(entry: &DeviceEntry) -> bool {
    let Some(path) = entry.get_str(keys::KEY_CONFIG) else {
        return false;
    };
    !path.is_empty() && !path.eq_ignore_ascii_case(keys::CONFIG_VALUE_NONE)
}

fn consider_simled(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let path = device_port(entry);
    let configured = entry.get_i64(keys::KEY_BAUD).unwrap_or(keys::BAUD_DEFAULT);
    let simled = SimLed {
        total: config_i32(
            entry
                .get_i64(keys::KEY_NUMLEDS)
                .unwrap_or(keys::NUMLEDS_DEFAULT),
        ),
        startled: config_i32(
            entry
                .get_i64(keys::KEY_STARTLED)
                .unwrap_or(keys::STARTLED_DEFAULT),
        ),
        endled: config_i32(
            entry
                .get_i64(keys::KEY_ENDLED)
                .unwrap_or(keys::ENDLED_DEFAULT),
        ),
    };
    let mut notices = serial_lookup_notices(
        &path,
        configured,
        names::SUBTYPE_SIMLED,
        games::MSG_SIMLED_INIT.to_string(),
    );
    let baud = serial_baud(configured);
    match open_serial(attempt, &path, baud) {
        OpenedSerial::Missing => serial_open_failed(notices),
        OpenedSerial::Ready(port) => {
            notices.extend(moza_ready_notices(baud));
            simled_ready(entry, id, notices, port, simled)
        }
    }
}

fn simled_ready(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: SerialPort,
    simled: SimLed,
) -> Considered {
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: port,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: Some(simled),
        custom_leds: None,
    }
}

#[derive(Clone, Copy)]
struct SimLed {
    total: i32,
    startled: i32,
    endled: i32,
}

fn simled_indexes(leds: &[Option<SimLed>]) -> Vec<usize> {
    leds.iter()
        .enumerate()
        .filter_map(|(index, led)| led.as_ref().map(|_| index))
        .collect()
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
    serial_wheel_hardware(entry, names::HARDWARE_MOZA_NEW)
}

fn moza_r5_entry(entry: &DeviceEntry) -> bool {
    serial_wheel_hardware(entry, names::HARDWARE_MOZA_R5)
}

fn serial_wheel_hardware(entry: &DeviceEntry, hardware: i32) -> bool {
    if entry_kind(entry) != DeviceKind::Serial {
        return false;
    }
    let kind = entry.get_str(keys::KEY_TYPE).unwrap_or("");
    if names::lookup(names::SERIAL_TYPES, kind) != Some(names::SUBTYPE_SERIAL_WHEEL) {
        return false;
    }
    let configured = entry.get_str(keys::KEY_SUBTYPE).unwrap_or("");
    names::lookup(names::HARDWARE, configured) == Some(hardware)
}

fn consider_moza_r5(
    entry: &DeviceEntry,
    slot: i32,
    id: i32,
    disable_audio: bool,
    attempt: &mut HidAttempt<'_>,
) -> Considered {
    if let Some(skip) = device_skip(entry, disable_audio) {
        return skipped(Vec::new(), skip, slot);
    }
    let path = device_port(entry);
    let configured = entry.get_i64(keys::KEY_BAUD).unwrap_or(keys::BAUD_DEFAULT);
    let mut notices = serial_lookup_notices(
        &path,
        configured,
        names::SUBTYPE_SERIAL_WHEEL,
        games::MSG_MOZA_R5_INIT.to_string(),
    );
    let baud = serial_baud(configured);
    match open_serial(attempt, &path, baud) {
        OpenedSerial::Missing => serial_open_failed(notices),
        OpenedSerial::Ready(port) => {
            notices.extend(moza_ready_notices(baud));
            moza_r5_ready(entry, id, notices, port)
        }
    }
}

fn moza_r5_ready(
    entry: &DeviceEntry,
    id: i32,
    notices: Vec<InitNotice>,
    port: SerialPort,
) -> Considered {
    Considered {
        setup_notices: Vec::new(),
        notices,
        prepared: Some(build_device(entry, id)),
        port: HidPort::Closed,
        tach: inactive_tach(),
        serial: port,
        wheel: None,
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: true,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
    }
}

fn open_moza_new(entry: &DeviceEntry, id: i32, attempt: &mut HidAttempt<'_>) -> Considered {
    let path = device_port(entry);
    let configured = entry.get_i64(keys::KEY_BAUD).unwrap_or(keys::BAUD_DEFAULT);
    let baud = serial::moza_r9_open_baud(configured);
    let mut notices = moza_lookup_notices(&path, configured);
    match open_serial(attempt, &path, baud) {
        OpenedSerial::Missing => serial_open_failed(notices),
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
    let mut notices = serial_preamble(
        names::SUBTYPE_SERIAL_WHEEL,
        games::MSG_MOZA_NEW_INIT.to_string(),
    );
    notices.extend(serial_open_attempt_notices(path, configured));
    notices
}

fn moza_ready_notices(baud: u32) -> Vec<InitNotice> {
    vec![
        notice(Level::Debug, games::MSG_SERIAL_PORT_OPENED),
        notice(Level::Debug, games::serial_baud_message(baud)),
        notice(Level::Debug, games::MSG_SERIAL_SETUP_OK),
    ]
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
        HidAttempt::Probe {
            serial, led_reply, ..
        } => {
            if serial(path) {
                return OpenedSerial::Ready(SerialPort::Captured {
                    path: path.to_string(),
                    frames: Vec::new(),
                    led_reply: led_reply.map(|bytes| bytes.to_vec()),
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
        sound: None,
        voice: None,
        g29: false,
        c5: false,
        c12: false,
        lua: None,
        gt: false,
        arduino_custom: false,
        moza_r5: false,
        csl: None,
        p1000: None,
        simnet: None,
        shift_lights: None,
        simwind: None,
        serial_haptic: None,
        simled: None,
        custom_leds: None,
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

fn telemetry_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn notice(level: Level, message: impl Into<String>) -> InitNotice {
    InitNotice {
        level,
        message: message.into(),
    }
}

enum FeatureWrite {
    Wrote(usize),
    Failed,
    Closed,
}

fn feature_port(port: &mut HidPort, report: &[u8], notices: &mut Vec<InitNotice>) -> bool {
    matches!(feature_write(port, report, notices), FeatureWrite::Wrote(_))
}

fn feature_write(port: &mut HidPort, report: &[u8], notices: &mut Vec<InitNotice>) -> FeatureWrite {
    match port {
        HidPort::Live(hid) => match hid.send_feature(report) {
            Ok(len) => FeatureWrite::Wrote(len),
            Err(_) => FeatureWrite::Failed,
        },
        HidPort::Captured(log) => {
            log.push(report.to_vec());
            FeatureWrite::Wrote(report.len())
        }
        HidPort::Closed => {
            notices.push(notice(Level::Debug, games::MSG_REVBURNER_NO_HANDLE));
            FeatureWrite::Closed
        }
    }
}

fn record_p1000(port: &mut HidPort, report: &[u8], notices: &mut Vec<InitNotice>) -> i32 {
    let wrote = feature_write(port, report, notices);
    if matches!(wrote, FeatureWrite::Closed) {
        return usb::P1000_REPORT_FAILED;
    }
    let nbytes = i32::try_from(usb::P1000_LEN).unwrap_or(i32::MAX);
    notices.push(notice(Level::Debug, games::p1000_sent_message(nbytes)));
    notices.push(notice(Level::Trace, games::p1000_bytes_message(report)));
    if matches!(wrote, FeatureWrite::Wrote(len) if len == usb::P1000_LEN) {
        return usb::P1000_REPORT_OK;
    }
    usb::P1000_REPORT_FAILED
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
    const HAPTIC_MOTOR_CHANNEL_ONE: i64 = 0;
    const HAPTIC_LOG_MOTOR_ONE: i32 = 1;
    const HAPTIC_MOTOR_FLAG: u8 = 1;
    const HAPTIC_MOTOR_ONE: usize = 0;
    const HAPTIC_EFFECT_ONE: usize = 1;
    const WHEEL_FRONT_LEFT: usize = 0;
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

    #[test]
    fn shiftlights_missing_port_is_not_scheduled() {
        const PORT: &str = "/dev/ttySHIFT-TEST";
        let config = shiftlights_config(PORT, keys::BAUD_DEFAULT, keys::NUMLIGHTS_DEFAULT);
        let mut seen = String::new();
        let missing = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| {
                seen = path.to_string();
                false
            },
        );
        assert_eq!(seen, PORT);
        assert!(missing.devices.is_empty());
        assert!(notice_has(&missing.notices, games::MSG_SHIFTLIGHTS_INIT));
        assert!(notice_has(&missing.notices, games::MSG_SERIAL_OPEN_ERROR));
        assert!(notice_has(
            &missing.notices,
            &games::serial_init_error_message(games::SERIAL_OPEN_ERROR)
        ));
        assert!(notice_has(
            &missing.notices,
            &games::could_not_initialize_message(CLASS_SERIAL)
        ));
        assert!(notice_absent(
            &missing.notices,
            games::MSG_SERIAL_PORT_OPENED
        ));
    }

    #[test]
    fn shiftlights_writes_the_lit_count_on_every_tick() {
        const PORT: &str = "/dev/ttySHIFT-TEST";
        const SAMPLE_RPM: u32 = 4_000;
        const SAMPLE_MAX_RPM: u32 = 8_000;
        const SAMPLE_LIT: u8 = 3;
        const REPEATED_TICKS: usize = 2;
        let config = shiftlights_config(PORT, keys::BAUD_DEFAULT, keys::NUMLIGHTS_DEFAULT);
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| path == PORT,
        );
        assert_eq!(loaded.devices.len(), 1);
        let baud = serial_baud(keys::BAUD_DEFAULT);
        assert!(notice_before(
            &loaded.notices,
            &games::serial_subtype_message(names::SUBTYPE_SHIFT_LIGHTS),
            games::MSG_SHIFTLIGHTS_INIT
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_SHIFTLIGHTS_INIT,
            games::MSG_SERIAL_START
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_SERIAL_START,
            games::MSG_SERIAL_PORT_OPENED
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_init_port_message(PORT, keys::BAUD_DEFAULT)
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_baud_message(baud)
        ));
        let expected = serial::shiftlights_byte(
            SAMPLE_RPM,
            SAMPLE_MAX_RPM,
            config_i32(keys::NUMLIGHTS_DEFAULT),
        );
        assert_eq!(expected, SAMPLE_LIT);
        let mut devices = loaded.devices;
        let frame = shift_frame(SAMPLE_RPM, SAMPLE_MAX_RPM);
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_has(
            &tick,
            &games::shiftlights_lit_message(i32::from(expected))
        ));
        assert!(notice_has(
            &tick,
            &games::arduino_copy_message(serial::SHIFT_PACKET_LEN)
        ));
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        let repeated = {
            let frames = devices.captured_serial(0).expect("captured");
            frames.len() == REPEATED_TICKS
                && frames.iter().all(|frame| frame.as_slice() == [expected])
        };
        assert!(repeated);
        let released = devices.release(PROBE_OPEN_NS);
        assert!(notice_has(&released, &games::serial_free_message(PORT)));
        assert!(notice_absent(
            &released,
            &games::shiftlights_lit_message(i32::from(expected))
        ));
        assert!(devices.captured_serial(0).is_none());
    }

    fn shift_frame(rpm: u32, maxrpm: u32) -> Telemetry {
        let mut frame = Telemetry::new();
        frame.set_rpms(rpm);
        frame.set_maxrpm(maxrpm);
        frame
    }

    #[test]
    fn simwind_missing_port_is_not_scheduled() {
        const PORT: &str = "/dev/ttyWIND-TEST";
        let config = simwind_config(PORT, keys::BAUD_DEFAULT);
        let mut seen = String::new();
        let missing = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| {
                seen = path.to_string();
                false
            },
        );
        assert_eq!(seen, PORT);
        assert!(missing.devices.is_empty());
        assert!(notice_has(
            &missing.notices,
            &games::simwind_init_message(keys::FANPOWER_DEFAULT)
        ));
        assert!(notice_has(&missing.notices, games::MSG_SERIAL_OPEN_ERROR));
        assert!(notice_absent(
            &missing.notices,
            games::MSG_SERIAL_PORT_OPENED
        ));
    }

    #[test]
    fn simwind_writes_speed_and_fan_on_every_tick() {
        const PORT: &str = "/dev/ttyWIND-TEST";
        const SAMPLE_KPH: u32 = 80;
        const REPEATED_TICKS: usize = 2;
        let config = simwind_config(PORT, keys::BAUD_DEFAULT);
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| path == PORT,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_before(
            &loaded.notices,
            &games::simwind_init_message(keys::FANPOWER_DEFAULT),
            games::MSG_SERIAL_START
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_SERIAL_START,
            games::MSG_SERIAL_PORT_OPENED
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_baud_message(serial_baud(keys::BAUD_DEFAULT))
        ));
        let expected = serial::simwind_report(SAMPLE_KPH, keys::FANPOWER_DEFAULT);
        let mut devices = loaded.devices;
        let mut frame = Telemetry::new();
        frame.set_velocity(SAMPLE_KPH);
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_has(
            &tick,
            &games::simwind_speed_message(i32::from(expected[serial::SIMWIND_BYTE_SPEED]))
        ));
        assert!(notice_has(
            &tick,
            &games::simwind_fan_message(
                i32::from(expected[serial::SIMWIND_BYTE_FAN]),
                keys::FANPOWER_DEFAULT
            )
        ));
        assert!(notice_has(
            &tick,
            &games::arduino_copy_message(serial::SIMWIND_PACKET_LEN)
        ));
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        let repeated = {
            let frames = devices.captured_serial(0).expect("captured");
            frames.len() == REPEATED_TICKS
                && frames.iter().all(|frame| frame.as_slice() == expected)
        };
        assert!(repeated);
        let released = devices.release(PROBE_OPEN_NS);
        assert!(notice_has(&released, &games::serial_free_message(PORT)));
        assert!(notice_absent(
            &released,
            &games::simwind_speed_message(i32::from(expected[serial::SIMWIND_BYTE_SPEED]))
        ));
        assert!(devices.captured_serial(0).is_none());
    }

    #[test]
    fn serial_haptic_missing_port_is_not_scheduled() {
        const PORT: &str = "/dev/ttyHAPTIC-TEST";
        let config = serial_haptic_config(PORT, Some(HAPTIC_MOTOR_CHANNEL_ONE));
        let mut seen = String::new();
        let missing = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| {
                seen = path.to_string();
                false
            },
        );
        assert_eq!(seen, PORT);
        assert!(missing.devices.is_empty());
        assert!(notice_has(&missing.notices, games::MSG_SERIAL_HAPTIC_INIT));
        assert!(notice_has(
            &missing.notices,
            &games::haptic_effect_message(games::VIBRATION_SLIP)
        ));
        assert!(notice_has(&missing.notices, games::MSG_SERIAL_OPEN_ERROR));
        assert!(notice_has(
            &missing.notices,
            &games::serial_init_error_message(games::SERIAL_OPEN_ERROR)
        ));
        assert!(notice_absent(
            &missing.notices,
            games::MSG_SERIAL_PORT_OPENED
        ));
    }

    #[test]
    fn serial_haptic_without_sim_support_does_not_open() {
        const PORT: &str = "/dev/ttyHAPTIC-TEST";
        let config = serial_haptic_config(PORT, Some(HAPTIC_MOTOR_CHANNEL_ONE));
        let mut seen = false;
        let missing = open_profile_with(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            false,
            None,
            &mut HidAttempt::Probe {
                hid: &mut |_vendor, _product| false,
                serial: &mut |_path| {
                    seen = true;
                    true
                },
                sysfs: None,
                now_ns: PROBE_OPEN_NS,
                led_reply: None,
            },
        );
        assert!(!seen);
        assert!(missing.devices.is_empty());
        assert!(notice_has(&missing.notices, games::MSG_SERIAL_HAPTIC_INIT));
        assert!(notice_has(
            &missing.notices,
            &games::serial_init_error_message(games::UNSUPPORTED_SIM_FEATURE)
        ));
        assert!(notice_has(
            &missing.notices,
            &games::could_not_initialize_message(CLASS_SERIAL)
        ));
        assert!(notice_absent(&missing.notices, games::MSG_SERIAL_START));
        assert!(notice_absent(
            &missing.notices,
            games::MSG_SERIAL_PORT_OPENED
        ));
        assert!(notice_absent(&missing.notices, games::MSG_USB_NO_HAPTICS));
        assert!(notice_absent(
            &missing.notices,
            &games::haptic_effect_message(games::VIBRATION_SLIP)
        ));
    }

    #[test]
    fn serial_haptic_writes_slip_and_keeps_the_effect_byte() {
        const PORT: &str = "/dev/ttyHAPTIC-TEST";
        const SAMPLE_SPEED: u32 = 80;
        const SAMPLE_Y: f64 = 1.0;
        const SAMPLE_GAS: f64 = 0.2;
        const SAMPLE_SLIP: f64 = -0.4;
        const STRONGER_SLIP: f64 = -0.8;
        const SAMPLE_PLAY: f64 = 0.4;
        const SLIP_SPEED: u8 = 102;
        const STRONGER_SPEED: u8 = 204;
        let config = serial_haptic_config(PORT, Some(HAPTIC_MOTOR_CHANNEL_ONE));
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| path == PORT,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_before(
            &loaded.notices,
            &games::serial_subtype_message(names::SUBTYPE_SERIAL_HAPTIC),
            games::MSG_SERIAL_HAPTIC_INIT
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_SERIAL_HAPTIC_INIT,
            &games::haptic_effect_message(games::VIBRATION_SLIP)
        ));
        assert!(notice_before(
            &loaded.notices,
            &games::haptic_effect_message(games::VIBRATION_SLIP),
            games::MSG_SERIAL_START
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_SERIAL_START,
            games::MSG_SERIAL_PORT_OPENED
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::haptic_motor_message(HAPTIC_MOTOR_CHANNEL_ONE)
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_baud_message(serial_baud(keys::BAUD_DEFAULT))
        ));
        let mut devices = loaded.devices;
        let frame = csl_frame(
            SAMPLE_SPEED,
            SAMPLE_Y,
            SAMPLE_GAS,
            WHEEL_FRONT_LEFT,
            SAMPLE_SLIP,
        );
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_has(&tick, games::MSG_SERIAL_HAPTIC_UPDATING));
        assert!(notice_has(
            &tick,
            &games::serial_haptic_channel_message(
                names::EFFECT_TYRE_SLIP,
                i32::from(SLIP_SPEED),
                HAPTIC_LOG_MOTOR_ONE,
                SAMPLE_PLAY,
                keys::AMPFACTOR_DEFAULT,
            )
        ));
        assert!(notice_has(
            &tick,
            &games::arduino_copy_message(serial::HAPTIC_PACKET_LEN)
        ));
        let held = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_absent(
            &held,
            &games::serial_haptic_channel_message(
                names::EFFECT_TYRE_SLIP,
                i32::from(SLIP_SPEED),
                HAPTIC_LOG_MOTOR_ONE,
                SAMPLE_PLAY,
                keys::AMPFACTOR_DEFAULT,
            )
        ));
        let stronger = csl_frame(
            SAMPLE_SPEED,
            SAMPLE_Y,
            SAMPLE_GAS,
            WHEEL_FRONT_LEFT,
            STRONGER_SLIP,
        );
        let _ = devices.tick(0, &stronger, PROBE_OPEN_NS);
        let idle = csl_frame(SAMPLE_SPEED, SAMPLE_Y, 0.0, WHEEL_FRONT_LEFT, SAMPLE_SLIP);
        let idle_tick = devices.tick(0, &idle, PROBE_OPEN_NS);
        assert!(notice_has(
            &idle_tick,
            &games::serial_haptic_channel_message(
                names::EFFECT_TYRE_SLIP,
                0,
                HAPTIC_LOG_MOTOR_ONE,
                0.0,
                keys::AMPFACTOR_DEFAULT,
            )
        ));
        let frames = devices.captured_serial(0).expect("captured");
        assert_eq!(
            frames,
            vec![
                haptic_slip_packet(SLIP_SPEED, true),
                haptic_slip_packet(SLIP_SPEED, false),
                haptic_slip_packet(STRONGER_SPEED, true),
                haptic_slip_packet(0, true),
            ]
        );
        let released = devices.release(PROBE_OPEN_NS);
        assert!(notice_before(
            &released,
            games::MSG_SERIAL_HAPTIC_ZERO,
            &games::serial_free_message(PORT)
        ));
        assert!(notice_absent(&released, games::MSG_SERIAL_HAPTIC_UPDATING));
        assert!(devices.captured_serial(0).is_none());
    }

    #[test]
    fn serial_haptic_unset_motor_writes_zeros() {
        const PORT: &str = "/dev/ttyHAPTIC-TEST";
        const SAMPLE_SPEED: u32 = 80;
        const SAMPLE_Y: f64 = 1.0;
        const SAMPLE_GAS: f64 = 0.2;
        const SAMPLE_SLIP: f64 = -0.4;
        let config = serial_haptic_config(PORT, None);
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| path == PORT,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_has(
            &loaded.notices,
            &games::haptic_motor_message(HAPTIC_MOTOR_DEFAULT)
        ));
        let mut devices = loaded.devices;
        let frame = csl_frame(
            SAMPLE_SPEED,
            SAMPLE_Y,
            SAMPLE_GAS,
            WHEEL_FRONT_LEFT,
            SAMPLE_SLIP,
        );
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_absent(
            &tick,
            &games::serial_haptic_channel_message(
                names::EFFECT_TYRE_SLIP,
                0,
                HAPTIC_LOG_MOTOR_ONE,
                SAMPLE_SLIP.abs(),
                keys::AMPFACTOR_DEFAULT,
            )
        ));
        let frames = devices.captured_serial(0).expect("captured");
        assert_eq!(frames, vec![vec![0; serial::HAPTIC_PACKET_LEN as usize]]);
    }

    fn serial_haptic_config(path: &str, motors: Option<i64>) -> CargopitConfig {
        const HAPTIC_FPS: i64 = 60;
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_DEVICE, keys::CLASS_SERIAL);
        let kind =
            names::name_for(names::SERIAL_TYPES, names::SUBTYPE_SERIAL_HAPTIC).expect("haptic");
        device.set_str(keys::KEY_TYPE, kind);
        device.set_str(keys::KEY_DEVPATH, path);
        device.set_int(keys::KEY_BAUD, keys::BAUD_DEFAULT);
        let effect = names::name_for(names::EFFECTS, names::EFFECT_TYRE_SLIP).expect("slip");
        device.set_str(keys::KEY_EFFECT, effect);
        if let Some(motors) = motors {
            device.set_int(keys::KEY_MOTORS, motors);
        }
        device.set_bool(keys::KEY_ENABLED, true);
        device.set_int(keys::KEY_FPS, HAPTIC_FPS);
        CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![device],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        }
    }

    fn haptic_slip_packet(speed: u8, flagged: bool) -> Vec<u8> {
        let mut packet = vec![0; serial::HAPTIC_PACKET_LEN as usize];
        packet[HAPTIC_EFFECT_ONE] = speed;
        if flagged {
            packet[HAPTIC_MOTOR_ONE] = HAPTIC_MOTOR_FLAG;
        }
        packet
    }

    #[test]
    fn simled_missing_port_is_not_scheduled() {
        const PORT: &str = "/dev/ttySIMLED-TEST";
        let config = simled_config(PORT, None);
        let mut seen = String::new();
        let missing = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| {
                seen = path.to_string();
                false
            },
        );
        assert_eq!(seen, PORT);
        assert!(missing.devices.is_empty());
        assert!(notice_has(&missing.notices, games::MSG_SIMLED_INIT));
        assert!(notice_has(&missing.notices, games::MSG_SERIAL_OPEN_ERROR));
        assert!(notice_absent(
            &missing.notices,
            games::MSG_SERIAL_PORT_OPENED
        ));
    }

    #[test]
    fn simled_writes_the_same_pattern_on_every_tick() {
        const PORT: &str = "/dev/ttySIMLED-TEST";
        const TOTAL: i64 = 8;
        const SAMPLE_RPM: u32 = 4_000;
        const SAMPLE_MAX: u32 = 8_000;
        const SAMPLE_LIT: i32 = 4;
        const REPEATED_TICKS: usize = 2;
        let config = simled_config(PORT, Some(SimLedSpanConfig::full(TOTAL)));
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| path == PORT,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_before(
            &loaded.notices,
            &games::serial_subtype_message(names::SUBTYPE_SIMLED),
            games::MSG_SIMLED_INIT
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_SIMLED_INIT,
            games::MSG_SERIAL_START
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_baud_message(serial_baud(keys::BAUD_DEFAULT))
        ));
        let expected = serial::simled_report(
            SAMPLE_RPM,
            SAMPLE_MAX,
            config_i32(TOTAL),
            config_i32(keys::STARTLED_DEFAULT),
            config_i32(TOTAL),
        )
        .expect("span");
        assert_eq!(expected.lit, SAMPLE_LIT);
        let mut devices = loaded.devices;
        let frame = shift_frame(SAMPLE_RPM, SAMPLE_MAX);
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_has(
            &tick,
            &games::shiftlights_lit_message(expected.lit)
        ));
        let copied = i32::try_from(expected.bytes.len()).unwrap_or(i32::MAX);
        assert!(notice_absent(&tick, &games::arduino_copy_message(copied)));
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        let repeated = {
            let frames = devices.captured_serial(0).expect("captured");
            frames.len() == REPEATED_TICKS && frames.iter().all(|frame| frame == &expected.bytes)
        };
        assert!(repeated);
        let released = devices.release(PROBE_OPEN_NS);
        assert!(notice_has(&released, &games::serial_free_message(PORT)));
        assert!(notice_absent(
            &released,
            &games::shiftlights_lit_message(expected.lit)
        ));
        assert!(devices.captured_serial(0).is_none());
    }

    #[test]
    fn simled_default_span_lights_the_first_led() {
        const PORT: &str = "/dev/ttySIMLED-TEST";
        const SAMPLE_RPM: u32 = 8_000;
        let config = simled_config(PORT, None);
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| path == PORT,
        );
        assert_eq!(loaded.devices.len(), 1);
        let expected = serial::simled_report(
            SAMPLE_RPM,
            SAMPLE_RPM,
            config_i32(keys::NUMLEDS_DEFAULT),
            config_i32(keys::STARTLED_DEFAULT),
            config_i32(keys::ENDLED_DEFAULT),
        )
        .expect("default span");
        let mut devices = loaded.devices;
        let frame = shift_frame(SAMPLE_RPM, SAMPLE_RPM);
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_has(
            &tick,
            &games::shiftlights_lit_message(expected.lit)
        ));
        assert_eq!(
            devices.captured_serial(0).map(|frames| frames.to_vec()),
            Some(vec![expected.bytes])
        );
    }

    #[test]
    fn custom_simled_missing_port_is_not_scheduled() {
        const PORT: &str = "/dev/ttySIMLED-TEST";
        let config = custom_simled_config(PORT, CUSTOM_SCRIPT_NAME);
        let mut seen = String::new();
        let missing = open_simled(
            &config,
            |path| {
                seen = path.to_string();
                false
            },
            None,
        );
        assert_eq!(seen, PORT);
        assert!(missing.devices.is_empty());
        assert!(notice_has(&missing.notices, games::MSG_SIMLED_CUSTOM_INIT));
        assert!(notice_has(&missing.notices, games::MSG_SERIAL_OPEN_ERROR));
        assert!(notice_absent(
            &missing.notices,
            games::MSG_SERIAL_PORT_OPENED
        ));
        assert!(notice_absent(&missing.notices, games::MSG_SIMLED_INIT));
    }

    #[test]
    fn custom_simled_writes_the_script_packet_every_tick() {
        const PORT: &str = "/dev/ttySIMLED-TEST";
        const REPEATED_TICKS: usize = 2;
        const CUSTOM_REPLY_LEDS: i32 = 8;
        const GREEN_CHANNEL: usize = 1;
        const LED_ON: u8 = 0xff;
        let _script = custom_script(CUSTOM_SCRIPT_NAME, CUSTOM_SIMLED_LUA);
        let config = custom_simled_config(PORT, CUSTOM_SCRIPT_NAME);
        let loaded = open_simled(&config, |path| path == PORT, None);
        assert_eq!(loaded.devices.len(), 1);
        assert!(custom_simled_notice_order(
            &loaded.notices,
            CUSTOM_REPLY_LEDS
        ));
        assert!(notice_absent(&loaded.notices, games::MSG_SIMLED_INIT));
        let query = serial::simled_count_query();
        assert_eq!(
            loaded
                .devices
                .captured_serial(0)
                .map(|frames| frames.to_vec()),
            Some(vec![query.clone()])
        );
        let expected = custom_simled_packet(CUSTOM_REPLY_LEDS);
        assert_eq!(expected.leds[GREEN_CHANNEL], LED_ON);
        let copied = i32::try_from(expected.packet.len()).unwrap_or(i32::MAX);
        let mut devices = loaded.devices;
        let frame = shift_frame(0, 0);
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_has(&tick, &games::arduino_copy_message(copied)));
        assert!(notice_has(
            &tick,
            &games::simled_custom_wrote_message(copied)
        ));
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        let frames = devices.captured_serial(0).expect("captured").to_vec();
        assert_eq!(frames.len(), 1 + REPEATED_TICKS);
        assert_eq!(frames[0], query);
        assert!(frames[1..].iter().all(|frame| frame == &expected.packet));
        let released = devices.release(PROBE_OPEN_NS);
        assert!(notice_has(&released, &games::serial_free_message(PORT)));
        assert!(notice_absent(
            &released,
            &games::arduino_copy_message(copied)
        ));
        assert!(notice_absent(
            &released,
            &games::simled_custom_wrote_message(copied)
        ));
        assert!(devices.captured_serial(0).is_none());
    }

    #[test]
    fn custom_simled_rejects_an_invalid_count() {
        const PORT: &str = "/dev/ttySIMLED-TEST";
        const INVALID_LED_REPLY: &[u8] = b"nope";
        let _script = custom_script(CUSTOM_INVALID_SCRIPT_NAME, CUSTOM_SIMLED_LUA);
        let config = custom_simled_config(PORT, CUSTOM_INVALID_SCRIPT_NAME);
        let loaded = open_simled(&config, |_path| true, Some(INVALID_LED_REPLY));
        assert!(loaded.devices.is_empty());
        assert!(notice_has(
            &loaded.notices,
            &games::simled_count_invalid_message("nope")
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::simled_count_message(SIMLED_COUNT_UNSET)
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_init_error_message(games::SERIAL_OPEN_ERROR)
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_free_message(PORT)
        ));
        assert!(notice_absent(&loaded.notices, games::MSG_SIMLED_LUA_OK));
    }

    #[test]
    fn custom_simled_with_no_count_still_loads_lua() {
        const PORT: &str = "/dev/ttySIMLED-TEST";
        const EMPTY_LED_REPLY: &[u8] = b"";
        const REPEATED_TICKS: usize = 2;
        let _script = custom_script(CUSTOM_EMPTY_SCRIPT_NAME, CUSTOM_SIMLED_LUA);
        let config = custom_simled_config(PORT, CUSTOM_EMPTY_SCRIPT_NAME);
        let loaded = open_simled(&config, |path| path == PORT, Some(EMPTY_LED_REPLY));
        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(
            notice_count(&loaded.notices, games::MSG_SIMLED_COUNT_ATTEMPT),
            serial::SIMLED_QUERY_ATTEMPTS
        );
        assert!(notice_has(
            &loaded.notices,
            &games::simled_count_message(SIMLED_COUNT_UNSET)
        ));
        assert!(notice_has(&loaded.notices, games::MSG_SIMLED_LUA_OK));
        let query = serial::simled_count_query();
        let mut devices = loaded.devices;
        let frame = shift_frame(0, 0);
        for _tick in 0..REPEATED_TICKS {
            let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
            assert!(tick.is_empty());
        }
        let frames = devices.captured_serial(0).expect("captured");
        assert_eq!(frames.len(), serial::SIMLED_QUERY_ATTEMPTS);
        assert!(frames.iter().all(|frame| frame == &query));
    }

    #[test]
    fn custom_simled_missing_script_is_not_scheduled() {
        const PORT: &str = "/dev/ttySIMLED-TEST";
        let path = std::env::temp_dir().join(CUSTOM_MISSING_SCRIPT_NAME);
        let _ = std::fs::remove_file(&path);
        let config = custom_simled_config(PORT, CUSTOM_MISSING_SCRIPT_NAME);
        let loaded = open_simled(&config, |_path| true, None);
        assert!(loaded.devices.is_empty());
        assert!(notice_has(
            &loaded.notices,
            &games::simled_count_message(CUSTOM_REPLY_LEDS)
        ));
        assert!(notice_has(&loaded.notices, games::MSG_SIMLED_LUA_INIT));
        assert!(notice_absent(&loaded.notices, games::MSG_SIMLED_LUA_OK));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_init_error_message(games::SERIAL_OPEN_ERROR)
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_free_message(PORT)
        ));
    }

    const ARDUINO_CUSTOM_LUA: &str = "Message = \"parity\"\n";
    const ARDUINO_CUSTOM_QUIET: &str = "-- quiet\n";
    const ARDUINO_CUSTOM_ERROR: &str = "Message = \"parity\"\nerror(\"boom\")\n";
    const ARDUINO_CUSTOM_MESSAGE: &str = "parity";
    const ARDUINO_SCRIPT_NAME: &str = "cargopit-arduino-custom.lua";
    const ARDUINO_QUIET_SCRIPT_NAME: &str = "cargopit-arduino-custom-quiet.lua";
    const ARDUINO_ERROR_SCRIPT_NAME: &str = "cargopit-arduino-custom-error.lua";
    const ARDUINO_MISSING_SCRIPT_NAME: &str = "cargopit-arduino-custom-missing.lua";

    #[test]
    fn arduino_custom_missing_port_is_not_scheduled() {
        const PORT: &str = "/dev/ttyCUSTOM-TEST";
        let _script = custom_script(ARDUINO_SCRIPT_NAME, ARDUINO_CUSTOM_LUA);
        let config = arduino_custom_config(PORT, Some(ARDUINO_SCRIPT_NAME));
        let mut seen = String::new();
        let missing = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| {
                seen = path.to_string();
                false
            },
        );
        assert_eq!(seen, PORT);
        assert!(missing.devices.is_empty());
        assert!(notice_has(&missing.notices, games::MSG_ARDUINO_CUSTOM_INIT));
        assert!(notice_has(&missing.notices, games::MSG_SERIAL_OPEN_ERROR));
        assert!(notice_absent(
            &missing.notices,
            games::MSG_SERIAL_PORT_OPENED
        ));
        assert!(notice_absent(&missing.notices, games::MSG_SIMLED_LUA_INIT));
    }

    #[test]
    fn arduino_custom_writes_the_script_message_every_tick() {
        const PORT: &str = "/dev/ttyCUSTOM-TEST";
        const REPEATED_TICKS: usize = 2;
        let _script = custom_script(ARDUINO_SCRIPT_NAME, ARDUINO_CUSTOM_LUA);
        let config = arduino_custom_config(PORT, Some(ARDUINO_SCRIPT_NAME));
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| path == PORT,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert!(arduino_custom_notice_order(&loaded.notices));
        assert!(notice_absent(
            &loaded.notices,
            games::MSG_SIMLED_COUNT_ATTEMPT
        ));
        assert_eq!(
            loaded.devices.captured_serial(0).map(|frames| frames.len()),
            Some(0)
        );
        let copied = i32::try_from(ARDUINO_CUSTOM_MESSAGE.len()).unwrap_or(i32::MAX);
        let mut devices = loaded.devices;
        let frame = shift_frame(0, 0);
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_has(&tick, &games::arduino_copy_message(copied)));
        assert!(notice_has(
            &tick,
            &games::arduino_custom_wrote_message(ARDUINO_CUSTOM_MESSAGE, copied)
        ));
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        let frames = devices.captured_serial(0).expect("captured").to_vec();
        assert_eq!(frames.len(), REPEATED_TICKS);
        assert!(frames
            .iter()
            .all(|frame| frame == ARDUINO_CUSTOM_MESSAGE.as_bytes()));
        let released = devices.release(PROBE_OPEN_NS);
        assert!(notice_has(&released, &games::serial_free_message(PORT)));
        assert!(notice_absent(
            &released,
            &games::arduino_copy_message(copied)
        ));
        assert!(notice_absent(
            &released,
            &games::arduino_custom_wrote_message(ARDUINO_CUSTOM_MESSAGE, copied)
        ));
        assert!(notice_absent(&released, games::MSG_C12_LUA_CLOSE));
        assert!(devices.captured_serial(0).is_none());
    }

    #[test]
    fn arduino_custom_without_a_message_writes_nothing() {
        const PORT: &str = "/dev/ttyCUSTOM-TEST";
        let _script = custom_script(ARDUINO_QUIET_SCRIPT_NAME, ARDUINO_CUSTOM_QUIET);
        let config = arduino_custom_config(PORT, Some(ARDUINO_QUIET_SCRIPT_NAME));
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| path == PORT,
        );
        assert_eq!(loaded.devices.len(), 1);
        let mut devices = loaded.devices;
        let tick = devices.tick(0, &shift_frame(0, 0), PROBE_OPEN_NS);
        assert!(tick.is_empty());
        assert_eq!(
            devices.captured_serial(0).map(|frames| frames.len()),
            Some(0)
        );
    }

    #[test]
    fn arduino_custom_keeps_a_message_set_before_a_script_error() {
        const PORT: &str = "/dev/ttyCUSTOM-TEST";
        let _script = custom_script(ARDUINO_ERROR_SCRIPT_NAME, ARDUINO_CUSTOM_ERROR);
        let config = arduino_custom_config(PORT, Some(ARDUINO_ERROR_SCRIPT_NAME));
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| path == PORT,
        );
        assert_eq!(loaded.devices.len(), 1);
        let copied = i32::try_from(ARDUINO_CUSTOM_MESSAGE.len()).unwrap_or(i32::MAX);
        let mut devices = loaded.devices;
        let tick = devices.tick(0, &shift_frame(0, 0), PROBE_OPEN_NS);
        assert!(notice_has(
            &tick,
            &games::arduino_custom_wrote_message(ARDUINO_CUSTOM_MESSAGE, copied)
        ));
        assert_eq!(
            devices.captured_serial(0).map(|frames| frames.to_vec()),
            Some(vec![ARDUINO_CUSTOM_MESSAGE.as_bytes().to_vec()])
        );
    }

    #[test]
    fn arduino_custom_missing_script_is_not_scheduled() {
        const PORT: &str = "/dev/ttyCUSTOM-TEST";
        let path = std::env::temp_dir().join(ARDUINO_MISSING_SCRIPT_NAME);
        let _ = std::fs::remove_file(&path);
        let config = arduino_custom_config(PORT, Some(ARDUINO_MISSING_SCRIPT_NAME));
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |_path| true,
        );
        assert!(loaded.devices.is_empty());
        assert!(notice_has(&loaded.notices, games::MSG_SIMLED_LUA_INIT));
        assert!(notice_absent(&loaded.notices, games::MSG_SIMLED_LUA_OK));
        assert!(notice_absent(
            &loaded.notices,
            games::MSG_SIMLED_COUNT_ATTEMPT
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_init_error_message(games::SERIAL_OPEN_ERROR)
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::serial_free_message(PORT)
        ));
    }

    #[test]
    fn arduino_custom_without_a_script_does_not_open() {
        const PORT: &str = "/dev/ttyCUSTOM-TEST";
        let mut seen = false;
        let missing = open_profile_ports(
            &arduino_custom_config(PORT, None),
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |_path| {
                seen = true;
                true
            },
        );
        assert!(!seen);
        assert!(missing.devices.is_empty());
        assert!(notice_has(&missing.notices, games::MSG_ARDUINO_CUSTOM_INIT));
        assert!(notice_has(&missing.notices, games::MSG_SERIAL_START));
        assert!(notice_absent(
            &missing.notices,
            games::MSG_SERIAL_PORT_OPENED
        ));
        assert!(notice_absent(&missing.notices, games::MSG_SERIAL_OPENING));
        assert!(notice_has(
            &missing.notices,
            &games::serial_init_error_message(games::SERIAL_OPEN_ERROR)
        ));
        let mut seen_none = false;
        let named_none = open_profile_ports(
            &arduino_custom_config(PORT, Some(keys::CONFIG_VALUE_NONE)),
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |_path| {
                seen_none = true;
                true
            },
        );
        assert!(!seen_none);
        assert!(named_none.devices.is_empty());
        assert!(notice_has(
            &named_none.notices,
            games::MSG_ARDUINO_CUSTOM_INIT
        ));
    }

    fn arduino_custom_config(port: &str, script: Option<&str>) -> CargopitConfig {
        const CUSTOM_FPS: i64 = 60;
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_DEVICE, keys::CLASS_SERIAL);
        let kind = names::name_for(names::SERIAL_TYPES, names::SUBTYPE_ARDUINO_CUSTOM)
            .expect("arduino custom");
        device.set_str(keys::KEY_TYPE, kind);
        device.set_str(keys::KEY_DEVPATH, port);
        device.set_int(keys::KEY_BAUD, keys::BAUD_DEFAULT);
        if let Some(name) = script {
            let path = if name.eq_ignore_ascii_case(keys::CONFIG_VALUE_NONE) {
                name.to_string()
            } else {
                std::env::temp_dir().join(name).display().to_string()
            };
            device.set_str(keys::KEY_CONFIG, path);
        }
        device.set_bool(keys::KEY_ENABLED, true);
        device.set_int(keys::KEY_FPS, CUSTOM_FPS);
        CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![device],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        }
    }

    fn arduino_custom_notice_order(notices: &[InitNotice]) -> bool {
        let messages = [
            games::serial_subtype_message(names::SUBTYPE_ARDUINO_CUSTOM),
            games::MSG_ARDUINO_CUSTOM_INIT.to_string(),
            games::MSG_SERIAL_START.to_string(),
            games::MSG_SERIAL_PORT_OPENED.to_string(),
            games::MSG_SIMLED_LUA_INIT.to_string(),
            games::MSG_SIMLED_LUA_OK.to_string(),
        ];
        notice_sequence(notices, &messages)
    }

    struct SimLedSpanConfig {
        total: i64,
        startled: i64,
        endled: i64,
    }

    impl SimLedSpanConfig {
        fn full(total: i64) -> Self {
            Self {
                total,
                startled: keys::STARTLED_DEFAULT,
                endled: total,
            }
        }
    }

    fn simled_config(path: &str, span: Option<SimLedSpanConfig>) -> CargopitConfig {
        const SIMLED_FPS: i64 = 60;
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_DEVICE, keys::CLASS_SERIAL);
        let kind = names::name_for(names::SERIAL_TYPES, names::SUBTYPE_SIMLED).expect("simled");
        device.set_str(keys::KEY_TYPE, kind);
        device.set_str(keys::KEY_DEVPATH, path);
        device.set_int(keys::KEY_BAUD, keys::BAUD_DEFAULT);
        if let Some(span) = span {
            device.set_int(keys::KEY_NUMLEDS, span.total);
            device.set_int(keys::KEY_STARTLED, span.startled);
            device.set_int(keys::KEY_ENDLED, span.endled);
        }
        device.set_bool(keys::KEY_ENABLED, true);
        device.set_int(keys::KEY_FPS, SIMLED_FPS);
        CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![device],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        }
    }

    const CUSTOM_SIMLED_LUA: &str = "led_clear_all()\nset_led_to_color(1, GREEN)\n";
    const CUSTOM_SCRIPT_NAME: &str = "cargopit-simled-custom.lua";
    const CUSTOM_EMPTY_SCRIPT_NAME: &str = "cargopit-simled-custom-empty.lua";
    const CUSTOM_INVALID_SCRIPT_NAME: &str = "cargopit-simled-custom-invalid.lua";
    const CUSTOM_MISSING_SCRIPT_NAME: &str = "cargopit-simled-missing.lua";
    const CUSTOM_REPLY_LEDS: i32 = 8;

    struct CustomSimLedPaint {
        leds: Vec<u8>,
        packet: Vec<u8>,
    }

    fn custom_script(name: &str, source: &str) -> TempScript {
        TempScript::write(&std::env::temp_dir().join(name), source)
    }

    fn custom_simled_config(port: &str, name: &str) -> CargopitConfig {
        let mut config = simled_config(port, None);
        let path = std::env::temp_dir().join(name);
        config.profiles[0].devices[0].set_str(keys::KEY_CONFIG, path.display().to_string());
        config
    }

    fn custom_simled_packet(total: i32) -> CustomSimLedPaint {
        let mut painter = LuaHost::load(CUSTOM_SIMLED_LUA, LuaLedMode::Serial).expect("script");
        let mut sample = Telemetry::new();
        let tick = painter
            .call(&mut sample, i64::from(total), &SystemClock::new())
            .expect("paint");
        let count = usize::try_from(total).unwrap_or(0);
        CustomSimLedPaint {
            leds: tick.leds.clone(),
            packet: serial::simled_packet(count, &tick.leds),
        }
    }

    fn open_simled(
        config: &CargopitConfig,
        mut serial: impl FnMut(&str) -> bool,
        led_reply: Option<&[u8]>,
    ) -> ProfileLoad {
        open_profile_with(
            config,
            0,
            false,
            PLAY_USES_PULSES,
            ASSUME_SIM_SUPPORTS_HAPTICS,
            None,
            &mut HidAttempt::Probe {
                hid: &mut |_vendor, _product| false,
                serial: &mut serial,
                sysfs: None,
                now_ns: PROBE_OPEN_NS,
                led_reply,
            },
        )
    }

    fn custom_simled_notice_order(notices: &[InitNotice], count: i32) -> bool {
        let messages = [
            games::serial_subtype_message(names::SUBTYPE_SIMLED),
            games::MSG_SIMLED_CUSTOM_INIT.to_string(),
            games::MSG_SERIAL_START.to_string(),
            games::MSG_SERIAL_PORT_OPENED.to_string(),
            games::MSG_SIMLED_COUNT_ATTEMPT.to_string(),
            games::simled_count_message(count),
            games::MSG_SIMLED_LUA_INIT.to_string(),
            games::MSG_SIMLED_LUA_OK.to_string(),
        ];
        notice_sequence(notices, &messages)
    }

    fn notice_sequence(notices: &[InitNotice], messages: &[String]) -> bool {
        let mut cursor = 0;
        for message in messages {
            let Some(found) = notices[cursor..]
                .iter()
                .position(|notice| notice.message == *message)
            else {
                return false;
            };
            cursor = cursor.saturating_add(found).saturating_add(1);
        }
        true
    }

    fn notice_count(notices: &[InitNotice], message: &str) -> usize {
        notices
            .iter()
            .filter(|notice| notice.message == message)
            .count()
    }

    fn simwind_config(path: &str, baud: i64) -> CargopitConfig {
        const WIND_FPS: i64 = 60;
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_DEVICE, keys::CLASS_SERIAL);
        let kind = names::name_for(names::SERIAL_TYPES, names::SUBTYPE_SIM_WIND).expect("sim wind");
        device.set_str(keys::KEY_TYPE, kind);
        device.set_str(keys::KEY_DEVPATH, path);
        device.set_int(keys::KEY_BAUD, baud);
        device.set_bool(keys::KEY_ENABLED, true);
        device.set_int(keys::KEY_FPS, WIND_FPS);
        CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![device],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        }
    }

    fn shiftlights_config(path: &str, baud: i64, lights: i64) -> CargopitConfig {
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_DEVICE, keys::CLASS_SERIAL);
        let kind = names::name_for(names::SERIAL_TYPES, names::SUBTYPE_SHIFT_LIGHTS)
            .expect("shift lights");
        device.set_str(keys::KEY_TYPE, kind);
        device.set_str(keys::KEY_DEVPATH, path);
        device.set_int(keys::KEY_BAUD, baud);
        const SHIFT_FPS: i64 = 60;
        device.set_int(keys::KEY_NUMLIGHTS, lights);
        device.set_bool(keys::KEY_ENABLED, true);
        device.set_int(keys::KEY_FPS, SHIFT_FPS);
        CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![device],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        }
    }

    const MOZA_WHEEL_FPS: i64 = 60;

    fn moza_config(path: &str, baud: i64) -> CargopitConfig {
        moza_wheel_config(path, baud, keys::SUBTYPE_MOZA_R9)
    }

    fn moza_wheel_config(path: &str, baud: i64, hardware: &str) -> CargopitConfig {
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_DEVICE, keys::CLASS_SERIAL);
        device.set_str(keys::KEY_TYPE, keys::TYPE_WHEEL);
        device.set_str(keys::KEY_SUBTYPE, hardware);
        device.set_str(keys::KEY_DEVPATH, path);
        device.set_int(keys::KEY_BAUD, baud);
        device.set_bool(keys::KEY_ENABLED, true);
        device.set_int(keys::KEY_FPS, MOZA_WHEEL_FPS);
        CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![device],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        }
    }

    fn hardware_name(value: i32, index: usize) -> &'static str {
        names::HARDWARE
            .iter()
            .filter(|entry| entry.value == value)
            .nth(index)
            .map(|entry| entry.name)
            .expect("hardware name")
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

    #[test]
    fn moza_r5_without_a_port_stays_closed() {
        const PORT: &str = "/dev/ttyMOZA-R5";
        let hardware = names::name_for(names::HARDWARE, names::HARDWARE_MOZA_R5).expect("r5");
        let config = moza_wheel_config(PORT, keys::BAUD_DEFAULT, hardware);
        let mut probed = String::new();
        let missing = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |path| {
                probed = path.to_string();
                false
            },
        );
        assert_eq!(probed, PORT);
        assert!(missing.devices.is_empty());
        assert!(missing
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_MOZA_R5_INIT));
        assert!(missing
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_SERIAL_OPEN_ERROR));
        assert!(missing
            .notices
            .iter()
            .all(|notice| notice.message != games::MSG_SERIAL_PORT_OPENED));
        assert!(missing
            .notices
            .iter()
            .all(|notice| notice.message != games::MSG_MOZA_ARMED));
        assert!(missing
            .notices
            .iter()
            .all(|notice| notice.message != games::moza_opened_message(PORT)));
        assert!(missing
            .notices
            .iter()
            .all(|notice| notice.message != games::MSG_SHARE_HANDLE));
    }

    #[test]
    fn moza_r5_writes_the_mask_every_tick() {
        const PORT: &str = "/dev/ttyMOZA-R5";
        const SAMPLE_RPM: u32 = 8_000;
        const SAMPLE_MAX: u32 = 8_000;
        const REPEATED_TICKS: usize = 2;
        let hardware = names::name_for(names::HARDWARE, names::HARDWARE_MOZA_R5).expect("r5");
        let config = moza_wheel_config(PORT, keys::BAUD_DEFAULT, hardware);
        let configured = serial_baud(keys::BAUD_DEFAULT);
        let floor = serial::moza_r9_open_baud(keys::BAUD_DEFAULT);
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
        assert_ne!(configured, floor);
        let subtype = games::serial_subtype_message(names::SUBTYPE_SERIAL_WHEEL);
        assert!(notice_before(
            &loaded.notices,
            &subtype,
            games::MSG_MOZA_R5_INIT
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_MOZA_R5_INIT,
            games::MSG_SERIAL_START
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_SERIAL_START,
            games::MSG_SERIAL_PORT_OPENED
        ));
        assert!(loaded
            .notices
            .iter()
            .any(|notice| { notice.message == games::serial_baud_message(configured) }));
        assert!(loaded
            .notices
            .iter()
            .all(|notice| notice.message != games::serial_baud_message(floor)));
        assert!(loaded
            .devices
            .captured_serial(0)
            .is_some_and(|frames| frames.is_empty()));
        let report = serial::moza_r5_report(SAMPLE_RPM, SAMPLE_MAX);
        let mut frame = Telemetry::new();
        frame.set_rpms(SAMPLE_RPM);
        frame.set_maxrpm(SAMPLE_MAX);
        let mut devices = loaded.devices;
        let mut tick = Vec::new();
        for _ in 0..REPEATED_TICKS {
            tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        }
        let frames = devices.captured_serial(0).expect("captured");
        assert_eq!(frames.len(), REPEATED_TICKS);
        assert!(frames
            .iter()
            .all(|written| written.as_slice() == report.bytes));
        let copied = games::moza_r5_copy_message(serial::MOZA_R5_PACKET_LEN);
        let wrote = games::moza_r5_write_message(&report.bytes, report.rpm, report.maxrpm);
        assert!(tick
            .iter()
            .any(|notice| notice.level == Level::Debug && notice.message == copied));
        assert!(tick
            .iter()
            .any(|notice| notice.level == Level::Trace && notice.message == wrote));
        let released = devices.release(PROBE_OPEN_NS);
        assert!(released
            .iter()
            .any(|notice| notice.message == games::serial_free_message(PORT)));
        assert!(released.iter().all(|notice| notice.message != copied));
        assert!(released.iter().all(|notice| notice.message != wrote));
        assert!(devices.captured_serial(0).is_none());
    }

    #[test]
    fn moza_r5_alias_opens_the_same_wheel() {
        const PORT: &str = "/dev/ttyMOZA-R3";
        const R5_ALIAS_INDEX: usize = 2;
        let alias = hardware_name(names::HARDWARE_MOZA_R5, R5_ALIAS_INDEX);
        let config = moza_wheel_config(PORT, keys::BAUD_DEFAULT, alias);
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
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_MOZA_R5_INIT));
    }

    #[test]
    fn moza_ks_pro_stays_closed() {
        const PORT: &str = "/dev/ttyMOZA-KS";
        let hardware = names::name_for(names::HARDWARE, names::HARDWARE_MOZA_KS_PRO).expect("ks");
        let config = moza_wheel_config(PORT, keys::BAUD_DEFAULT, hardware);
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| false,
            |_path| true,
        );
        assert!(loaded.devices.is_empty());
        assert!(loaded
            .notices
            .iter()
            .any(|notice| { notice.message == games::could_not_initialize_message(CLASS_SERIAL) }));
        assert!(loaded
            .notices
            .iter()
            .all(|notice| notice.message != games::MSG_MOZA_R5_INIT));
    }

    #[test]
    fn g29_writes_the_led_report_for_rpm() {
        const SAMPLE_RPM: u32 = 6_000;
        const SAMPLE_MAX: u32 = 7_000;
        const G29_FPS: i64 = 60;
        let config = g29_config(G29_FPS);
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
            .any(|notice| notice.message == games::MSG_G29_MISSING));
        assert!(missing.notices.iter().any(|notice| {
            notice.message == games::usb_init_error_message(games::ERROR_UNKNOWN)
        }));
        assert!(missing
            .notices
            .iter()
            .any(|notice| { notice.message == games::could_not_initialize_message(CLASS_USB) }));
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| true,
            |_path| false,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_G29_FOUND));
        let mut frame = Telemetry::new();
        frame.set_rpms(SAMPLE_RPM);
        frame.set_maxrpm(SAMPLE_MAX);
        let mut devices = loaded.devices;
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        let expected = usb::g29_report(SAMPLE_RPM, SAMPLE_MAX).to_vec();
        assert_eq!(
            devices.captured_reports(0).and_then(|frames| frames.last()),
            Some(&expected)
        );
        let rpm = i32::try_from(SAMPLE_RPM).unwrap_or(i32::MAX);
        assert!(tick
            .iter()
            .any(|notice| notice.message == games::g29_write_message(&expected, rpm)));
        let _ = devices.release(PROBE_OPEN_NS);
        let blank = usb::g29_report(G29_RELEASE_RPM, G29_RELEASE_RPM).to_vec();
        assert_eq!(
            devices.captured_reports(0).and_then(|frames| frames.last()),
            Some(&blank)
        );
    }

    fn g29_config(fps: i64) -> CargopitConfig {
        wheel_config(fps, names::HARDWARE_LOGITECH_G29)
    }

    #[test]
    fn c5_writes_the_led_report_for_rpm() {
        const SAMPLE_RPM: u32 = 6_000;
        const SAMPLE_MAX: u32 = 7_000;
        const SAMPLE_GEAR: u32 = 4;
        const SAMPLE_VELOCITY: u32 = 300;
        const C5_FPS: i64 = 60;
        let config = wheel_config(C5_FPS, names::HARDWARE_CAMMUS_C5);
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
            .any(|notice| notice.message == games::MSG_C5_MISSING));
        assert!(missing.notices.iter().any(|notice| {
            notice.message == games::usb_init_error_message(games::ERROR_UNKNOWN)
        }));
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| true,
            |_path| false,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_C5_FOUND));
        let mut frame = Telemetry::new();
        frame.set_rpms(SAMPLE_RPM);
        frame.set_maxrpm(SAMPLE_MAX);
        frame.set_gear(SAMPLE_GEAR);
        frame.set_velocity(SAMPLE_VELOCITY);
        let mut devices = loaded.devices;
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        let expected =
            usb::c5_report(SAMPLE_RPM, SAMPLE_MAX, SAMPLE_GEAR, SAMPLE_VELOCITY).to_vec();
        assert_eq!(
            devices.captured_reports(0).and_then(|frames| frames.last()),
            Some(&expected)
        );
        assert!(tick.iter().any(|notice| {
            notice.message
                == games::c5_write_message(
                    &expected,
                    telemetry_i32(SAMPLE_RPM),
                    telemetry_i32(SAMPLE_VELOCITY),
                    telemetry_i32(SAMPLE_GEAR),
                )
        }));
        let _ = devices.release(PROBE_OPEN_NS);
        let blank = usb::c5_report(
            C5_RELEASE_RPM,
            C5_RELEASE_RPM,
            C5_RELEASE_GEAR,
            C5_RELEASE_VELOCITY,
        )
        .to_vec();
        assert_eq!(
            devices.captured_reports(0).and_then(|frames| frames.last()),
            Some(&blank)
        );
    }

    #[test]
    fn c12_writes_the_led_report_for_rpm() {
        const SAMPLE_RPM: u32 = 6_000;
        const SAMPLE_MAX: u32 = 7_000;
        const SAMPLE_GEAR: u32 = 4;
        const SAMPLE_VELOCITY: u32 = 300;
        const C12_FPS: i64 = 60;
        let config = wheel_config(C12_FPS, names::HARDWARE_CAMMUS_C12);
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
            .any(|notice| notice.message == games::MSG_C12_MISSING));
        assert!(missing.notices.iter().any(|notice| {
            notice.message == games::usb_init_error_message(games::ERROR_UNKNOWN)
        }));
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| true,
            |_path| false,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_C12_FOUND));
        let mut frame = Telemetry::new();
        frame.set_rpms(SAMPLE_RPM);
        frame.set_maxrpm(SAMPLE_MAX);
        frame.set_gear(SAMPLE_GEAR);
        frame.set_velocity(SAMPLE_VELOCITY);
        let mut devices = loaded.devices;
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        let expected =
            usb::c12_report(SAMPLE_RPM, SAMPLE_MAX, SAMPLE_GEAR, SAMPLE_VELOCITY).to_vec();
        assert_eq!(
            devices.captured_reports(0).and_then(|frames| frames.last()),
            Some(&expected)
        );
        assert!(tick.iter().any(|notice| {
            notice.message
                == games::c12_write_message(
                    &expected,
                    telemetry_i32(SAMPLE_RPM),
                    telemetry_i32(SAMPLE_VELOCITY),
                    telemetry_i32(SAMPLE_GEAR),
                )
        }));
        let before = devices.captured_reports(0).map(|frames| frames.len());
        let _ = devices.release(PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            before
        );
        assert_eq!(
            devices.captured_reports(0).and_then(|frames| frames.last()),
            Some(&expected)
        );
    }

    #[test]
    fn c12_lua_writes_shift_light_packets() {
        const C12_FPS: i64 = 60;
        let path = std::env::temp_dir().join("cargopit-c12-leds.lua");
        let source = format!(
            "set_led_to_color({}, RED)\nset_led_to_color({}, BLUE)\n",
            usb::C12_LED_FIRST + usb::C12_LED_NUMBER_OFFSET,
            usb::C12_LED_TOTAL,
        );
        let _script = TempScript::write(&path, &source);
        let config = wheel_with_config(C12_FPS, names::HARDWARE_CAMMUS_C12, &path);
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| true,
            |_path| false,
        );
        assert_eq!(loaded.devices.len(), 1);
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_C12_FOUND));
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_C12_LUA));
        let shown = path.display().to_string();
        assert!(loaded
            .setup_notices
            .iter()
            .any(|notice| notice.message == games::tach_config_load_message(&shown)));
        let mut devices = loaded.devices;
        let tick = devices.tick(0, &Telemetry::new(), PROBE_OPEN_NS);
        let reports = devices.captured_reports(0).expect("captured");
        assert_eq!(reports.len(), usb::c12_led_reports(&[]).len());
        let first_led =
            u8::try_from(usb::C12_LED_FIRST + usb::C12_LED_NUMBER_OFFSET).unwrap_or(u8::MAX);
        assert_eq!(reports[0][usb::C12_BYTE_LED], first_led);
        assert_eq!(reports[0][usb::C12_BYTE_RED], u8::MAX);
        assert_eq!(reports[0][usb::C12_BYTE_GREEN], 0);
        assert_eq!(reports[0][usb::C12_BYTE_BLUE], 0);
        let last = reports.last().expect("last led");
        assert_eq!(
            last[usb::C12_BYTE_LED],
            u8::try_from(usb::C12_LED_TOTAL).unwrap_or(u8::MAX)
        );
        assert_eq!(last[usb::C12_BYTE_BLUE], u8::MAX);
        assert_eq!(last[usb::C12_BYTE_RED], 0);
        assert!(tick
            .iter()
            .any(|notice| notice.message == games::c12_led_message(&reports[0])));
        assert_eq!(
            games::c12_led_message(&reports[0]),
            "writing bytes xfaxfbx02x10xffx00x00 from red 255 green 0 blue 0"
        );
        let before = reports.len();
        let released = devices.release(PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            Some(before)
        );
        assert!(released
            .iter()
            .any(|notice| notice.message == games::MSG_C12_LUA_CLOSE));
    }

    #[test]
    fn c12_lua_failure_is_not_scheduled() {
        const C12_FPS: i64 = 60;
        let path = std::env::temp_dir().join("cargopit-c12-missing.lua");
        let _ = std::fs::remove_file(&path);
        let config = wheel_with_config(C12_FPS, names::HARDWARE_CAMMUS_C12, &path);
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| true,
            |_path| false,
        );
        assert!(loaded.devices.is_empty());
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_C12_FOUND));
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_C12_LUA));
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::MSG_C12_LUA_ISSUE));
        assert!(loaded.notices.iter().any(|notice| {
            notice.message == games::usb_init_error_message(games::USB_INIT_LUA_FAILED)
        }));
        assert!(loaded
            .notices
            .iter()
            .any(|notice| notice.message == games::could_not_initialize_message(CLASS_USB)));
    }

    #[test]
    fn gt_neo_without_config_does_not_open() {
        const GT_FPS: i64 = 60;
        let config = wheel_config(GT_FPS, names::HARDWARE_SIMAGIC_GT_NEO);
        let missing = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| panic!("hid open"),
            |_path| false,
        );
        assert!(missing.devices.is_empty());
        assert!(missing
            .setup_notices
            .iter()
            .any(|notice| notice.message == games::MSG_TACH_CONFIG_NONE));
        assert!(notice_has(&missing.notices, games::MSG_GT_ATTEMPT));
        assert!(notice_has(&missing.notices, games::MSG_GT_NEEDS_CONFIG));
        assert!(notice_has(
            &missing.notices,
            &games::usb_init_error_message(games::USB_INIT_LUA_FAILED)
        ));
        assert!(notice_has(
            &missing.notices,
            &games::could_not_initialize_message(CLASS_USB)
        ));
        assert!(notice_absent(&missing.notices, games::MSG_GT_INIT));
        assert!(notice_absent(&missing.notices, games::MSG_GT_FOUND));
        assert!(notice_absent(&missing.notices, games::MSG_GT_LUA));
        let mut named = wheel_config(GT_FPS, names::HARDWARE_SIMAGIC_GT_NEO);
        named.profiles[0].devices[0].set_str(keys::KEY_CONFIG, keys::CONFIG_VALUE_NONE);
        let named = open_profile_ports(
            &named,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| panic!("hid open"),
            |_path| false,
        );
        assert!(named.devices.is_empty());
        assert!(named
            .setup_notices
            .iter()
            .all(|notice| notice.message != games::MSG_TACH_CONFIG_NONE));
        assert!(notice_has(&named.notices, games::MSG_GT_NEEDS_CONFIG));
        assert!(notice_absent(&named.notices, games::MSG_GT_INIT));
    }

    #[test]
    fn gt_neo_missing_wheel_is_not_scheduled() {
        const GT_FPS: i64 = 60;
        let path = std::env::temp_dir().join("cargopit-gt-neo-absent.lua");
        let config = wheel_with_config(GT_FPS, names::HARDWARE_SIMAGIC_GT_NEO, &path);
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
        assert!(notice_has(&missing.notices, games::MSG_GT_INIT));
        assert!(notice_has(&missing.notices, games::MSG_GT_MISSING));
        assert!(notice_has(
            &missing.notices,
            &games::usb_init_error_message(games::ERROR_UNKNOWN)
        ));
        assert!(notice_absent(&missing.notices, games::MSG_GT_LUA));
        assert!(notice_absent(&missing.notices, games::MSG_C12_LUA_ISSUE));
        assert!(notice_absent(&missing.notices, games::MSG_GT_FOUND));
    }

    #[test]
    fn gt_neo_lua_sends_feature_reports() {
        const GT_FPS: i64 = 60;
        const FIRST_LED_RED: usize = 0;
        let path = std::env::temp_dir().join("cargopit-gt-neo-leds.lua");
        let source = format!("set_led_to_color({}, RED)\n", usb::GT_NEO_LUA_FIRST);
        let _script = TempScript::write(&path, &source);
        let config = wheel_with_config(GT_FPS, names::HARDWARE_SIMAGIC_GT_NEO, &path);
        let mut opened_gt = false;
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |vendor, product| {
                opened_gt = vendor == usb::GT_NEO_VID && product == usb::GT_NEO_PID;
                opened_gt
            },
            |_path| false,
        );
        assert!(opened_gt);
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_has(&loaded.notices, games::MSG_GT_FOUND));
        assert!(notice_has(&loaded.notices, games::MSG_GT_LUA));
        let shown = path.display().to_string();
        assert!(loaded
            .setup_notices
            .iter()
            .any(|notice| notice.message == games::tach_config_load_message(&shown)));
        let mut colors = vec![0u8; usb::gt_neo_color_len()];
        colors[FIRST_LED_RED] = u8::MAX;
        let expected: Vec<Vec<u8>> = usb::gt_neo_reports(&colors)
            .into_iter()
            .map(|report| report.to_vec())
            .collect();
        let mut devices = loaded.devices;
        let _tick = devices.tick(0, &Telemetry::new(), PROBE_OPEN_NS);
        assert_eq!(devices.captured_reports(0), Some(expected.as_slice()));
        let before = expected.len();
        let released = devices.release(PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            Some(before)
        );
        assert!(notice_absent(&released, games::MSG_C12_LUA_CLOSE));
    }

    #[test]
    fn gt_neo_lua_failure_is_not_scheduled() {
        const GT_FPS: i64 = 60;
        let path = std::env::temp_dir().join("cargopit-gt-neo-missing.lua");
        let _ = std::fs::remove_file(&path);
        let config = wheel_with_config(GT_FPS, names::HARDWARE_SIMAGIC_GT_NEO, &path);
        let loaded = open_profile_ports(
            &config,
            0,
            false,
            PLAY_USES_PULSES,
            PROBE_OPEN_NS,
            |_vendor, _product| true,
            |_path| false,
        );
        assert!(loaded.devices.is_empty());
        assert!(notice_has(&loaded.notices, games::MSG_GT_FOUND));
        assert!(notice_has(&loaded.notices, games::MSG_GT_LUA));
        assert!(notice_has(&loaded.notices, games::MSG_C12_LUA_ISSUE));
        assert!(notice_has(
            &loaded.notices,
            &games::usb_init_error_message(games::USB_INIT_LUA_FAILED)
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::could_not_initialize_message(CLASS_USB)
        ));
    }

    #[test]
    fn csl_missing_pedal_is_not_scheduled() {
        let config = csl_config();
        let missing = open_csl(&config, true, |_pattern| CSL_PROBE_MISSING);
        assert!(missing.devices.is_empty());
        assert!(notice_has(&missing.notices, games::MSG_CSL_INIT));
        assert!(notice_has(&missing.notices, games::MSG_CSL_MISSING));
        assert!(notice_has(
            &missing.notices,
            &games::usb_init_error_message(games::ERROR_UNKNOWN)
        ));
        assert!(notice_absent(&missing.notices, games::MSG_CSL_FOUND));
        let permission = open_csl(&config, true, |_pattern| CSL_PROBE_PERMISSION);
        assert!(permission.devices.is_empty());
        assert!(notice_has(&permission.notices, games::MSG_CSL_PERMISSION));
        assert!(notice_has(
            &permission.notices,
            &games::usb_init_error_message(games::USB_INIT_CSL_PERMISSION)
        ));
        let unreadable = open_csl(&config, true, |_pattern| CSL_PROBE_OPEN_FAILED);
        assert!(unreadable.devices.is_empty());
        assert!(notice_has(&unreadable.notices, games::MSG_CSL_OPEN));
        assert!(notice_has(
            &unreadable.notices,
            &games::usb_init_error_message(games::ERROR_UNKNOWN)
        ));
    }

    #[test]
    fn csl_slip_writes_rumble_when_the_effect_changes() {
        const CSL_SPEED: u32 = 80;
        const CSL_Y_VELOCITY: f64 = 1.0;
        const CSL_GAS: f64 = 0.2;
        const CSL_GAS_OFF: f64 = 0.0;
        const CSL_SLIP: f64 = -0.4;
        const CSL_SLIP_MORE: f64 = -0.8;
        const WHEEL_FRONT_LEFT: usize = 0;
        let config = csl_config();
        let mut pattern = String::new();
        let loaded = open_csl(&config, true, |seen| {
            pattern = seen.to_string();
            CSL_PROBE_CAPTURED
        });
        assert_eq!(pattern, usb::CSL_SYSFS_GLOB);
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_before(
            &loaded.notices,
            &games::haptic_effect_message(games::VIBRATION_SLIP),
            games::MSG_INIT_USB
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_INIT_USB,
            games::MSG_CSL_FOUND
        ));
        assert!(notice_has(&loaded.notices, games::MSG_CSL_ATTEMPT));
        assert!(notice_has(&loaded.notices, games::MSG_CSL_INIT));
        let slip = usb::csl_rumble_text(VibrationEffect::TyreSlip, CSL_GAS);
        let idle = usb::csl_rumble_text(VibrationEffect::TyreSlip, CSL_GAS_OFF);
        let slip_bytes = slip.into_bytes();
        let idle_bytes = idle.into_bytes();
        let mut devices = loaded.devices;
        let frame = csl_frame(
            CSL_SPEED,
            CSL_Y_VELOCITY,
            CSL_GAS,
            WHEEL_FRONT_LEFT,
            CSL_SLIP,
        );
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_sysfs(0).and_then(|frames| frames.last()),
            Some(&slip_bytes)
        );
        let once = devices.captured_sysfs(0).map(|frames| frames.len());
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert_eq!(devices.captured_sysfs(0).map(|frames| frames.len()), once);
        let stronger = csl_frame(
            CSL_SPEED,
            CSL_Y_VELOCITY,
            CSL_GAS,
            WHEEL_FRONT_LEFT,
            CSL_SLIP_MORE,
        );
        let _ = devices.tick(0, &stronger, PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_sysfs(0).map(|frames| frames.len()),
            once.map(|count| count.saturating_add(1))
        );
        let coast = csl_frame(
            CSL_SPEED,
            CSL_Y_VELOCITY,
            CSL_GAS_OFF,
            WHEEL_FRONT_LEFT,
            CSL_SLIP,
        );
        let _ = devices.tick(0, &coast, PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_sysfs(0).and_then(|frames| frames.last()),
            Some(&idle_bytes)
        );
        let before = devices.captured_sysfs(0).map(|frames| frames.len());
        let _ = devices.release(PROBE_OPEN_NS);
        assert_eq!(devices.captured_sysfs(0).map(|frames| frames.len()), before);
    }

    #[test]
    fn csl_without_haptic_support_opens_and_stays_quiet() {
        const CSL_SPEED: u32 = 80;
        const CSL_Y_VELOCITY: f64 = 1.0;
        const CSL_GAS: f64 = 0.2;
        const CSL_SLIP: f64 = -0.4;
        const WHEEL_FRONT_LEFT: usize = 0;
        let config = csl_config();
        let loaded = open_csl(&config, false, |_pattern| CSL_PROBE_CAPTURED);
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_before(
            &loaded.notices,
            games::MSG_USB_NO_HAPTICS,
            games::MSG_INIT_USB
        ));
        assert!(notice_has(&loaded.notices, games::MSG_CSL_FOUND));
        assert!(notice_absent(
            &loaded.notices,
            &games::haptic_effect_message(games::VIBRATION_SLIP)
        ));
        let mut devices = loaded.devices;
        let frame = csl_frame(
            CSL_SPEED,
            CSL_Y_VELOCITY,
            CSL_GAS,
            WHEEL_FRONT_LEFT,
            CSL_SLIP,
        );
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_sysfs(0).map(|frames| frames.len()),
            Some(0)
        );
    }

    #[test]
    fn p1000_missing_pedal_is_not_scheduled() {
        let config = p1000_config();
        let mut seen = (0u16, 0u16);
        let missing = open_probed_hid(&config, true, |vendor, product| {
            seen = (vendor, product);
            false
        });
        assert_eq!(seen, (usb::P1000_VID, usb::P1000_PID));
        assert!(missing.devices.is_empty());
        assert!(notice_has(&missing.notices, &games::p1000_init_message()));
        assert!(notice_has(
            &missing.notices,
            &games::p1000_missing_message()
        ));
        assert!(notice_has(
            &missing.notices,
            &games::usb_init_error_message(games::ERROR_UNKNOWN)
        ));
        assert!(notice_has(
            &missing.notices,
            &games::could_not_initialize_message(CLASS_USB)
        ));
        assert!(notice_absent(
            &missing.notices,
            &games::p1000_found_message()
        ));
        assert!(notice_absent(
            &missing.notices,
            &games::p1000_sent_message(p1000_nbytes())
        ));
    }

    #[test]
    fn p1000_slip_sends_feature_reports_when_play_changes() {
        const P1000_SPEED: u32 = 80;
        const P1000_Y_VELOCITY: f64 = 1.0;
        const P1000_GAS: f64 = 0.2;
        const P1000_GAS_OFF: f64 = 0.0;
        const P1000_SLIP: f64 = -0.4;
        const P1000_SLIP_MORE: f64 = -0.8;
        const WHEEL_FRONT_LEFT: usize = 0;
        let config = p1000_config();
        let loaded = open_probed_hid(&config, true, |vendor, product| {
            vendor == usb::P1000_VID && product == usb::P1000_PID
        });
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_before(
            &loaded.notices,
            &games::haptic_effect_message(games::VIBRATION_SLIP),
            games::MSG_INIT_USB
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_INIT_USB,
            &games::p1000_found_message()
        ));
        assert!(notice_has(&loaded.notices, &games::p1000_init_message()));
        assert!(notice_has(
            &loaded.notices,
            &games::p1000_sent_message(p1000_nbytes())
        ));
        let init = usb::p1000_init_report();
        assert!(notice_has(
            &loaded.notices,
            &games::p1000_bytes_message(&init)
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::p1000_init_result_message(usb::P1000_REPORT_OK)
        ));
        assert!(notice_absent(
            &loaded.notices,
            &games::p1000_problem_message()
        ));
        let mut devices = loaded.devices;
        assert_eq!(
            devices
                .captured_reports(0)
                .and_then(|frames| frames.first()),
            Some(&init.to_vec())
        );
        let active = usb::p1000_reports(VibrationEffect::TyreSlip, P1000_GAS);
        let frame = csl_frame(
            P1000_SPEED,
            P1000_Y_VELOCITY,
            P1000_GAS,
            WHEEL_FRONT_LEFT,
            P1000_SLIP,
        );
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_has(
            &tick,
            &games::p1000_sent_message(p1000_nbytes())
        ));
        assert!(notice_has(
            &tick,
            &games::p1000_bytes_message(active.first().expect("slip report"))
        ));
        const INIT_REPORTS: usize = 1;
        let after_slip = devices.captured_reports(0).map(|frames| frames.len());
        assert_eq!(after_slip, Some(INIT_REPORTS.saturating_add(active.len())));
        let expected_active = active.first().expect("slip report").to_vec();
        assert_eq!(
            devices.captured_reports(0).and_then(|frames| frames.last()),
            Some(&expected_active)
        );
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            after_slip
        );
        let stronger = csl_frame(
            P1000_SPEED,
            P1000_Y_VELOCITY,
            P1000_GAS,
            WHEEL_FRONT_LEFT,
            P1000_SLIP_MORE,
        );
        let _ = devices.tick(0, &stronger, PROBE_OPEN_NS);
        let after_stronger = after_slip.map(|count| count.saturating_add(active.len()));
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            after_stronger
        );
        let coast = csl_frame(
            P1000_SPEED,
            P1000_Y_VELOCITY,
            P1000_GAS_OFF,
            WHEEL_FRONT_LEFT,
            P1000_SLIP,
        );
        let idle = usb::p1000_reports(VibrationEffect::TyreSlip, P1000_GAS_OFF);
        const IDLE_PAIR: usize = 2;
        assert_eq!(idle.len(), IDLE_PAIR);
        let _ = devices.tick(0, &coast, PROBE_OPEN_NS);
        let expected_len = after_stronger.unwrap_or(0).saturating_add(idle.len());
        let (before, idle_matches) = {
            let frames = devices.captured_reports(0).expect("captured");
            let tail = frames.len().saturating_sub(idle.len());
            let lock = frames.get(tail).map(Vec::as_slice) == Some(idle[0].as_slice());
            let slip =
                frames.get(tail.saturating_add(1)).map(Vec::as_slice) == Some(idle[1].as_slice());
            let matches = lock && slip;
            (frames.len(), matches)
        };
        assert_eq!(before, expected_len);
        assert!(idle_matches);
        let _ = devices.release(PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            Some(before)
        );
    }

    #[test]
    fn p1000_without_haptic_support_sends_init_and_stays_quiet() {
        const P1000_SPEED: u32 = 80;
        const P1000_Y_VELOCITY: f64 = 1.0;
        const P1000_GAS: f64 = 0.2;
        const P1000_SLIP: f64 = -0.4;
        const WHEEL_FRONT_LEFT: usize = 0;
        const INIT_ONLY: usize = 1;
        let config = p1000_config();
        let loaded = open_probed_hid(&config, false, |vendor, product| {
            vendor == usb::P1000_VID && product == usb::P1000_PID
        });
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_before(
            &loaded.notices,
            games::MSG_USB_NO_HAPTICS,
            games::MSG_INIT_USB
        ));
        assert!(notice_has(&loaded.notices, &games::p1000_found_message()));
        assert!(notice_absent(
            &loaded.notices,
            &games::haptic_effect_message(games::VIBRATION_SLIP)
        ));
        let mut devices = loaded.devices;
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            Some(INIT_ONLY)
        );
        let frame = csl_frame(
            P1000_SPEED,
            P1000_Y_VELOCITY,
            P1000_GAS,
            WHEEL_FRONT_LEFT,
            P1000_SLIP,
        );
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            Some(INIT_ONLY)
        );
    }

    #[test]
    fn simnet_missing_pedal_is_not_scheduled() {
        let config = simnet_config();
        let mut seen = (0u16, 0u16);
        let missing = open_probed_hid(&config, true, |vendor, product| {
            seen = (vendor, product);
            false
        });
        assert_eq!(seen, (usb::SIMNET_VID, usb::SIMNET_PID));
        assert!(missing.devices.is_empty());
        assert!(notice_has(&missing.notices, games::MSG_SIMNET_INIT));
        assert!(notice_has(&missing.notices, games::MSG_SIMNET_MISSING));
        assert!(notice_has(
            &missing.notices,
            &games::usb_init_error_message(games::ERROR_UNKNOWN)
        ));
        assert!(notice_has(
            &missing.notices,
            &games::could_not_initialize_message(CLASS_USB)
        ));
        assert!(notice_absent(
            &missing.notices,
            &games::simnet_found_message(games::SIMNET_CAPTURED_HANDLE)
        ));
    }

    #[test]
    fn simnet_slip_writes_when_play_changes() {
        const SIMNET_SPEED: u32 = 80;
        const SIMNET_Y_VELOCITY: f64 = 1.0;
        const SIMNET_GAS: f64 = 0.2;
        const SIMNET_GAS_OFF: f64 = 0.0;
        const SIMNET_SLIP: f64 = -0.4;
        const SIMNET_SLIP_MORE: f64 = -0.8;
        const WHEEL_FRONT_LEFT: usize = 0;
        const SIMNET_MOTOR: u32 = 1;
        const SIMNET_FREQUENCY: u32 = 40;
        const SIMNET_AMPLITUDE: u32 = 100;
        let config = simnet_config();
        let loaded = open_probed_hid(&config, true, |vendor, product| {
            vendor == usb::SIMNET_VID && product == usb::SIMNET_PID
        });
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_before(
            &loaded.notices,
            &games::haptic_effect_message(games::VIBRATION_SLIP),
            games::MSG_INIT_USB
        ));
        assert!(notice_before(
            &loaded.notices,
            games::MSG_INIT_USB,
            &games::simnet_found_message(games::SIMNET_CAPTURED_HANDLE)
        ));
        assert!(notice_has(&loaded.notices, games::MSG_SIMNET_INIT));
        assert!(notice_absent(&loaded.notices, games::MSG_SIMNET_MISSING));
        let active = usb::simnet_report(SIMNET_MOTOR, SIMNET_FREQUENCY, SIMNET_AMPLITUDE, true);
        let idle = usb::simnet_report(SIMNET_MOTOR, SIMNET_FREQUENCY, SIMNET_AMPLITUDE, false);
        let mut devices = loaded.devices;
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            Some(0)
        );
        let frame = csl_frame(
            SIMNET_SPEED,
            SIMNET_Y_VELOCITY,
            SIMNET_GAS,
            WHEEL_FRONT_LEFT,
            SIMNET_SLIP,
        );
        let tick = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert!(notice_has(&tick, &games::simnet_write_message(&active)));
        assert_eq!(
            devices.captured_reports(0).and_then(|frames| frames.last()),
            Some(&active.to_vec())
        );
        let once = devices.captured_reports(0).map(|frames| frames.len());
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert_eq!(devices.captured_reports(0).map(|frames| frames.len()), once);
        let stronger = csl_frame(
            SIMNET_SPEED,
            SIMNET_Y_VELOCITY,
            SIMNET_GAS,
            WHEEL_FRONT_LEFT,
            SIMNET_SLIP_MORE,
        );
        let _ = devices.tick(0, &stronger, PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            once.map(|count| count.saturating_add(1))
        );
        assert_eq!(
            devices.captured_reports(0).and_then(|frames| frames.last()),
            Some(&active.to_vec())
        );
        let coast = csl_frame(
            SIMNET_SPEED,
            SIMNET_Y_VELOCITY,
            SIMNET_GAS_OFF,
            WHEEL_FRONT_LEFT,
            SIMNET_SLIP,
        );
        let _ = devices.tick(0, &coast, PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_reports(0).and_then(|frames| frames.last()),
            Some(&idle.to_vec())
        );
        let before = devices.captured_reports(0).map(|frames| frames.len());
        let _ = devices.release(PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            before
        );
    }

    #[test]
    fn simnet_without_haptic_support_opens_and_stays_quiet() {
        const SIMNET_SPEED: u32 = 80;
        const SIMNET_Y_VELOCITY: f64 = 1.0;
        const SIMNET_GAS: f64 = 0.2;
        const SIMNET_SLIP: f64 = -0.4;
        const WHEEL_FRONT_LEFT: usize = 0;
        let config = simnet_config();
        let loaded = open_probed_hid(&config, false, |vendor, product| {
            vendor == usb::SIMNET_VID && product == usb::SIMNET_PID
        });
        assert_eq!(loaded.devices.len(), 1);
        assert!(notice_before(
            &loaded.notices,
            games::MSG_USB_NO_HAPTICS,
            games::MSG_INIT_USB
        ));
        assert!(notice_has(
            &loaded.notices,
            &games::simnet_found_message(games::SIMNET_CAPTURED_HANDLE)
        ));
        assert!(notice_absent(
            &loaded.notices,
            &games::haptic_effect_message(games::VIBRATION_SLIP)
        ));
        let mut devices = loaded.devices;
        let frame = csl_frame(
            SIMNET_SPEED,
            SIMNET_Y_VELOCITY,
            SIMNET_GAS,
            WHEEL_FRONT_LEFT,
            SIMNET_SLIP,
        );
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        assert_eq!(
            devices.captured_reports(0).map(|frames| frames.len()),
            Some(0)
        );
    }

    fn simnet_config() -> CargopitConfig {
        const SIMNET_FPS: i64 = 60;
        const SIMNET_MOTOR: i64 = 1;
        const SIMNET_FREQUENCY: i64 = 40;
        const SIMNET_AMPLITUDE: i64 = 100;
        let mut config = haptic_wheel_config(SIMNET_FPS, names::HARDWARE_SIMNET);
        let device = &mut config.profiles[0].devices[0];
        device.set_int(keys::KEY_MOTORS, SIMNET_MOTOR);
        device.set_int(keys::KEY_FREQUENCY, SIMNET_FREQUENCY);
        device.set_int(keys::KEY_AMPLITUDE, SIMNET_AMPLITUDE);
        config
    }

    fn p1000_nbytes() -> i32 {
        i32::try_from(usb::P1000_LEN).unwrap_or(i32::MAX)
    }

    fn p1000_config() -> CargopitConfig {
        const P1000_FPS: i64 = 60;
        haptic_wheel_config(P1000_FPS, names::HARDWARE_SIMAGIC_P1000)
    }

    fn open_probed_hid(
        config: &CargopitConfig,
        supports_haptics: bool,
        mut hid: impl FnMut(u16, u16) -> bool,
    ) -> ProfileLoad {
        open_profile_with(
            config,
            0,
            false,
            PLAY_USES_PULSES,
            supports_haptics,
            None,
            &mut HidAttempt::Probe {
                hid: &mut hid,
                serial: &mut |_path| false,
                sysfs: None,
                now_ns: PROBE_OPEN_NS,
                led_reply: None,
            },
        )
    }

    fn csl_config() -> CargopitConfig {
        const CSL_FPS: i64 = 60;
        haptic_wheel_config(CSL_FPS, names::HARDWARE_CSL_ELITE)
    }

    fn haptic_wheel_config(fps: i64, hardware: i32) -> CargopitConfig {
        let mut config = wheel_config(fps, hardware);
        let effect = names::name_for(names::EFFECTS, names::EFFECT_TYRE_SLIP).expect("slip name");
        config.profiles[0].devices[0].set_str(keys::KEY_EFFECT, effect);
        config
    }

    fn csl_frame(speed: u32, y_velocity: f64, gas: f64, wheel: usize, slip: f64) -> Telemetry {
        let mut frame = Telemetry::new();
        frame.set_velocity(speed);
        frame.set_y_velocity(y_velocity);
        frame.set_gas(gas);
        frame.set_tyre_slip(wheel, slip);
        frame
    }

    fn open_csl(
        config: &CargopitConfig,
        supports_haptics: bool,
        mut sysfs: impl FnMut(&str) -> i32,
    ) -> ProfileLoad {
        open_profile_with(
            config,
            0,
            false,
            PLAY_USES_PULSES,
            supports_haptics,
            None,
            &mut HidAttempt::Probe {
                hid: &mut |_vendor, _product| panic!("hid open"),
                serial: &mut |_path| false,
                sysfs: Some(&mut sysfs),
                now_ns: PROBE_OPEN_NS,
                led_reply: None,
            },
        )
    }

    fn notice_before(notices: &[InitNotice], earlier: &str, later: &str) -> bool {
        let Some(first) = notices.iter().position(|notice| notice.message == earlier) else {
            return false;
        };
        let Some(second) = notices.iter().position(|notice| notice.message == later) else {
            return false;
        };
        first < second
    }

    fn notice_has(notices: &[InitNotice], message: &str) -> bool {
        notices.iter().any(|notice| notice.message == message)
    }

    fn notice_absent(notices: &[InitNotice], message: &str) -> bool {
        notices.iter().all(|notice| notice.message != message)
    }

    fn wheel_with_config(fps: i64, hardware: i32, config_path: &Path) -> CargopitConfig {
        let mut config = wheel_config(fps, hardware);
        let shown = config_path.display().to_string();
        config.profiles[0].devices[0].set_str(keys::KEY_CONFIG, &shown);
        config
    }

    struct TempScript {
        path: PathBuf,
    }

    impl TempScript {
        fn write(path: &Path, source: &str) -> Self {
            std::fs::write(path, source).expect("lua script");
            Self {
                path: path.to_path_buf(),
            }
        }
    }

    impl Drop for TempScript {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn wheel_config(fps: i64, hardware: i32) -> CargopitConfig {
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_DEVICE, keys::CLASS_USB);
        device.set_str(keys::KEY_TYPE, keys::TYPE_WHEEL);
        let subtype = names::name_for(names::HARDWARE, hardware).expect("wheel name");
        device.set_str(keys::KEY_SUBTYPE, subtype);
        device.set_bool(keys::KEY_ENABLED, true);
        device.set_int(keys::KEY_FPS, fps);
        CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![device],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        }
    }

    const SOUND_SINK: &str = "alsa_output.usb-test.analog-stereo";
    const SOUND_FPS: i64 = 60;
    const SOUND_VOLUME: i64 = 40;
    const SOUND_OTHER_VOLUME: i64 = 80;
    const SOUND_CHANNELS: i64 = 2;
    const SOUND_FREQUENCY: i64 = 50;
    const SOUND_DURATION_S: f64 = 0.1;
    const SOUND_NOISE: i64 = 0;

    fn sound_entry(effect: &str, tyre: Option<&str>) -> DeviceEntry {
        let mut device = DeviceEntry::new();
        device.set_str(keys::KEY_DEVICE, keys::CLASS_SOUND);
        device.set_str(keys::KEY_EFFECT, effect);
        if let Some(tyre) = tyre {
            device.set_str(keys::KEY_TYRE, tyre);
        }
        device.set_str(keys::KEY_DEVID, SOUND_SINK);
        device.set_int(keys::KEY_FPS, SOUND_FPS);
        device.set_bool(keys::KEY_ENABLED, true);
        device.set_int(keys::KEY_STREAM_VOLUME, SOUND_VOLUME);
        device.set_int(keys::KEY_VOLUME, SOUND_OTHER_VOLUME);
        device.set_int(keys::KEY_CHANNELS, SOUND_CHANNELS);
        device.set_int(keys::KEY_PAN, sound_host::SOUND_PAN_ALL);
        device.set_int(keys::KEY_FREQUENCY, SOUND_FREQUENCY);
        device.set_int(keys::KEY_NOISE, SOUND_NOISE);
        device.set_float(keys::KEY_DURATION, SOUND_DURATION_S);
        device
    }

    fn sound_profile(device: DeviceEntry) -> CargopitConfig {
        CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![device],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        }
    }

    fn stereo_all_mask() -> u32 {
        const CHANNEL_BIT: u32 = 1;
        let channels = u32::try_from(SOUND_CHANNELS).unwrap_or(0);
        (CHANNEL_BIT << channels) - CHANNEL_BIT
    }

    #[test]
    fn sound_devices_log_the_c_init_sequence() {
        let gear = open_profile_at(
            &sound_profile(sound_entry("Gear", None)),
            0,
            false,
            PROBE_OPEN_NS,
            false,
            None,
        );
        assert_eq!(gear.devices.len(), 1);
        assert_eq!(gear.devices.effect(0), Some(names::EFFECT_GEAR));
        let gear_node = games::sound_node_message("cargopit.Gear");
        let expected = vec![
            games::haptic_effect_message(games::VIBRATION_GEAR),
            games::haptic_summary_message(names::EFFECT_GEAR, names::TYRE_FRONT_LEFT),
            games::haptic_duration_message(SOUND_DURATION_S),
            games::haptic_frequency_message(SOUND_FREQUENCY),
            games::haptic_amplitude_message(sound_host::SOUND_AMPLITUDE_UNITY),
            games::haptic_motor_message(sound_host::SOUND_MOTOR_DEFAULT),
            games::sound_subtype_message(names::EFFECT_GEAR),
            games::sound_effect_message(games::VIBRATION_GEAR),
            games::sound_use_message(SOUND_SINK),
            games::MSG_SOUND_STANDALONE.to_string(),
            games::sound_volume_message(SOUND_VOLUME),
            games::sound_channel_mask_message(stereo_all_mask()),
            games::sound_channels_message(SOUND_CHANNELS),
            games::sound_noise_message(SOUND_NOISE),
            gear_node,
            games::initialized_devices_message(1),
            games::starting_device_message(
                names::DEVICE_SOUND,
                0,
                u32::try_from(SOUND_FPS).unwrap_or(0),
            ),
        ];
        let gear_messages: Vec<String> = gear
            .notices
            .iter()
            .map(|notice| notice.message.clone())
            .collect();
        assert_eq!(gear_messages, expected);
        let captured = gear.devices.captured_sound(0).expect("captured shaker");
        assert_eq!(captured.node, "cargopit.Gear");
        assert_eq!(captured.sink, SOUND_SINK);
        assert_eq!(captured.stream_name, "Gear");
        assert_eq!(captured.volume_percent, SOUND_VOLUME);
        assert!(captured.tyre_name.is_none());
        assert!(captured.gear);

        let lock = open_profile_at(
            &sound_profile(sound_entry("TyreLock", Some("ALL"))),
            0,
            false,
            PROBE_OPEN_NS,
            true,
            None,
        );
        assert_eq!(lock.devices.effect(0), Some(names::EFFECT_TYRE_LOCK));
        assert!(lock.notices.iter().any(|notice| {
            notice.message == games::sound_node_message("cargopit.TyreLock.All")
        }));
        assert_eq!(
            lock.devices
                .captured_sound(0)
                .and_then(|sound| sound.tyre_name.as_deref()),
            Some("All")
        );

        let blocked = open_profile_at(
            &sound_profile(sound_entry("Suspension", Some("All"))),
            0,
            false,
            PROBE_OPEN_NS,
            false,
            None,
        );
        assert!(blocked.devices.is_empty());
        assert_eq!(blocked.notices[0].message, games::MSG_SOUND_SKIP_HAPTICS);
        assert_eq!(
            blocked.notices[1].message,
            games::could_not_initialize_message(CLASS_SOUND)
        );
        assert_eq!(
            blocked.notices[2].message,
            games::initialized_devices_message(0)
        );
    }

    #[test]
    fn sound_connect_fails_when_pulse_is_not_ready() {
        let mut pulse = PulseSession::not_ready();
        let failed = open_profile_at(
            &sound_profile(sound_entry("Gear", None)),
            0,
            false,
            PROBE_OPEN_NS,
            true,
            Some(&mut pulse),
        );
        assert!(failed.devices.is_empty());
        assert!(failed.notices.iter().any(|notice| {
            notice.message
                == games::sound_connect_error(
                    "cargopit.Gear",
                    SOUND_SINK,
                    cargopit_devices::transport::PULSE_CONTEXT_MISSING,
                )
        }));
        assert!(failed
            .notices
            .iter()
            .any(|notice| { notice.message == games::could_not_initialize_message(CLASS_SOUND) }));
    }

    #[test]
    fn sound_tick_renders_engine_samples() {
        const ENGINE_RPM: u32 = 3_000;
        const ENGINE_IDLE: u32 = 800;
        const ENGINE_MAX: u32 = 7_000;
        const ENGINE_THROTTLE: f64 = 1.0;
        const BYTES_PER_SAMPLE: usize = 2;
        const RENDER_FRAMES: usize = 48;
        let loaded = open_profile_at(
            &sound_profile(sound_entry("Engine", None)),
            0,
            false,
            PROBE_OPEN_NS,
            true,
            None,
        );
        assert_eq!(loaded.devices.len(), 1);
        let channels = usize::try_from(SOUND_CHANNELS).unwrap_or(0);
        let nbytes = RENDER_FRAMES * BYTES_PER_SAMPLE * channels;
        let silent = loaded.devices.rendered_sound(0, nbytes).expect("voice");
        assert!(silent.iter().all(|byte| *byte == 0));
        let mut frame = Telemetry::new();
        frame.set_rpms(ENGINE_RPM);
        frame.set_idlerpm(ENGINE_IDLE);
        frame.set_maxrpm(ENGINE_MAX);
        frame.set_gas(ENGINE_THROTTLE);
        let mut devices = loaded.devices;
        let _ = devices.tick(0, &frame, PROBE_OPEN_NS);
        let played = devices.rendered_sound(0, nbytes).expect("voice");
        assert!(played.iter().any(|byte| *byte != 0));
    }
}
