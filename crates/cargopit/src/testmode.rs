//! Hardware-test announcements and telemetry. Ticks are counted, not slept.
//! Device backends stay closed.

use cargopit_config::config::{CargopitConfig, DeviceEntry};
use cargopit_config::keys::{self, KEY_THRESHOLD, KEY_TYRE};
use cargopit_config::names::{
    self, EFFECT_ABS, EFFECT_ENGINE, EFFECT_GEAR, EFFECT_SUSPENSION, EFFECT_TYRE_LOCK,
    EFFECT_TYRE_SLIP, TYRE_ALL_FOUR, TYRE_FRONTS, TYRE_FRONT_LEFT, TYRE_FRONT_RIGHT, TYRE_REARS,
    TYRE_REAR_LEFT, TYRE_REAR_RIGHT,
};
use cargopit_devices::clock::VirtualClock;
use cargopit_devices::haptic::{HapticEffect, HapticSettings, TyreId, VibrationEffect};
use cargopit_devices::telemetry::Telemetry;
use cargopit_devices::DeviceKind;
use simapi_sys::WHEEL_COUNT;

use crate::devices::{self, profile_index};
use crate::games;

pub const TICK_US: u64 = 16_000;
pub const STEP_PREFIX: &str = "test step: ";
pub const MSG_PREPARING: &str = "preparing test with";
pub const MSG_STARTING: &str = "Starting";
pub const MSG_FINISHED: &str = "Finished";
pub const MSG_STOPPED: &str = "Stopped";
pub const MSG_REV: &str = "Revving rpm from idle to redline and back";
pub const MSG_RPM_IDLE: &str = "Setting rpms to idle";
pub const MSG_COAST: &str = "Returning to idle";
pub const MSG_GREEN: &str = "Green Flag!";
pub const MSG_YELLOW: &str = "Yellow Flag!";
pub const MSG_BLUE: &str = "Blue Flag!";
pub const MSG_RED: &str = "Red Flag!";
pub const MSG_FIRST: &str = "Shifting into first gear";
pub const MSG_SECOND: &str = "Shifting into second gear";
pub const MSG_THIRD: &str = "Shifting into third gear";
pub const MSG_FOURTH: &str = "Shifting into fourth gear";
pub const MSG_SPEED_SLOW: &str = "Setting speed to 100";
pub const MSG_SPEED_FAST: &str = "Setting speed to 200";
pub const MSG_SPEED_TOP: &str = "Setting speed to 300";
pub const MSG_SPIN: &str = "Testing wheel spin";
pub const MSG_LOCK: &str = "Testing wheel lock";
pub const MSG_ABS: &str = "Testing ABS";
pub const MSG_BUTTONS: &str = "Lighting brake button LEDs";
pub const MSG_SUSPENSION: &str = "Testing suspension";
pub const MSG_NO_DEVICES: &str = "No devices loaded for test";
pub const MSG_DEVICE_INDEX: &str = "testing device index";
pub const LABEL_SERIAL_LIGHTS: &str = "serial lights";
pub const LABEL_USB_LIGHTS: &str = "USB lights";
pub const LABEL_DEVICE: &str = "device";
pub const LABEL_EFFECT: &str = "effect";
pub const LABEL_GEAR: &str = "gear";
pub const LABEL_TYRE_LOCK: &str = "tyre lock";
pub const LABEL_ABS: &str = "ABS";
pub const LABEL_TYRE_SLIP: &str = "tyre slip";
pub const LABEL_SUSPENSION: &str = "suspension";

const US_PER_MS: u64 = 1000;
const PHASE_HOLD_US: u64 = 3_000_000;
const IDLE_HOLD_US: u64 = 1_000_000;
const RPM_IDLE: u32 = 1000;
const RPM_MAX: u32 = 8000;
const RPM_SWEEP_STEP: u32 = 50;
const VELOCITY_CRUISE: u32 = 160;
const VELOCITY_SLOW: u32 = 100;
const VELOCITY_FAST: u32 = 200;
const VELOCITY_TOP: u32 = 300;
const VELOCITY_SPIN: u32 = 15;
const VELOCITY_LOCK: u32 = 150;
const VELOCITY_IDLE: u32 = 0;
const Y_VELOCITY: f64 = 100.0;
const AXIS_IDLE: f64 = 0.0;
const PEDAL_APPLIED: f64 = 0.85;
const GAS_CRUISE: f64 = 0.40;
const SLIP_SPIN: f64 = -0.45;
const SLIP_LOCK: f64 = 0.85;
const SLIP_ABS_A: f64 = 0.40;
const SLIP_ABS_B: f64 = 0.72;
const SLIP_CLEAR: f64 = 0.0;
const ABS_OFF: f64 = 0.0;
const ABS_ACTIVE: f64 = 1.0;
const BRAKE_TEMP_HOT: f64 = 0.90;
const BRAKE_TEMP_COLD: f64 = 0.0;
const SUSP_VEL_A: f64 = 2.0;
const SUSP_VEL_B: f64 = 18.0;
const TYRE_DIAMETER_UNSET: f64 = -1.0;
const TYRE_DIAMETER_FL: f64 = 0.638636385206394;
const TYRE_DIAMETER_FR: f64 = 0.633384434597093;
const TYRE_DIAMETER_RL: f64 = 0.710475735564615;
const TYRE_DIAMETER_RR: f64 = 0.710475735564615;
const TYRE_RPS_SPIN: f64 = 50.0;
const TYRE_RPS_LOCK: f64 = 25.0;
const TYRE_RPS_CLEAR: f64 = 0.0;
const CAR_NAME: &str = "CAR";
const GEAR_CHAR_NEUTRAL: char = 'N';
const GEAR_CHAR_FIRST: char = '1';
const GEAR_CHAR_SECOND: char = '2';
const GEAR_CHAR_THIRD: char = '3';
const GEAR_CHAR_FOURTH: char = '4';
const GEAR_NEUTRAL: u32 = 1;
const GEAR_FIRST: u32 = 2;
const GEAR_SECOND: u32 = 3;
const GEAR_THIRD: u32 = 4;
const GEAR_FOURTH: u32 = 5;
const GEAR_PULSE_TICKS: u64 = 8;
const ABS_PULSE_TICKS: u64 = 2;
const STATUS_OFF: u32 = 0;
const FLAG_GREEN: u8 = 0;
const FLAG_YELLOW: u8 = 1;
const FLAG_RED: u8 = 2;
const FLAG_BLUE: u8 = 4;
const DEVICE_INDEX_SELECT: i32 = 0;
const C_PRINTF_F_PRECISION: usize = 6;
const QUIT_KEY: u8 = b'q';
const QUIT_KEY_UPPER: u8 = b'Q';
const QUIT_KEY_ESC: u8 = 0o33;
const SIMAPI_TEST: u8 = simapi_sys::bindings::SimulatorAPI_SIMULATORAPI_SIMAPI_TEST as u8;
const SIMEXE_TEST_NONE: u64 =
    simapi_sys::bindings::SimulatorEXE_SIMULATOREXE_SIMAPI_TEST_NONE as u64;
const SIMAPI_VERSION: u8 = simapi_sys::SIMAPI_VERSION_VALUE as u8;
const THRESHOLD_DEFAULT: f64 = 0.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mod {
    None,
    Gear { gear: u32, ch: char },
    Abs,
    Suspension,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestSubject {
    pub kind: DeviceKind,
    pub effect: Option<i32>,
    pub active: bool,
    pub threshold: f64,
    pub tyre: i32,
}

impl TestSubject {
    pub fn lights(kind: DeviceKind) -> Self {
        Self {
            kind,
            effect: None,
            active: true,
            threshold: THRESHOLD_DEFAULT,
            tyre: TYRE_FRONT_LEFT,
        }
    }

    pub fn haptic(kind: DeviceKind, effect: i32) -> Self {
        Self {
            kind,
            effect: Some(effect),
            active: true,
            threshold: THRESHOLD_DEFAULT,
            tyre: TYRE_FRONT_LEFT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickView {
    pub rpms: u32,
    pub velocity: u32,
    pub gear: u32,
    pub gear_char: u8,
    pub slip: f64,
    pub flag: u8,
    pub susp: f64,
    pub abs_value: f64,
    pub diameter_fl: f64,
    pub simon: bool,
}

pub struct RunOptions<'a> {
    pub subjects: &'a [TestSubject],
    pub device_index: Option<u32>,
    pub trace: bool,
    pub publish: bool,
}

pub struct Run {
    pub lines: Vec<String>,
    pub tick_count: u64,
    pub stopped: bool,
    pub simon: bool,
    pub simstatus: u32,
    pub velocity: u32,
    pub rpms: u32,
    pub gear: u32,
    pub flag: u8,
    pub car: String,
    pub mtick: u64,
    pub trace: Vec<TickView>,
}

pub enum Plan {
    Empty,
    MissingIndex,
    Ready {
        subjects: Vec<TestSubject>,
        device_index: Option<u32>,
    },
}

pub fn preparing(device_count: usize) -> String {
    format!("{STEP_PREFIX}{MSG_PREPARING} {device_count} devices...")
}

pub fn named(action: &str, label: &str) -> String {
    format!("{STEP_PREFIX}{action} {label}")
}

pub fn step(message: &str) -> String {
    format!("{STEP_PREFIX}{message}")
}

pub fn tick_count(hold_us: u64) -> u64 {
    hold_us / TICK_US
}

pub fn embedded_tick_ms() -> u64 {
    TICK_US / US_PER_MS
}

pub fn is_quit_key(byte: u8) -> bool {
    byte == QUIT_KEY || byte == QUIT_KEY_UPPER || byte == QUIT_KEY_ESC
}

pub fn map_open_warning(error: i32) -> String {
    format!(
        "Could not open shared telemetry memory for test mode (error {error}) - test sequence will still drive local devices, but external tools won't see it"
    )
}

pub fn light_script(label: &str) -> Vec<String> {
    let subject = TestSubject::lights(DeviceKind::Usb);
    let options = RunOptions {
        subjects: &[subject],
        device_index: None,
        trace: false,
        publish: false,
    };
    run(&options, &mut |_| false)
        .lines
        .into_iter()
        .filter(|line| line.starts_with(STEP_PREFIX))
        .skip(1)
        .map(|line| line.replace(LABEL_USB_LIGHTS, label))
        .collect()
}

pub fn plan(
    config: Option<&CargopitConfig>,
    config_index: i32,
    device_index: i32,
    disable_audio: bool,
) -> Plan {
    let Some(config) = config else {
        return Plan::Empty;
    };
    let Some(index) = profile_index(config.profiles.len(), config_index) else {
        return Plan::Empty;
    };
    let devices = &config.profiles[index].devices;
    if device_index >= DEVICE_INDEX_SELECT {
        let Some(entry) = devices.get(device_index as usize) else {
            return Plan::MissingIndex;
        };
        return Plan::Ready {
            subjects: vec![subject_from(entry, disable_audio, true)],
            device_index: Some(device_index as u32),
        };
    }
    let subjects = devices
        .iter()
        .map(|entry| subject_from(entry, disable_audio, false))
        .collect();
    Plan::Ready {
        subjects,
        device_index: None,
    }
}

pub fn run<F>(options: &RunOptions<'_>, halt: &mut F) -> Run
where
    F: FnMut(u64) -> bool,
{
    run_frames(options, halt, &mut |_frame: &[u8]| {})
}

pub fn run_frames<F, P>(options: &RunOptions<'_>, halt: &mut F, on_frame: &mut P) -> Run
where
    F: FnMut(u64) -> bool,
    P: FnMut(&[u8]),
{
    let mut runner = Runner::new(options.trace, options.publish, halt, on_frame);
    set_identity(&mut runner.frame);
    runner.publish_frame(false);
    if let Some(index) = options.device_index {
        runner.lines.push(format!("{MSG_DEVICE_INDEX} {index}"));
    }
    runner.lines.push(preparing(options.subjects.len()));
    for subject in options.subjects {
        if runner.halted() {
            break;
        }
        runner.run_one(subject);
    }
    runner.finish();
    runner.into_run()
}

fn subject_from(entry: &DeviceEntry, disable_audio: bool, force_enabled: bool) -> TestSubject {
    let kind = devices::entry_kind(entry);
    let effect = devices::device_effect(entry);
    let enabled = force_enabled || entry.get_bool(keys::KEY_ENABLED) != Some(false);
    let muted = disable_audio && kind == DeviceKind::Sound;
    let effect_id = effect.unwrap_or(EFFECT_ENGINE);
    TestSubject {
        kind,
        effect,
        active: enabled && !muted,
        threshold: tyre_threshold(entry, effect_id),
        tyre: tyre_id_value(entry, effect_id),
    }
}

fn tyre_threshold(entry: &DeviceEntry, effect: i32) -> f64 {
    if !effect_uses_tyre(effect) {
        return THRESHOLD_DEFAULT;
    }
    entry.get_f64(KEY_THRESHOLD).unwrap_or(THRESHOLD_DEFAULT)
}

fn tyre_id_value(entry: &DeviceEntry, effect: i32) -> i32 {
    if !effect_uses_tyre(effect) {
        return TYRE_ALL_FOUR;
    }
    let Some(name) = entry.get_str(KEY_TYRE) else {
        return TYRE_FRONT_LEFT;
    };
    names::tyre_or_all_four(name)
}

fn effect_uses_tyre(effect: i32) -> bool {
    effect == EFFECT_TYRE_SLIP
        || effect == EFFECT_TYRE_LOCK
        || effect == EFFECT_ABS
        || effect == EFFECT_SUSPENSION
}

struct Runner<'a, F, P> {
    frame: Telemetry,
    lines: Vec<String>,
    trace: Vec<TickView>,
    scratch: Vec<u8>,
    tick_count: u64,
    stopped: bool,
    record_trace: bool,
    publish: bool,
    halt: &'a mut F,
    on_frame: &'a mut P,
}

impl<'a, F, P> Runner<'a, F, P>
where
    F: FnMut(u64) -> bool,
    P: FnMut(&[u8]),
{
    fn new(record_trace: bool, publish: bool, halt: &'a mut F, on_frame: &'a mut P) -> Self {
        let frame = Telemetry::new();
        let scratch = vec![0u8; frame.byte_len()];
        Self {
            frame,
            lines: Vec::new(),
            trace: Vec::new(),
            scratch,
            tick_count: 0,
            stopped: false,
            record_trace,
            publish,
            halt,
            on_frame,
        }
    }

    fn into_run(self) -> Run {
        Run {
            lines: self.lines,
            tick_count: self.tick_count,
            stopped: self.stopped,
            simon: self.frame.simon(),
            simstatus: self.frame.simstatus(),
            velocity: self.frame.velocity(),
            rpms: self.frame.rpms(),
            gear: self.frame.gear(),
            flag: self.frame.player_flag(),
            car: self.frame.car(),
            mtick: self.frame.mtick(),
            trace: self.trace,
        }
    }

    fn halted(&mut self) -> bool {
        if self.stopped {
            return true;
        }
        if (self.halt)(self.tick_count) {
            self.stopped = true;
            return true;
        }
        false
    }

    fn announce(&mut self, message: &str) {
        self.lines.push(step(message));
    }

    fn announce_named(&mut self, action: &str, label: &str) {
        self.lines.push(named(action, label));
    }

    fn publish_frame(&mut self, bump_mtick: bool) {
        if !self.publish {
            return;
        }
        if bump_mtick {
            self.frame.set_mtick(self.frame.mtick().wrapping_add(1));
        }
        if !self.frame.copy_into(&mut self.scratch) {
            return;
        }
        (self.on_frame)(&self.scratch);
    }

    fn record(&mut self) {
        self.tick_count = self.tick_count.saturating_add(1);
        self.publish_frame(true);
        if !self.record_trace {
            return;
        }
        let gear_char = self.frame.gearc().bytes().next().unwrap_or(0);
        self.trace.push(TickView {
            rpms: self.frame.rpms(),
            velocity: self.frame.velocity(),
            gear: self.frame.gear(),
            gear_char,
            slip: self.frame.tyre_slip(0),
            flag: self.frame.player_flag(),
            susp: self.frame.susp_velocity(0),
            abs_value: self.frame.abs(),
            diameter_fl: self.frame.tyre_diameter(0),
            simon: self.frame.simon(),
        });
    }

    fn drive(&mut self, ticks: u64, modifier: Mod) {
        for tick in 0..ticks {
            if self.halted() {
                return;
            }
            apply_mod(&mut self.frame, modifier, tick);
            self.record();
        }
    }

    fn phase(&mut self, message: &str, hold_us: u64, modifier: Mod) {
        if self.halted() {
            return;
        }
        self.announce(message);
        self.drive(tick_count(hold_us), modifier);
    }

    fn sweep(&mut self, from: u32, to: u32) {
        if self.stopped {
            return;
        }
        let mut signed = RPM_SWEEP_STEP as i32;
        if to < from {
            signed = -signed;
        }
        let mut rpm = from;
        loop {
            if self.halted() {
                return;
            }
            self.frame.set_rpms(rpm);
            self.record();
            if rpm == to {
                return;
            }
            rpm = next_rpm(rpm, to, signed);
        }
    }

    fn run_one(&mut self, subject: &TestSubject) {
        if !subject.active || self.halted() {
            return;
        }
        let label = device_label(subject);
        self.announce_named(MSG_STARTING, label);
        set_basic(&mut self.frame);
        if is_haptic(subject) {
            self.run_haptic(subject);
        } else {
            self.run_lights();
        }
        if self.halted() {
            return;
        }
        reset_idle(&mut self.frame);
        self.phase(MSG_COAST, IDLE_HOLD_US, Mod::None);
        self.announce_named(MSG_FINISHED, label);
    }

    fn run_haptic(&mut self, subject: &TestSubject) {
        match subject.effect.unwrap_or(EFFECT_ENGINE) {
            EFFECT_ENGINE => self.run_engine(),
            EFFECT_GEAR => self.run_gear(),
            EFFECT_TYRE_SLIP => self.run_spin(subject),
            EFFECT_TYRE_LOCK => self.run_lock(subject),
            EFFECT_ABS => self.run_abs(subject),
            EFFECT_SUSPENSION => self.run_suspension(),
            _ => {}
        }
    }

    fn run_engine(&mut self) {
        self.frame.set_gas(GAS_CRUISE);
        self.announce(MSG_REV);
        self.sweep(RPM_IDLE, RPM_MAX);
        self.sweep(RPM_MAX, RPM_IDLE);
        self.frame.set_rpms(RPM_IDLE);
        self.phase(MSG_RPM_IDLE, PHASE_HOLD_US, Mod::None);
    }

    fn run_gear(&mut self) {
        self.phase_gear(MSG_FIRST, GEAR_FIRST, GEAR_CHAR_FIRST);
        self.phase_gear(MSG_SECOND, GEAR_SECOND, GEAR_CHAR_SECOND);
        self.phase_gear(MSG_THIRD, GEAR_THIRD, GEAR_CHAR_THIRD);
        self.phase_gear(MSG_FOURTH, GEAR_FOURTH, GEAR_CHAR_FOURTH);
    }

    fn phase_gear(&mut self, message: &str, gear: u32, ch: char) {
        set_gear(&mut self.frame, gear, ch);
        self.phase(message, PHASE_HOLD_US, Mod::Gear { gear, ch });
    }

    fn run_spin(&mut self, subject: &TestSubject) {
        set_wheel_spin(&mut self.frame);
        self.lines.push(slip_line(subject, &self.frame));
        self.phase(MSG_SPIN, PHASE_HOLD_US, Mod::None);
    }

    fn run_lock(&mut self, subject: &TestSubject) {
        set_wheel_lock(&mut self.frame);
        self.lines.push(slip_line(subject, &self.frame));
        self.phase(MSG_LOCK, PHASE_HOLD_US, Mod::None);
    }

    fn run_abs(&mut self, subject: &TestSubject) {
        set_abs_phase(&mut self.frame);
        self.lines.push(slip_line(subject, &self.frame));
        self.phase(MSG_ABS, PHASE_HOLD_US, Mod::Abs);
    }

    fn run_suspension(&mut self) {
        self.frame.set_gas(GAS_CRUISE);
        fill_susp(&mut self.frame, SUSP_VEL_A);
        self.phase(MSG_SUSPENSION, PHASE_HOLD_US, Mod::Suspension);
    }

    fn run_lights(&mut self) {
        self.frame.set_gas(GAS_CRUISE);
        self.announce(MSG_REV);
        self.sweep(RPM_IDLE, RPM_MAX);
        self.sweep(RPM_MAX, RPM_IDLE);
        self.frame.set_rpms(RPM_IDLE);
        self.phase(MSG_RPM_IDLE, PHASE_HOLD_US, Mod::None);
        self.frame.set_player_flag(FLAG_GREEN);
        self.phase(MSG_GREEN, PHASE_HOLD_US, Mod::None);
        self.frame.set_player_flag(FLAG_YELLOW);
        self.phase(MSG_YELLOW, PHASE_HOLD_US, Mod::None);
        self.frame.set_player_flag(FLAG_BLUE);
        self.phase(MSG_BLUE, PHASE_HOLD_US, Mod::None);
        self.frame.set_player_flag(FLAG_RED);
        self.phase(MSG_RED, PHASE_HOLD_US, Mod::None);
        set_brake_heat(&mut self.frame);
        self.phase(MSG_BUTTONS, PHASE_HOLD_US, Mod::None);
        self.frame.set_velocity(VELOCITY_SLOW);
        self.phase(MSG_SPEED_SLOW, PHASE_HOLD_US, Mod::None);
        self.frame.set_velocity(VELOCITY_FAST);
        self.phase(MSG_SPEED_FAST, PHASE_HOLD_US, Mod::None);
        self.frame.set_velocity(VELOCITY_TOP);
        self.phase(MSG_SPEED_TOP, PHASE_HOLD_US, Mod::None);
    }

    fn finish(&mut self) {
        if self.stopped {
            self.announce(MSG_STOPPED);
            reset_idle(&mut self.frame);
            self.record();
        }
        self.frame.set_simon(false);
        self.frame.set_simstatus(STATUS_OFF);
        self.record();
    }
}

fn set_identity(frame: &mut Telemetry) {
    frame.set_simon(true);
    frame.set_simstatus(games::STATUS_ACTIVE_PLAY as u32);
    frame.set_simapi(SIMAPI_TEST);
    frame.set_simexe(SIMEXE_TEST_NONE);
    frame.set_simapiversion(SIMAPI_VERSION);
}

fn set_basic(frame: &mut Telemetry) {
    frame.set_car(CAR_NAME);
    set_gear(frame, GEAR_NEUTRAL, GEAR_CHAR_NEUTRAL);
    frame.set_velocity(VELOCITY_CRUISE);
    frame.set_rpms(RPM_IDLE);
    frame.set_maxrpm(RPM_MAX);
    frame.set_idlerpm(RPM_IDLE);
    clear_effects(frame);
    frame.set_x_velocity(AXIS_IDLE);
    frame.set_y_velocity(Y_VELOCITY);
    frame.set_z_velocity(AXIS_IDLE);
}

fn clear_effects(frame: &mut Telemetry) {
    frame.set_gas(AXIS_IDLE);
    frame.set_brake(AXIS_IDLE);
    frame.set_abs(ABS_OFF);
    fill_slip(frame, SLIP_CLEAR);
    fill_brake_temp(frame, BRAKE_TEMP_COLD);
    fill_susp(frame, AXIS_IDLE);
    fill_rps(frame, TYRE_RPS_CLEAR);
    fill_diameter(frame, TYRE_DIAMETER_UNSET);
}

fn reset_idle(frame: &mut Telemetry) {
    clear_effects(frame);
    frame.set_velocity(VELOCITY_IDLE);
    frame.set_rpms(RPM_IDLE);
    frame.set_player_flag(FLAG_GREEN);
    set_gear(frame, GEAR_NEUTRAL, GEAR_CHAR_NEUTRAL);
}

fn set_rolling(frame: &mut Telemetry) {
    frame.set_y_velocity(Y_VELOCITY);
    frame.set_z_velocity(AXIS_IDLE);
}

fn set_wheel_spin(frame: &mut Telemetry) {
    set_rolling(frame);
    frame.set_velocity(VELOCITY_SPIN);
    frame.set_gas(PEDAL_APPLIED);
    frame.set_brake(AXIS_IDLE);
    frame.set_abs(ABS_OFF);
    fill_slip(frame, SLIP_SPIN);
    fill_brake_temp(frame, BRAKE_TEMP_COLD);
    set_tyre_fallback(frame, TYRE_RPS_SPIN);
}

fn set_wheel_lock(frame: &mut Telemetry) {
    set_rolling(frame);
    frame.set_velocity(VELOCITY_LOCK);
    frame.set_gas(AXIS_IDLE);
    frame.set_brake(PEDAL_APPLIED);
    frame.set_abs(ABS_OFF);
    fill_slip(frame, SLIP_LOCK);
    fill_brake_temp(frame, BRAKE_TEMP_HOT);
    set_tyre_fallback(frame, TYRE_RPS_LOCK);
}

fn set_brake_heat(frame: &mut Telemetry) {
    set_rolling(frame);
    frame.set_velocity(VELOCITY_LOCK);
    frame.set_gas(AXIS_IDLE);
    frame.set_brake(PEDAL_APPLIED);
    frame.set_abs(ABS_OFF);
    fill_slip(frame, SLIP_CLEAR);
    fill_brake_temp(frame, BRAKE_TEMP_HOT);
    fill_rps(frame, TYRE_RPS_CLEAR);
    fill_diameter(frame, TYRE_DIAMETER_UNSET);
}

fn set_abs_phase(frame: &mut Telemetry) {
    set_rolling(frame);
    frame.set_velocity(VELOCITY_LOCK);
    frame.set_gas(AXIS_IDLE);
    frame.set_brake(PEDAL_APPLIED);
    frame.set_abs(ABS_ACTIVE);
    fill_slip(frame, SLIP_ABS_A);
    fill_brake_temp(frame, BRAKE_TEMP_HOT);
    set_tyre_fallback(frame, TYRE_RPS_LOCK);
}

fn set_tyre_fallback(frame: &mut Telemetry, rps: f64) {
    fill_rps(frame, rps);
    frame.set_tyre_diameter(0, TYRE_DIAMETER_FL);
    frame.set_tyre_diameter(1, TYRE_DIAMETER_FR);
    frame.set_tyre_diameter(2, TYRE_DIAMETER_RL);
    frame.set_tyre_diameter(3, TYRE_DIAMETER_RR);
}

fn set_gear(frame: &mut Telemetry, gear: u32, ch: char) {
    frame.set_gear(gear);
    let mut bytes = [0u8; 4];
    frame.set_gearc(ch.encode_utf8(&mut bytes));
}

fn fill_slip(frame: &mut Telemetry, value: f64) {
    for index in 0..WHEEL_COUNT {
        frame.set_tyre_slip(index, value);
    }
}

fn fill_brake_temp(frame: &mut Telemetry, value: f64) {
    for index in 0..WHEEL_COUNT {
        frame.set_brake_temp(index, value);
    }
}

fn fill_susp(frame: &mut Telemetry, value: f64) {
    for index in 0..WHEEL_COUNT {
        frame.set_susp_velocity(index, value);
    }
}

fn fill_rps(frame: &mut Telemetry, value: f64) {
    for index in 0..WHEEL_COUNT {
        frame.set_tyre_rps(index, value);
    }
}

fn fill_diameter(frame: &mut Telemetry, value: f64) {
    for index in 0..WHEEL_COUNT {
        frame.set_tyre_diameter(index, value);
    }
}

fn apply_mod(frame: &mut Telemetry, modifier: Mod, tick: u64) {
    match modifier {
        Mod::None => {}
        Mod::Gear { gear, ch } => apply_gear(frame, gear, ch, tick),
        Mod::Abs => apply_abs(frame, tick),
        Mod::Suspension => apply_suspension(frame, tick),
    }
}

fn apply_gear(frame: &mut Telemetry, gear: u32, ch: char, tick: u64) {
    if (tick / GEAR_PULSE_TICKS).is_multiple_of(2) {
        set_gear(frame, gear, ch);
        return;
    }
    set_gear(frame, GEAR_NEUTRAL, GEAR_CHAR_NEUTRAL);
}

fn apply_abs(frame: &mut Telemetry, tick: u64) {
    let mut slip = SLIP_ABS_A;
    if !(tick / ABS_PULSE_TICKS).is_multiple_of(2) {
        slip = SLIP_ABS_B;
    }
    fill_slip(frame, slip);
}

fn apply_suspension(frame: &mut Telemetry, tick: u64) {
    let mut vel = SUSP_VEL_A;
    if !tick.is_multiple_of(2) {
        vel = SUSP_VEL_B;
    }
    fill_susp(frame, vel);
}

fn next_rpm(rpm: u32, to: u32, signed_step: i32) -> u32 {
    let next = rpm as i32 + signed_step;
    if signed_step > 0 && next >= to as i32 {
        return to;
    }
    if signed_step < 0 && next <= to as i32 {
        return to;
    }
    next as u32
}

fn is_haptic(subject: &TestSubject) -> bool {
    if subject.kind == DeviceKind::Sound {
        return true;
    }
    matches!(
        subject.effect,
        Some(effect) if effect_uses_named_haptic(effect)
    )
}

fn effect_uses_named_haptic(effect: i32) -> bool {
    effect == EFFECT_GEAR
        || effect == EFFECT_TYRE_LOCK
        || effect == EFFECT_TYRE_SLIP
        || effect == EFFECT_ABS
        || effect == EFFECT_SUSPENSION
}

fn device_label(subject: &TestSubject) -> &'static str {
    if is_haptic(subject) {
        return effect_label(subject.effect.unwrap_or(EFFECT_ENGINE));
    }
    if subject.kind == DeviceKind::Serial {
        return LABEL_SERIAL_LIGHTS;
    }
    if subject.kind == DeviceKind::Usb {
        return LABEL_USB_LIGHTS;
    }
    LABEL_DEVICE
}

fn effect_label(effect: i32) -> &'static str {
    if effect == EFFECT_GEAR {
        return LABEL_GEAR;
    }
    if effect == EFFECT_TYRE_LOCK {
        return LABEL_TYRE_LOCK;
    }
    if effect == EFFECT_ABS {
        return LABEL_ABS;
    }
    if effect == EFFECT_TYRE_SLIP {
        return LABEL_TYRE_SLIP;
    }
    if effect == EFFECT_SUSPENSION {
        return LABEL_SUSPENSION;
    }
    LABEL_EFFECT
}

fn slip_line(subject: &TestSubject, frame: &Telemetry) -> String {
    let effect = subject.effect.unwrap_or(EFFECT_ENGINE);
    let play = slip_play(subject, frame);
    format!(
        "{STEP_PREFIX}{} play={} threshold={} brake={} gas={} yvel={} slip={}",
        effect_label(effect),
        c_float(play),
        c_float(subject.threshold),
        c_float(frame.brake()),
        c_float(frame.gas()),
        c_float(frame.y_velocity()),
        c_float(frame.tyre_slip(0)),
    )
}

fn slip_play(subject: &TestSubject, frame: &Telemetry) -> f64 {
    let Some(effect) = vibration_effect(subject.effect.unwrap_or(EFFECT_ENGINE)) else {
        return 0.0;
    };
    let settings = HapticSettings {
        effect,
        tyre: tyre_id(subject.tyre),
        threshold: subject.threshold,
        ..HapticSettings::default()
    };
    let mut probe = HapticEffect::new(&settings);
    probe.play_with_clock(frame, &VirtualClock::new())
}

fn vibration_effect(effect: i32) -> Option<VibrationEffect> {
    if effect == EFFECT_ENGINE {
        return Some(VibrationEffect::EngineRpm);
    }
    if effect == EFFECT_GEAR {
        return Some(VibrationEffect::GearShift);
    }
    if effect == EFFECT_ABS {
        return Some(VibrationEffect::AbsBrakes);
    }
    if effect == EFFECT_TYRE_SLIP {
        return Some(VibrationEffect::TyreSlip);
    }
    if effect == EFFECT_TYRE_LOCK {
        return Some(VibrationEffect::TyreLock);
    }
    if effect == EFFECT_SUSPENSION {
        return Some(VibrationEffect::Suspension);
    }
    None
}

fn tyre_id(value: i32) -> TyreId {
    if value == TYRE_FRONT_LEFT {
        return TyreId::FrontLeft;
    }
    if value == TYRE_FRONT_RIGHT {
        return TyreId::FrontRight;
    }
    if value == TYRE_REAR_LEFT {
        return TyreId::RearLeft;
    }
    if value == TYRE_REAR_RIGHT {
        return TyreId::RearRight;
    }
    if value == TYRE_FRONTS {
        return TyreId::Fronts;
    }
    if value == TYRE_REARS {
        return TyreId::Rears;
    }
    TyreId::AllFour
}

fn c_float(value: f64) -> String {
    format!("{value:.precision$}", precision = C_PRINTF_F_PRECISION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargopit_config::config::{DeviceEntry, SimProfile};

    fn options(subjects: &[TestSubject]) -> RunOptions<'_> {
        RunOptions {
            subjects,
            device_index: None,
            trace: true,
            publish: false,
        }
    }

    fn frame_from(bytes: &[u8]) -> Telemetry {
        let buf = simapi_sys::SimDataBuf::from_bytes(bytes).expect("frame");
        Telemetry::from_buf(buf)
    }

    fn sweep_ticks() -> u64 {
        let span = u64::from(RPM_MAX - RPM_IDLE) / u64::from(RPM_SWEEP_STEP) + 1;
        span * 2
    }

    fn phase_ticks() -> u64 {
        tick_count(PHASE_HOLD_US)
    }

    fn coast_ticks() -> u64 {
        tick_count(IDLE_HOLD_US)
    }

    #[test]
    fn announcements_use_the_c_prefix_and_sixteen_millisecond_tick() {
        assert_eq!(embedded_tick_ms(), 16);
        assert_eq!(tick_count(TICK_US), 1);
        assert_eq!(preparing(0), "test step: preparing test with 0 devices...");
        let lines = light_script(LABEL_USB_LIGHTS);
        assert!(lines[0].starts_with(STEP_PREFIX));
        assert!(lines.iter().any(|line| line.contains(MSG_REV)));
        assert!(lines.iter().any(|line| line.contains(MSG_GREEN)));
        assert!(lines.iter().any(|line| line.contains(MSG_SPEED_TOP)));
        assert!(is_quit_key(QUIT_KEY));
        assert!(is_quit_key(QUIT_KEY_UPPER));
        assert!(is_quit_key(QUIT_KEY_ESC));
        assert!(!is_quit_key(b'a'));
    }

    #[test]
    fn usb_lights_cover_flags_speeds_and_the_rpm_sweep() {
        let subject = TestSubject::lights(DeviceKind::Usb);
        let run = run(&options(&[subject]), &mut |_| false);
        assert!(run.lines[0].contains("1 devices"));
        assert!(run
            .lines
            .iter()
            .any(|line| line == &named(MSG_STARTING, LABEL_USB_LIGHTS)));
        assert_eq!(
            run.tick_count,
            sweep_ticks() + phase_ticks() * 9 + coast_ticks() + 1
        );
        assert!(run.trace.iter().any(|tick| tick.rpms == RPM_MAX));
        assert!(run.trace.iter().any(|tick| tick.velocity == VELOCITY_SLOW));
        assert!(run.trace.iter().any(|tick| tick.velocity == VELOCITY_FAST));
        assert!(run.trace.iter().any(|tick| tick.velocity == VELOCITY_TOP));
        assert!(run.trace.iter().any(|tick| tick.flag == FLAG_YELLOW));
        assert!(run.trace.iter().any(|tick| tick.flag == FLAG_BLUE));
        assert!(run.trace.iter().any(|tick| tick.flag == FLAG_RED));
        assert_eq!(run.car, CAR_NAME);
        assert!(!run.simon);
        assert_eq!(run.simstatus, STATUS_OFF);
        assert_eq!(run.velocity, VELOCITY_IDLE);
        assert_eq!(run.rpms, RPM_IDLE);
        assert!(run.trace.iter().any(|tick| tick.simon));
    }

    #[test]
    fn engine_on_sound_is_labeled_effect_and_usb_engine_is_lights() {
        let sound = TestSubject::haptic(DeviceKind::Sound, EFFECT_ENGINE);
        let sound_run = run(&options(&[sound]), &mut |_| false);
        assert!(sound_run
            .lines
            .iter()
            .any(|line| line == &named(MSG_STARTING, LABEL_EFFECT)));
        assert!(!sound_run.lines.iter().any(|line| line.contains(MSG_GREEN)));
        assert_eq!(
            sound_run.tick_count,
            sweep_ticks() + phase_ticks() + coast_ticks() + 1
        );

        let usb = TestSubject::haptic(DeviceKind::Usb, EFFECT_ENGINE);
        let usb_run = run(&options(&[usb]), &mut |_| false);
        assert!(usb_run
            .lines
            .iter()
            .any(|line| line == &named(MSG_STARTING, LABEL_USB_LIGHTS)));
        assert!(usb_run.lines.iter().any(|line| line.contains(MSG_RED)));
    }

    #[test]
    fn gear_pulses_back_to_neutral_every_eight_ticks() {
        let subject = TestSubject::haptic(DeviceKind::Usb, EFFECT_GEAR);
        let run = run(&options(&[subject]), &mut |_| false);
        assert!(run.lines.iter().any(|line| line.contains(MSG_FIRST)));
        assert!(run.lines.iter().any(|line| line.contains(MSG_FOURTH)));
        assert!(!run.lines.iter().any(|line| line.contains(MSG_REV)));
        assert_eq!(run.trace[0].gear, GEAR_FIRST);
        assert_eq!(run.trace[0].gear_char, b'1');
        assert_eq!(run.trace[GEAR_PULSE_TICKS as usize].gear, GEAR_NEUTRAL);
        assert_eq!(run.trace[GEAR_PULSE_TICKS as usize].gear_char, b'N');
    }

    #[test]
    fn spin_logs_front_left_play_and_sets_the_fallback_diameter() {
        let subject = TestSubject::haptic(DeviceKind::Sound, EFFECT_TYRE_SLIP);
        let run = run(&options(&[subject]), &mut |_| false);
        let play = run
            .lines
            .iter()
            .find(|line| line.contains("play="))
            .unwrap();
        assert!(play.starts_with(&format!("{STEP_PREFIX}{LABEL_TYRE_SLIP} play=")));
        assert!(play.contains("play=0.450000"));
        assert!(play.contains("slip=-0.450000"));
        assert!(play.contains("gas=0.850000"));
        assert!(play.contains("yvel=100.000000"));
        assert_eq!(run.trace[0].slip, SLIP_SPIN);
        assert_eq!(run.trace[0].diameter_fl, TYRE_DIAMETER_FL);
        assert_eq!(run.trace[0].velocity, VELOCITY_SPIN);
    }

    #[test]
    fn abs_slip_alternates_and_suspension_velocity_toggles() {
        let abs = TestSubject::haptic(DeviceKind::Serial, EFFECT_ABS);
        let abs_run = run(&options(&[abs]), &mut |_| false);
        assert!(abs_run
            .lines
            .iter()
            .any(|line| line.contains("slip=0.400000")));
        assert_eq!(abs_run.trace[0].slip, SLIP_ABS_A);
        assert_eq!(abs_run.trace[ABS_PULSE_TICKS as usize].slip, SLIP_ABS_B);
        assert_eq!(abs_run.trace[0].abs_value, ABS_ACTIVE);

        let susp = TestSubject::haptic(DeviceKind::Usb, EFFECT_SUSPENSION);
        let susp_run = run(&options(&[susp]), &mut |_| false);
        assert!(susp_run
            .lines
            .iter()
            .any(|line| line.contains(MSG_SUSPENSION)));
        assert_eq!(susp_run.trace[0].susp, SUSP_VEL_A);
        assert_eq!(susp_run.trace[1].susp, SUSP_VEL_B);
    }

    #[test]
    fn quit_during_the_sweep_stops_and_idles() {
        let subject = TestSubject::lights(DeviceKind::Serial);
        let run = run(&options(&[subject]), &mut |ticks| ticks >= 4);
        assert!(run.stopped);
        assert!(run.lines.iter().any(|line| line.contains(MSG_STOPPED)));
        assert!(run.lines.iter().any(|line| line.contains(MSG_REV)));
        assert!(!run.lines.iter().any(|line| line.contains(MSG_FINISHED)));
        assert_eq!(run.velocity, VELOCITY_IDLE);
        assert_eq!(run.rpms, RPM_IDLE);
        assert_eq!(run.flag, FLAG_GREEN);
        assert_eq!(run.gear, GEAR_NEUTRAL);
        assert!(!run.simon);
        assert!(!run.trace.last().unwrap().simon);
    }

    #[test]
    fn disabled_and_muted_devices_stay_in_the_count_without_a_script() {
        let mut disabled = TestSubject::lights(DeviceKind::Usb);
        disabled.active = false;
        let run = run(&options(&[disabled]), &mut |_| false);
        assert_eq!(run.lines, vec![preparing(1)]);
        assert!(!run.stopped);
        assert_eq!(run.tick_count, 1);
        assert_eq!(run.mtick, 0);
    }

    #[test]
    fn publish_copies_each_frame_and_leaves_mtick_at_zero_when_closed() {
        const SHM_OPEN_FAILED: i32 = 10;
        const MAP_OPEN_WARNING: &str = "Could not open shared telemetry memory for test mode (error 10) - test sequence will still drive local devices, but external tools won't see it";
        let mut disabled = TestSubject::lights(DeviceKind::Usb);
        disabled.active = false;
        let subjects = [disabled];
        let mut opts = options(&subjects);
        opts.publish = false;
        let mut closed_calls = 0u32;
        let closed = run_frames(&opts, &mut |_| false, &mut |_frame: &[u8]| {
            closed_calls = closed_calls.saturating_add(1);
        });
        assert_eq!(closed_calls, 0);
        assert_eq!(closed.mtick, 0);

        opts.publish = true;
        let mut frames = Vec::new();
        let run = run_frames(&opts, &mut |_| false, &mut |frame| {
            frames.push(frame.to_vec());
        });
        assert_eq!(run.tick_count, 1);
        assert_eq!(run.mtick, 1);
        assert_eq!(frames.len(), 2);
        let open = frame_from(&frames[0]);
        let close = frame_from(&frames[1]);
        assert_eq!(open.mtick(), 0);
        assert!(open.simon());
        assert_eq!(open.simstatus(), games::STATUS_ACTIVE_PLAY as u32);
        assert_eq!(close.mtick(), 1);
        assert!(!close.simon());
        assert_eq!(close.simstatus(), STATUS_OFF);
        assert_eq!(run.simstatus, STATUS_OFF);
        assert_eq!(map_open_warning(SHM_OPEN_FAILED), MAP_OPEN_WARNING);
    }

    #[test]
    fn plan_selects_one_config_index_and_reads_tyre_settings() {
        let mut slip = DeviceEntry::new();
        slip.set_str(keys::KEY_DEVICE, "Sound");
        slip.set_str(keys::KEY_TYPE, "Haptic");
        slip.set_str(keys::KEY_EFFECT, "TyreSlip");
        slip.set_str(KEY_TYRE, "All");
        slip.set_float(KEY_THRESHOLD, 0.2);
        slip.set_bool(keys::KEY_ENABLED, true);
        let mut usb = DeviceEntry::new();
        usb.set_str(keys::KEY_DEVICE, "USB");
        usb.set_str(keys::KEY_TYPE, "Tachometer");
        usb.set_bool(keys::KEY_ENABLED, false);
        let config = CargopitConfig {
            profiles: vec![SimProfile {
                devices: vec![slip, usb],
                ..SimProfile::default()
            }],
            extra: Vec::new(),
        };
        let Plan::Ready {
            subjects,
            device_index,
        } = plan(Some(&config), -1, -1, false)
        else {
            panic!("profile");
        };
        assert!(device_index.is_none());
        assert_eq!(subjects.len(), 2);
        assert!(subjects[0].active);
        assert_eq!(subjects[0].kind, DeviceKind::Sound);
        assert_eq!(subjects[1].kind, DeviceKind::Usb);
        assert_eq!(subjects[0].tyre, TYRE_ALL_FOUR);
        assert_eq!(subjects[0].threshold, 0.2);
        assert!(!subjects[1].active);
        let slip_run = run(&options(&subjects[..1]), &mut |_| false);
        let play = slip_run
            .lines
            .iter()
            .find(|line| line.contains("play="))
            .unwrap();
        assert!(play.contains("play=1.000000"));
        assert!(play.contains("threshold=0.200000"));
        let Plan::Ready { subjects, .. } = plan(Some(&config), -1, -1, true) else {
            panic!("muted");
        };
        assert!(!subjects[0].active);

        let Plan::Ready {
            subjects,
            device_index,
        } = plan(Some(&config), -1, 1, false)
        else {
            panic!("index");
        };
        assert_eq!(device_index, Some(1));
        assert!(subjects[0].active);
        assert!(matches!(
            plan(Some(&config), -1, 4, false),
            Plan::MissingIndex
        ));
        assert!(matches!(plan(None, -1, -1, false), Plan::Empty));
    }
}
