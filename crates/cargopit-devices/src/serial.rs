//! Serial Arduino and Moza encoders. Output matches the C capture goldens.

use std::cell::{Cell, RefCell};

use crate::clock::{Clock, VirtualClock};
use crate::haptic::{HapticEffect, HapticSettings, TyreId, VibrationEffect};
use crate::lua_host::{LuaHost, LuaLedMode};
use crate::telemetry::Telemetry;

const ORIGIN_US: u64 = 1_000_000;
const TICK_US: u64 = 16_000;
const CLOCK_MONOTONIC: i32 = 1;
const NS_PER_MS: u64 = 1_000_000;
const NS_PER_US: u64 = 1_000;
const CAPTURE_PORT: &str = "/dev/ttyPARITY0";
const BAUD_DEFAULT: i32 = 115_200;
const BAUD_FIRST_OPEN: i32 = 9_600;
const BAUD_R9_FLOOR: i32 = 115_200;
const SHIFT_LIGHTS: i32 = 8;
const SIMLED_COUNT: i32 = 8;
pub const LED_FIRST: i32 = 1;
const SIMLED_END_ALL: i32 = 0;
const FAN_POWER: f64 = 0.5;
const AMP_FACTOR: f64 = 1.0;
const KPH_TO_MPH: f64 = 0.621317;
const FAN_BYTE_SCALE: f64 = 255.0;
const EFFECT_BYTE_SCALE: f64 = 255.0;
const PLAY_LIMIT: f64 = 1.0;
const ARDUINO_TIMEOUT_MS: u32 = 9_000;
pub const SIMLED_QUERY_WAIT_MS: u32 = 5_000;
pub const SIMLED_QUERY_ATTEMPTS: usize = 4;
pub const SIMLED_READ_TIMEOUT_MS: u64 = 10;
pub const SIMLED_READ_CAP: usize = 255;
const HAPTIC_ZERO_TIMEOUT_MS: u32 = 9_000;
const MOZA_TIMEOUT_MS: u32 = 1_000;
const REDLINE_MARGIN: f64 = 0.05;
const HEADER_MARKS: usize = 6;
const MARK_BYTE: u8 = 0xff;
const SLED_TAG: &[u8] = b"sleds";
const LEDSC_TAG: &[u8] = b"ledsc";
const LED_PACKET_TAIL: [u8; 3] = [0xff, 0xfe, 0xfd];
const PACKET_META: usize = HEADER_MARKS + 5 + LED_PACKET_TAIL.len();
const RGB_CHANNELS: usize = 3;
const GREEN_CHANNEL: usize = 1;
const RED_CHANNEL: usize = 0;
pub const SIMLED_COUNT_REPLY: &[u8] = b"8\r";
const C_SPACE: u8 = b' ';
const C_TAB: u8 = b'\t';
const C_LF: u8 = b'\n';
const C_CR: u8 = b'\r';
const C_VT: u8 = 0x0b;
const C_FF: u8 = 0x0c;
const ASCII_PLUS: u8 = b'+';
const ASCII_MINUS: u8 = b'-';
const ASCII_DIGIT_ZERO: u8 = b'0';
const DECIMAL_RADIX: u64 = 10;
const HAPTIC_HZ: u32 = 40;
const HAPTIC_AMP: u32 = 100;
const HAPTIC_THRESHOLD: f64 = 0.2;
const HAPTIC_DURATION_S: f64 = 0.10;
const MOTOR_1: u32 = 0;
const MOTOR_LEFT_FRONT: u32 = 4;
const MOTOR_REAR: u32 = 7;
const MOTOR_FRONT_AXLE: u32 = 8;
const MOTOR_RIGHT_REAR: u32 = 10;
const MOTOR_LEFT_REAR: u32 = 11;
const MOTOR_DIAGONAL: u32 = 13;
const MOTOR_ALL: u32 = 14;
const MOTOR_RIGHT: u32 = 2;
const MOTOR_RIGHT_MID: u32 = 6;
const MOTOR_RIGHT_FRONT: u32 = 9;
const MOTOR_RIGHT_ALL: u32 = 12;
const HAPTIC_MOTORS: usize = 4;
const HAPTIC_MOTOR_STRIDE: usize = 2;
const HAPTIC_EFFECT_OFFSET: usize = 1;
const HAPTIC_PACKET: usize = HAPTIC_MOTORS * HAPTIC_MOTOR_STRIDE;
pub const HAPTIC_PACKET_LEN: i32 = HAPTIC_PACKET as i32;
const HAPTIC_MOTOR_FLAG: u8 = 1;
const HAPTIC_CHANNEL_SLOTS: usize = 2;
const HAPTIC_SLOT_ONE: usize = 0;
const HAPTIC_SLOT_THREE: usize = 1;
const HAPTIC_LOG_MOTOR_ONE: i32 = 1;
const HAPTIC_LOG_MOTOR_THREE: i32 = 3;
const HAPTIC_MOTOR_ONE_INDEX: usize = 0;
const HAPTIC_MOTOR_THREE_INDEX: usize = 2;
const MOZA_MAGIC: u32 = 0x0d;
const MOZA_START: u8 = 0x7e;
const MOZA_R5_TEMPLATE: [u8; 11] = [0x7e, 0x06, 0x41, 0x13, 0xfd, 0xde, 0, 0, 0, 0, 0];
const MOZA_R5_SIZE: usize = 11;
const MOZA_BLINK_BIT: u32 = 7;
const MOZA_PERCENT: f32 = 100.0;
const KS_MASK_TEMPLATE: [u8; 11] = [0x7e, 0x06, 0x3f, 0x17, 0x1a, 0, 0, 0, 0, 0, 0];
const KS_MASK_SIZE: usize = 11;
const KS_COLOR_SIZE: usize = 27;
const FLAG_YELLOW: u8 = 1;
const KS_BLINK_SHIFT: u32 = 7;
const KS_BLINK_PERCENT: i32 = 98;
const STATUS_ACTIVE: u32 = 2;
const MOZA_GROUP: u8 = 0x3f;
const MOZA_DEVICE: u8 = 0x17;
const MOZA_LED_COUNT: usize = 10;
const MOZA_COLOUR_BYTES: usize = 40;
const MOZA_COLOUR_CHUNK: usize = 20;
const MOZA_MAX_FRAME: usize = 64;
const MOZA_BRIGHTNESS: u8 = 100;
const MOZA_RPM_WINDOW: f32 = 0.80;
const MOZA_HYSTERESIS: f32 = 0.4;
const MOZA_ALERT_MS: u64 = 110;
const MOZA_BUTTON_MS: u64 = 80;
const MOZA_BRAKE_SHOW: f32 = 0.04;
const MOZA_BRAKE_HIDE: f32 = 0.02;
const MOZA_LOCK_SLIP: f32 = 0.28;
const MOZA_CLEAR_SLIP: f32 = 0.18;
const MOZA_MIN_BRAKE: f32 = 0.20;
const MOZA_MIN_SPEED_MS: f32 = 4.0;
const MOZA_KM_H_TO_M_S: f32 = 0.277778;
const MOZA_DR2_COLD: f32 = 120.0;
const MOZA_DR2_HOT: f32 = 620.0;
const MOZA_ACR_COLD: f32 = 320.0;
const MOZA_ACR_HOT: f32 = 750.0;
const MOZA_HEAT_OFF: f32 = 0.06;
const MOZA_YELLOW_FULL: f32 = 0.55;
const MOZA_TEMP_NORMAL_MAX: f32 = 1.5;
const MOZA_PLACEHOLDER_SPAN: f64 = 1.5;
const MOZA_CORNERS: usize = 4;
const MOZA_BTN_FL: usize = 1;
const MOZA_BTN_FR: usize = 8;
const MOZA_BTN_RL: usize = 3;
const MOZA_BTN_RR: usize = 6;
const ACR_SIMEXE: u64 = 3_917_090;
const ABS_ON: f64 = 0.5;
const RGB_STRIDE: usize = 4;
const CLOCK_OP: &str = concat!("clock_", "gettime");
const WALL_OP: &str = concat!("get", "timeofday");
const RPM_RGB: [[u8; 3]; MOZA_LED_COUNT] = [
    [0, 255, 0],
    [0, 255, 0],
    [0, 255, 0],
    [0, 255, 0],
    [255, 196, 0],
    [255, 196, 0],
    [255, 196, 0],
    [255, 0, 0],
    [255, 0, 0],
    [255, 0, 0],
];
const PURPLE: [u8; 3] = [180, 0, 255];
const BLUE_ALERT: [u8; 3] = [0, 120, 255];
const BRAKE_YELLOW: [u8; 3] = [255, 220, 0];
const HEAT_RED: [u8; 3] = [255, 0, 0];
const BRAKE_BUTTONS: [usize; MOZA_CORNERS] = [MOZA_BTN_FL, MOZA_BTN_FR, MOZA_BTN_RL, MOZA_BTN_RR];

pub const SERIAL_DEVICES: &[&str] = &[
    "shiftlights",
    "simwind",
    "serial_haptic",
    "simled",
    "simled_custom",
    "arduino_custom",
    "moza_r5",
    "moza_new",
    "moza_ks_pro",
    "shared_serial_port",
];

struct Port {
    name: String,
    reply_ready: bool,
    refs: u32,
}

struct Log {
    tick: Cell<u32>,
    lines: RefCell<String>,
    clock: RefCell<VirtualClock>,
    ports: RefCell<Vec<Option<Port>>>,
}

struct Trace<'a> {
    log: &'a Log,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bar {
    Rpm,
    Brake,
    Lock,
    Abs,
}

#[derive(Clone, Copy)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

struct MozaNew {
    armed: bool,
    braking: bool,
    lock_latched: bool,
    abs_latched: bool,
    has_button_write: bool,
    bar: Bar,
    last_rpm_lit: usize,
    last_brake_lit: usize,
    last_corners: [Rgb; MOZA_CORNERS],
    started_ns: u64,
    last_button_ns: u64,
}

impl Log {
    fn new() -> Self {
        let mut clock = VirtualClock::new();
        clock.advance_us(ORIGIN_US);
        Self {
            tick: Cell::new(0),
            lines: RefCell::new(String::new()),
            clock: RefCell::new(clock),
            ports: RefCell::new(Vec::new()),
        }
    }

    fn text(&self) -> String {
        self.lines.borrow().clone()
    }

    fn set_tick(&self, tick: u32) {
        self.tick.set(tick);
    }

    fn advance_tick(&self) {
        self.clock.borrow_mut().advance_us(TICK_US);
    }

    fn op(&self, name: &str, detail: &str) {
        let ms = self.clock.borrow().monotonic_ms();
        self.lines.borrow_mut().push_str(&format!(
            "tick {} t_ms {ms} {name} {detail}\n",
            self.tick.get()
        ));
    }

    fn bytes(&self, op: &str, data: &[u8]) {
        let mut hex = String::with_capacity(data.len() * 2);
        for byte in data {
            hex.push_str(&format!("{byte:02x}"));
        }
        self.op(op, &hex);
    }

    fn open_port(&self, name: &str, baud: i32) -> usize {
        if let Some(id) = self.find_open(name) {
            self.ports.borrow_mut()[id].as_mut().unwrap().refs += 1;
            return id;
        }
        self.op("sp_get_port_by_name", name);
        self.op("sp_open", &format!("port={name}"));
        self.op("sp_set_baudrate", &format!("port={name} baud={baud}"));
        self.ports.borrow_mut().push(Some(Port {
            name: name.to_string(),
            reply_ready: false,
            refs: 1,
        }));
        self.ports.borrow().len() - 1
    }

    fn find_open(&self, name: &str) -> Option<usize> {
        self.ports.borrow().iter().position(|slot| {
            slot.as_ref()
                .is_some_and(|port| port.refs > 0 && port.name.eq_ignore_ascii_case(name))
        })
    }

    fn write_port(&self, id: usize, data: &[u8]) {
        let name = self.ports.borrow()[id].as_ref().unwrap().name.clone();
        self.bytes(&format!("sp_blocking_write port={name}"), data);
        if data
            .windows(LEDSC_TAG.len())
            .any(|window| window == LEDSC_TAG)
        {
            self.ports.borrow_mut()[id].as_mut().unwrap().reply_ready = true;
        }
    }

    fn wait_port(&self, timeout_ms: u32) {
        self.op("sp_wait", &format!("timeout_ms={timeout_ms}"));
    }

    fn read_port(&self, id: usize) -> Vec<u8> {
        let ready = self.ports.borrow()[id].as_ref().unwrap().reply_ready;
        if !ready {
            self.op("sp_blocking_read", "empty");
            return Vec::new();
        }
        self.ports.borrow_mut()[id].as_mut().unwrap().reply_ready = false;
        self.bytes("sp_blocking_read", SIMLED_COUNT_REPLY);
        SIMLED_COUNT_REPLY.to_vec()
    }

    fn close_port(&self, id: usize) {
        let mut ports = self.ports.borrow_mut();
        let Some(port) = ports[id].as_mut() else {
            return;
        };
        if port.refs == 0 {
            return;
        }
        port.refs -= 1;
        if port.refs > 0 {
            return;
        }
        let name = port.name.clone();
        *ports[id].as_mut().unwrap() = Port {
            name: name.clone(),
            reply_ready: false,
            refs: 0,
        };
        drop(ports);
        self.op("sp_close", &name);
        self.op("sp_free_port", &name);
    }
}

impl Clock for Trace<'_> {
    fn monotonic_ms(&self) -> u64 {
        self.log.clock.borrow().monotonic_ms()
    }

    fn monotonic_us(&self) -> u64 {
        self.log.clock.borrow().monotonic_us()
    }

    fn monotonic_ns(&self) -> u64 {
        let ns = self.log.clock.borrow().monotonic_ns();
        self.log
            .op(CLOCK_OP, &format!("clk={CLOCK_MONOTONIC} ns={ns}"));
        ns
    }

    fn wall_ms(&self) -> u64 {
        self.log.op(WALL_OP, "virtual");
        self.log.clock.borrow().wall_ms()
    }
}

pub fn capture_serial(name: &str, frames: &[Telemetry], lua_source: &str) -> Option<String> {
    let log = Log::new();
    match name {
        "shiftlights" => {
            let id = run_shiftlights(&log, frames, BAUD_DEFAULT);
            log.close_port(id);
        }
        "simwind" => {
            let id = run_simwind(&log, frames, BAUD_DEFAULT);
            log.close_port(id);
        }
        "serial_haptic" => run_haptic(&log, frames),
        "simled" => run_simled(&log, frames),
        "simled_custom" => run_simled_custom(&log, frames, lua_source)?,
        "arduino_custom" => run_arduino_custom(&log, frames, lua_source)?,
        "moza_r5" => run_moza_r5(&log, frames),
        "moza_new" => run_moza_new(&log, frames),
        "moza_ks_pro" => run_moza_ks(&log, frames),
        "shared_serial_port" => run_shared(&log, frames),
        _ => return None,
    }
    Some(log.text())
}

fn each_frame(log: &Log, frames: &[Telemetry], mut step: impl FnMut(&Log, &Telemetry)) {
    for (index, frame) in frames.iter().enumerate() {
        log.set_tick(index as u32);
        step(log, frame);
        log.advance_tick();
    }
}

fn revlights_lit(rpm: i32, maxrpm: i32, steps: i32) -> i32 {
    if rpm <= 0 || maxrpm <= 0 || steps <= 0 {
        return 0;
    }
    let margin = (REDLINE_MARGIN * f64::from(maxrpm)).ceil() as i32;
    let interval = (maxrpm - margin) / steps;
    if interval <= 0 {
        return steps;
    }
    (rpm / interval).min(steps)
}

pub const SHIFT_PACKET_LEN: i32 = 1;
pub const SIMWIND_PACKET_LEN: i32 = 2;
const SIMWIND_LEN: usize = SIMWIND_PACKET_LEN as usize;
pub const SIMWIND_BYTE_SPEED: usize = 0;
pub const SIMWIND_BYTE_FAN: usize = 1;

pub fn shiftlights_byte(rpm: u32, maxrpm: u32, lights: i32) -> u8 {
    let lit = revlights_lit(rpm as i32, maxrpm as i32, lights);
    lit as u8
}

fn run_shiftlights(log: &Log, frames: &[Telemetry], baud: i32) -> usize {
    let id = log.open_port(CAPTURE_PORT, baud);
    each_frame(log, frames, |_log, frame| {
        let lit = shiftlights_byte(frame.rpms(), frame.maxrpm(), SHIFT_LIGHTS);
        log.write_port(id, &[lit]);
    });
    id
}

pub fn simwind_report(velocity_kph: u32, fanpower: f64) -> [u8; SIMWIND_LEN] {
    let mut bytes = [0; SIMWIND_LEN];
    bytes[SIMWIND_BYTE_SPEED] = (f64::from(velocity_kph) * KPH_TO_MPH).ceil() as u8;
    bytes[SIMWIND_BYTE_FAN] = (fanpower * FAN_BYTE_SCALE) as u8;
    bytes
}

fn run_simwind(log: &Log, frames: &[Telemetry], baud: i32) -> usize {
    let id = log.open_port(CAPTURE_PORT, baud);
    each_frame(log, frames, |_log, frame| {
        log.write_port(id, &simwind_report(frame.velocity(), FAN_POWER));
    });
    id
}

fn run_shared(log: &Log, frames: &[Telemetry]) {
    let lights = run_shiftlights(log, &[], BAUD_FIRST_OPEN);
    let wind = run_simwind(log, &[], BAUD_DEFAULT);
    each_frame(log, frames, |_log, frame| {
        let lit = revlights_lit(frame.rpms() as i32, frame.maxrpm() as i32, SHIFT_LIGHTS);
        log.write_port(lights, &[lit as u8]);
    });
    log.close_port(lights);
    each_frame(log, frames, |_log, frame| {
        let fan = (FAN_POWER * FAN_BYTE_SCALE) as u8;
        let mph = (f64::from(frame.velocity()) * KPH_TO_MPH).ceil() as u8;
        log.write_port(wind, &[mph, fan]);
    });
    log.close_port(wind);
}

fn haptic_settings() -> HapticSettings {
    HapticSettings {
        effect: VibrationEffect::TyreSlip,
        tyre: TyreId::AllFour,
        frequency: HAPTIC_HZ,
        amplitude: HAPTIC_AMP,
        threshold: HAPTIC_THRESHOLD,
        duration: HAPTIC_DURATION_S,
        motor_position: MOTOR_1,
        ..HapticSettings::default()
    }
}

fn motor_one(position: u32) -> bool {
    matches!(
        position,
        MOTOR_1
            | MOTOR_LEFT_FRONT
            | MOTOR_REAR
            | MOTOR_FRONT_AXLE
            | MOTOR_RIGHT_REAR
            | MOTOR_LEFT_REAR
            | MOTOR_DIAGONAL
            | MOTOR_ALL
    )
}

fn motor_three(position: u32) -> bool {
    matches!(
        position,
        MOTOR_RIGHT
            | MOTOR_RIGHT_MID
            | MOTOR_FRONT_AXLE
            | MOTOR_RIGHT_FRONT
            | MOTOR_RIGHT_REAR
            | MOTOR_LEFT_REAR
            | MOTOR_RIGHT_ALL
            | MOTOR_ALL
    )
}

#[derive(Clone, Copy)]
pub struct SerialHapticChannel {
    pub motor: i32,
    pub speed: u8,
}

#[derive(Clone, Copy)]
pub struct SerialHapticStep {
    pub packet: [u8; HAPTIC_PACKET],
    pub channels: [Option<SerialHapticChannel>; HAPTIC_CHANNEL_SLOTS],
}

#[derive(Default)]
pub struct SerialHapticState {
    packet: [u8; HAPTIC_PACKET],
    scaled: f64,
}

impl SerialHapticState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(&mut self, raw_play: f64, ampfactor: f64, motor: u32) -> SerialHapticStep {
        clear_haptic_motors(&mut self.packet);
        let scaled = clamp_haptic_play(raw_play * ampfactor);
        let mut channels = [None; HAPTIC_CHANNEL_SLOTS];
        if scaled != self.scaled {
            let speed = haptic_effect_speed(scaled);
            if motor_one(motor) {
                set_haptic_channel(&mut self.packet, HAPTIC_MOTOR_ONE_INDEX, speed);
                channels[HAPTIC_SLOT_ONE] = Some(SerialHapticChannel {
                    motor: HAPTIC_LOG_MOTOR_ONE,
                    speed,
                });
            }
            if motor_three(motor) {
                set_haptic_channel(&mut self.packet, HAPTIC_MOTOR_THREE_INDEX, speed);
                channels[HAPTIC_SLOT_THREE] = Some(SerialHapticChannel {
                    motor: HAPTIC_LOG_MOTOR_THREE,
                    speed,
                });
            }
            self.scaled = scaled;
        }
        SerialHapticStep {
            packet: self.packet,
            channels,
        }
    }
}

pub fn haptic_stop_packet() -> [u8; HAPTIC_PACKET] {
    let mut stopped = [0; HAPTIC_PACKET];
    for index in 0..HAPTIC_MOTORS {
        stopped[index * HAPTIC_MOTOR_STRIDE] = HAPTIC_MOTOR_FLAG;
    }
    stopped
}

fn clamp_haptic_play(play: f64) -> f64 {
    if play > PLAY_LIMIT {
        return PLAY_LIMIT;
    }
    play
}

fn haptic_effect_speed(play: f64) -> u8 {
    (EFFECT_BYTE_SCALE * play).ceil() as u8
}

fn clear_haptic_motors(packet: &mut [u8; HAPTIC_PACKET]) {
    for index in 0..HAPTIC_MOTORS {
        packet[index * HAPTIC_MOTOR_STRIDE] = 0;
    }
}

fn set_haptic_channel(packet: &mut [u8; HAPTIC_PACKET], motor_index: usize, speed: u8) {
    let motor = motor_index * HAPTIC_MOTOR_STRIDE;
    packet[motor] = HAPTIC_MOTOR_FLAG;
    packet[motor + HAPTIC_EFFECT_OFFSET] = speed;
}

fn run_haptic(log: &Log, frames: &[Telemetry]) {
    let id = log.open_port(CAPTURE_PORT, BAUD_DEFAULT);
    let clock = Trace { log };
    let mut effect = HapticEffect::new(&haptic_settings());
    let mut state = SerialHapticState::new();
    each_frame(log, frames, |_log, frame| {
        let raw = effect.play_with_clock(frame, &clock);
        let step = state.tick(raw, AMP_FACTOR, MOTOR_1);
        let _ = ARDUINO_TIMEOUT_MS;
        log.write_port(id, &step.packet);
    });
    let _ = HAPTIC_ZERO_TIMEOUT_MS;
    log.write_port(id, &haptic_stop_packet());
    log.close_port(id);
}

fn led_packet(total: usize, rgb: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; total * RGB_CHANNELS + PACKET_META];
    for slot in &mut bytes[..HEADER_MARKS] {
        *slot = MARK_BYTE;
    }
    bytes[HEADER_MARKS..HEADER_MARKS + SLED_TAG.len()].copy_from_slice(SLED_TAG);
    let start = HEADER_MARKS + SLED_TAG.len();
    let end = start + total * RGB_CHANNELS;
    let copy = rgb.len().min(end - start);
    bytes[start..start + copy].copy_from_slice(&rgb[..copy]);
    let tail = bytes.len() - LED_PACKET_TAIL.len();
    bytes[tail..].copy_from_slice(&LED_PACKET_TAIL);
    bytes
}

pub struct SimLedReport {
    pub lit: i32,
    pub bytes: Vec<u8>,
}

pub fn simled_report(
    rpm: u32,
    maxrpm: u32,
    total: i32,
    startled: i32,
    endled: i32,
) -> Option<SimLedReport> {
    let span = simled_span(total, startled, endled)?;
    let lit = if rpm > 0 && maxrpm > 0 {
        revlights_lit(rpm as i32, maxrpm as i32, span.avail)
    } else {
        0
    };
    let rgb = simled_rgb(lit, span.total, span.avail, span.start);
    Some(SimLedReport {
        lit,
        bytes: led_packet(span.total as usize, &rgb),
    })
}

pub fn simled_blank(total: i32) -> Option<Vec<u8>> {
    let count = usize::try_from(total).ok()?;
    Some(led_packet(count, &[]))
}

pub fn simled_packet(total: usize, rgb: &[u8]) -> Vec<u8> {
    led_packet(total, rgb)
}

pub fn simled_count_query() -> Vec<u8> {
    let mut query = vec![MARK_BYTE; HEADER_MARKS];
    query.extend_from_slice(LEDSC_TAG);
    query
}

pub enum SimLedCount {
    Count(i32),
    Invalid,
    Waiting,
}

pub fn parse_simled_count(bytes: &[u8]) -> SimLedCount {
    if bytes.is_empty() {
        return SimLedCount::Waiting;
    }
    let Some(parsed) = parse_strtol(c_string(bytes)) else {
        return SimLedCount::Invalid;
    };
    finish_simled_count(parsed)
}

pub fn simled_count_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(c_string(bytes)).into_owned()
}

fn c_string(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|byte| *byte == 0) {
        Some(end) => &bytes[..end],
        None => bytes,
    }
}

struct ParsedCount<'a> {
    negative: bool,
    value: u64,
    rest: &'a [u8],
}

fn parse_strtol(text: &[u8]) -> Option<ParsedCount<'_>> {
    let (negative, digits) = split_sign(skip_c_space(text));
    let (value, rest) = take_decimal(digits)?;
    Some(ParsedCount {
        negative,
        value,
        rest,
    })
}

fn finish_simled_count(parsed: ParsedCount<'_>) -> SimLedCount {
    let rest = skip_crlf(parsed.rest);
    if !rest.is_empty() || (parsed.negative && parsed.value != 0) {
        return SimLedCount::Invalid;
    }
    let Ok(count) = i32::try_from(parsed.value) else {
        return SimLedCount::Invalid;
    };
    SimLedCount::Count(count)
}

fn split_sign(bytes: &[u8]) -> (bool, &[u8]) {
    match bytes.first().copied() {
        Some(ASCII_MINUS) => (true, &bytes[1..]),
        Some(ASCII_PLUS) => (false, &bytes[1..]),
        _ => (false, bytes),
    }
}

fn take_decimal(bytes: &[u8]) -> Option<(u64, &[u8])> {
    if bytes.first().is_none_or(|byte| !byte.is_ascii_digit()) {
        return None;
    }
    let mut value = 0u64;
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        let digit = u64::from(bytes[index] - ASCII_DIGIT_ZERO);
        value = value.checked_mul(DECIMAL_RADIX)?.checked_add(digit)?;
        index += 1;
    }
    Some((value, &bytes[index..]))
}

fn skip_c_space(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .position(|byte| !is_c_space(*byte))
        .unwrap_or(bytes.len());
    &bytes[end..]
}

fn skip_crlf(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .position(|byte| *byte != C_CR && *byte != C_LF)
        .unwrap_or(bytes.len());
    &bytes[end..]
}

fn is_c_space(byte: u8) -> bool {
    matches!(byte, C_SPACE | C_TAB | C_LF | C_CR | C_VT | C_FF)
}

struct SimLedSpan {
    total: i32,
    start: i32,
    avail: i32,
}

fn simled_span(total: i32, startled: i32, endled: i32) -> Option<SimLedSpan> {
    if total < LED_FIRST {
        return None;
    }
    let mut end = endled;
    if end == SIMLED_END_ALL {
        end = total;
    }
    let mut start = startled;
    if start < LED_FIRST {
        start = LED_FIRST;
    }
    if end > total {
        end = total;
    }
    let avail = end - start + 1;
    if avail < LED_FIRST {
        return None;
    }
    Some(SimLedSpan {
        total,
        start,
        avail,
    })
}

fn simled_rgb(lit: i32, total: i32, avail: i32, startled: i32) -> Vec<u8> {
    let mut rgb = vec![0; total as usize * RGB_CHANNELS];
    let half = avail / 2;
    let last = avail - 1;
    for index in 0..lit {
        let led = index + startled - LED_FIRST;
        if led < 0 || led >= total {
            continue;
        }
        let base = led as usize * RGB_CHANNELS;
        if index < half {
            rgb[base + GREEN_CHANNEL] = MARK_BYTE;
        } else if index < last {
            rgb[base + RED_CHANNEL] = MARK_BYTE;
            rgb[base + GREEN_CHANNEL] = MARK_BYTE;
        }
        if index == last {
            rgb[base + RED_CHANNEL] = MARK_BYTE;
            rgb[base + GREEN_CHANNEL] = 0;
        }
    }
    rgb
}

fn run_simled(log: &Log, frames: &[Telemetry]) {
    let id = log.open_port(CAPTURE_PORT, BAUD_DEFAULT);
    let total = SIMLED_COUNT;
    each_frame(log, frames, |_log, frame| {
        let Some(report) = simled_report(frame.rpms(), frame.maxrpm(), total, LED_FIRST, total)
        else {
            return;
        };
        log.write_port(id, &report.bytes);
    });
    if let Some(blank) = simled_blank(total) {
        log.write_port(id, &blank);
    }
    log.close_port(id);
}

fn query_led_count(log: &Log, id: usize) -> Option<i32> {
    log.wait_port(SIMLED_QUERY_WAIT_MS);
    log.write_port(id, &simled_count_query());
    log.wait_port(SIMLED_QUERY_WAIT_MS);
    let reply = log.read_port(id);
    let SimLedCount::Count(count) = parse_simled_count(&reply) else {
        return None;
    };
    if count < LED_FIRST {
        return None;
    }
    Some(count)
}

fn run_simled_custom(log: &Log, frames: &[Telemetry], lua_source: &str) -> Option<()> {
    let id = log.open_port(CAPTURE_PORT, BAUD_DEFAULT);
    let total = query_led_count(log, id)?;
    let mut lua = LuaHost::load(lua_source, LuaLedMode::Serial).ok()?;
    let clock = Trace { log };
    let mut owned: Vec<Telemetry> = frames.iter().map(Telemetry::clone_buf).collect();
    for (index, frame) in owned.iter_mut().enumerate() {
        log.set_tick(index as u32);
        let tick = lua.call(frame, i64::from(total), &clock).ok()?;
        log.write_port(id, &led_packet(total as usize, &tick.leds));
        log.advance_tick();
    }
    log.close_port(id);
    Some(())
}

fn run_arduino_custom(log: &Log, frames: &[Telemetry], lua_source: &str) -> Option<()> {
    let id = log.open_port(CAPTURE_PORT, BAUD_DEFAULT);
    let mut lua = LuaHost::load(lua_source, LuaLedMode::Serial).ok()?;
    let clock = Trace { log };
    let mut owned: Vec<Telemetry> = frames.iter().map(Telemetry::clone_buf).collect();
    for (index, frame) in owned.iter_mut().enumerate() {
        log.set_tick(index as u32);
        let tick = lua.call(frame, 0, &clock).ok()?;
        if let Some(message) = tick.message {
            log.write_port(id, message.as_bytes());
        }
        log.advance_tick();
    }
    log.close_port(id);
    Some(())
}

fn moza_checksum(data: &[u8]) -> u8 {
    let sum = data
        .iter()
        .fold(MOZA_MAGIC, |acc, byte| acc + u32::from(*byte));
    (sum % 256) as u8
}

fn flag_byte(value: i32, bit: u32) -> bool {
    value >= bit as i32
}

fn r5_packet(rpm: u32, maxrpm: u32) -> [u8; MOZA_R5_SIZE] {
    let mut bytes = MOZA_R5_TEMPLATE;
    let percent = ((rpm as f32) / (maxrpm as f32) * MOZA_PERCENT).round() as i32;
    if flag_byte(percent, 10) {
        bytes[9] |= 1 << 0;
    }
    if flag_byte(percent, 20) {
        bytes[9] |= 1 << 1;
    }
    if flag_byte(percent, 30) {
        bytes[9] |= 1 << 2;
    }
    if flag_byte(percent, 40) {
        bytes[9] |= 1 << 3;
    }
    if flag_byte(percent, 50) {
        bytes[9] |= 1 << 4;
    }
    if flag_byte(percent, 60) {
        bytes[9] |= 1 << 5;
    }
    if flag_byte(percent, 70) {
        bytes[9] |= 1 << 6;
    }
    if flag_byte(percent, 80) {
        bytes[9] |= 1 << 7;
    }
    if flag_byte(percent, 90) {
        bytes[8] |= 1 << 0;
    }
    if flag_byte(percent, 92) {
        bytes[8] |= 1 << 1;
    }
    if flag_byte(percent, 94) {
        bytes[8] |= 1 << MOZA_BLINK_BIT;
    }
    bytes[10] = moza_checksum(&bytes);
    bytes
}

fn run_moza_r5(log: &Log, frames: &[Telemetry]) {
    let id = log.open_port(CAPTURE_PORT, BAUD_DEFAULT);
    each_frame(log, frames, |_log, _frame| {});
    let _ = r5_packet;
    let _ = MOZA_TIMEOUT_MS;
    log.close_port(id);
}

fn ks_color(template: [u8; KS_COLOR_SIZE]) -> [u8; KS_COLOR_SIZE] {
    let mut bytes = template;
    bytes[KS_COLOR_SIZE - 1] = moza_checksum(&bytes);
    bytes
}

fn ks_init_colors() -> [[u8; KS_COLOR_SIZE]; 6] {
    [
        ks_color([
            0x7e, 0x16, 0x3f, 0x17, 0x19, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0xff, 0, 0, 4,
            0xff, 0, 0, 0,
        ]),
        ks_color([
            0x7e, 0x16, 0x3f, 0x17, 0x19, 0, 5, 0xff, 0, 0, 6, 0xff, 0, 0, 7, 0xff, 0, 0, 8, 0xff,
            0, 0, 9, 0xff, 0, 0, 0,
        ]),
        ks_color([
            0x7e, 0x16, 0x3f, 0x17, 0x19, 0, 10, 0xff, 0x7f, 0, 11, 0xff, 0x7f, 0, 12, 0xff, 0x7f,
            0, 13, 0, 0, 0xff, 14, 0, 0, 0xff, 0,
        ]),
        ks_color([
            0x7e, 0x16, 0x3f, 0x17, 0x19, 1, 0, 0xff, 0, 0, 1, 0xff, 0, 0, 2, 0xff, 0, 0, 3, 0xff,
            0, 0, 4, 0xff, 0, 0, 0,
        ]),
        ks_color([
            0x7e, 0x16, 0x3f, 0x17, 0x19, 1, 5, 0xff, 0, 0, 6, 0xff, 0, 0, 7, 0xff, 0, 0, 8, 0xff,
            0, 0, 9, 0xff, 0, 0, 0,
        ]),
        ks_color([
            0x7e, 0x16, 0x3f, 0x17, 0x19, 0, 15, 0, 0, 0, 16, 0, 0, 0, 17, 0, 0, 0, 15, 0, 0, 0,
            16, 0, 0, 0, 0,
        ]),
    ]
}

fn ks_mask(frame: &Telemetry) -> [u8; KS_MASK_SIZE] {
    let mut bytes = KS_MASK_TEMPLATE;
    let mut percent =
        ((frame.rpms() as f32) / (frame.maxrpm() as f32) * MOZA_PERCENT).round() as i32;
    if percent >= KS_BLINK_PERCENT && ((frame.mtick() >> KS_BLINK_SHIFT) & 1) == 1 {
        percent = 0;
    }
    set_ks_bits(&mut bytes, percent);
    if frame.player_flag() == FLAG_YELLOW {
        bytes[6] |= (1 << 0) | (1 << 1) | (1 << 2);
        bytes[7] |= 1 << 7;
        bytes[8] |= (1 << 0) | (1 << 1);
    }
    bytes[10] = moza_checksum(&bytes);
    bytes
}

fn set_ks_bits(bytes: &mut [u8; KS_MASK_SIZE], percent: i32) {
    const STEPS: [(i32, usize, u8); 12] = [
        (75, 6, 3),
        (77, 6, 4),
        (79, 6, 5),
        (81, 6, 6),
        (83, 6, 7),
        (85, 7, 0),
        (87, 7, 1),
        (89, 7, 2),
        (91, 7, 3),
        (93, 7, 4),
        (95, 7, 5),
        (97, 7, 6),
    ];
    for (threshold, index, bit) in STEPS {
        if percent >= threshold {
            bytes[index] |= 1 << bit;
        }
    }
}

fn run_moza_ks(log: &Log, frames: &[Telemetry]) {
    let id = log.open_port(CAPTURE_PORT, BAUD_DEFAULT);
    for packet in ks_init_colors() {
        log.write_port(id, &packet);
    }
    each_frame(log, frames, |_log, _frame| {});
    let _ = ks_mask;
    log.close_port(id);
}

fn wire_checksum(decoded: &[u8]) -> u8 {
    let mut sum = MOZA_MAGIC;
    for byte in decoded {
        sum += u32::from(*byte);
    }
    for byte in decoded.iter().skip(2) {
        if *byte == MOZA_START {
            sum += u32::from(MOZA_START);
        }
    }
    (sum & 0xff) as u8
}

fn byte_stuff(frame: &[u8]) -> Option<Vec<u8>> {
    if frame.len() < 2 {
        return None;
    }
    let mut out = Vec::with_capacity(frame.len() + 4);
    out.push(frame[0]);
    out.push(frame[1]);
    for byte in &frame[2..] {
        if out.len() + 1 >= MOZA_MAX_FRAME {
            return None;
        }
        out.push(*byte);
        if *byte == MOZA_START {
            out.push(MOZA_START);
        }
    }
    Some(out)
}

trait FrameSink {
    fn push_frame(&mut self, data: &[u8]) -> bool;
}

struct LogSink<'a> {
    log: &'a Log,
    id: usize,
}

impl FrameSink for LogSink<'_> {
    fn push_frame(&mut self, data: &[u8]) -> bool {
        self.log.write_port(self.id, data);
        true
    }
}

struct VecSink<'a> {
    frames: &'a mut Vec<Vec<u8>>,
}

impl FrameSink for VecSink<'_> {
    fn push_frame(&mut self, data: &[u8]) -> bool {
        self.frames.push(data.to_vec());
        true
    }
}

struct FixedNs(u64);

impl Clock for FixedNs {
    fn monotonic_ms(&self) -> u64 {
        self.0 / NS_PER_MS
    }

    fn monotonic_us(&self) -> u64 {
        self.0 / NS_PER_US
    }

    fn monotonic_ns(&self) -> u64 {
        self.0
    }

    fn wall_ms(&self) -> u64 {
        0
    }
}

struct MozaSignals {
    arm_failed: bool,
    just_armed: bool,
    rpm_failed: bool,
    rpm_sent: bool,
}

impl MozaSignals {
    fn none() -> Self {
        Self {
            arm_failed: false,
            just_armed: false,
            rpm_failed: false,
            rpm_sent: false,
        }
    }
}

fn write_frame(out: &mut dyn FrameSink, cmd: &[u8], payload: &[u8]) -> bool {
    let body = cmd.len() + payload.len();
    if body > 255 || 5 + body > MOZA_MAX_FRAME {
        return false;
    }
    let mut decoded = Vec::with_capacity(5 + body);
    decoded.push(MOZA_START);
    decoded.push(body as u8);
    decoded.push(MOZA_GROUP);
    decoded.push(MOZA_DEVICE);
    decoded.extend_from_slice(cmd);
    decoded.extend_from_slice(payload);
    let sum = wire_checksum(&decoded);
    decoded.push(sum);
    let Some(stuffed) = byte_stuff(&decoded) else {
        return false;
    };
    out.push_frame(&stuffed)
}

fn send_rpm_mask(out: &mut dyn FrameSink, active: u32) -> bool {
    let window = (1u32 << MOZA_LED_COUNT) - 1;
    let mut payload = [0; 8];
    payload[..4].copy_from_slice(&active.to_le_bytes());
    payload[4..].copy_from_slice(&window.to_le_bytes());
    write_frame(out, &[0x1a, 0x00], &payload)
}

fn send_buttons(out: &mut dyn FrameSink, mask: u16) -> bool {
    write_frame(out, &[0x1a, 0x01], &mask.to_le_bytes())
}

fn send_colours(out: &mut dyn FrameSink, group: u8, table: &[u8; MOZA_COLOUR_BYTES]) -> bool {
    let cmd = [0x19, group];
    write_frame(out, &cmd, &table[..MOZA_COLOUR_CHUNK])
        && write_frame(out, &cmd, &table[MOZA_COLOUR_CHUNK..])
}

fn fill_solid(table: &mut [u8; MOZA_COLOUR_BYTES], rgb: [u8; 3]) {
    for index in 0..MOZA_LED_COUNT {
        let base = index * RGB_STRIDE;
        table[base] = index as u8;
        table[base + 1] = rgb[0];
        table[base + 2] = rgb[1];
        table[base + 3] = rgb[2];
    }
}

fn fill_rpm(table: &mut [u8; MOZA_COLOUR_BYTES]) {
    for (index, rgb) in RPM_RGB.iter().enumerate() {
        let base = index * RGB_STRIDE;
        table[base] = index as u8;
        table[base + 1] = rgb[0];
        table[base + 2] = rgb[1];
        table[base + 3] = rgb[2];
    }
}

fn arm_telemetry(out: &mut dyn FrameSink) -> bool {
    let bright = [MOZA_BRIGHTNESS];
    if !write_frame(out, &[0x1c, 0x00], &[1]) {
        return false;
    }
    if !write_frame(out, &[0x1b, 0x00, 0xff], &bright) {
        return false;
    }
    let mut rpm = [0; MOZA_COLOUR_BYTES];
    fill_rpm(&mut rpm);
    if !send_colours(out, 0x00, &rpm) {
        return false;
    }
    if !write_frame(out, &[0x1b, 0x01, 0xff], &bright) {
        return false;
    }
    write_frame(out, &[0x1d, 0x00], &[0])
}

impl MozaNew {
    fn new() -> Self {
        Self {
            armed: false,
            braking: false,
            lock_latched: false,
            abs_latched: false,
            has_button_write: false,
            bar: Bar::Rpm,
            last_rpm_lit: 0,
            last_brake_lit: 0,
            last_corners: [Rgb { r: 0, g: 0, b: 0 }; MOZA_CORNERS],
            started_ns: 0,
            last_button_ns: 0,
        }
    }

    fn reset(&mut self, clock: &impl Clock) {
        *self = Self::new();
        self.started_ns = clock.monotonic_ns();
    }
}

fn mask_from_lit(lit: usize) -> u32 {
    if lit == 0 {
        return 0;
    }
    if lit >= 32 {
        return u32::MAX;
    }
    (1u32 << lit) - 1
}

fn apply_hysteresis(continuous: f32, raw: usize, last_lit: usize) -> usize {
    if raw == 0 || raw == last_lit {
        return raw;
    }
    let last = last_lit as f32;
    if raw > last_lit && continuous < last + 0.5 + MOZA_HYSTERESIS {
        return last_lit;
    }
    if raw < last_lit && continuous > last - 0.5 - MOZA_HYSTERESIS {
        return last_lit;
    }
    raw
}

fn rpm_bits(rpm: f32, redline: f32, idle: f32, last_lit: usize) -> (u32, usize) {
    if redline <= idle {
        return (0, 0);
    }
    let span = redline - idle;
    let window_start = idle + MOZA_RPM_WINDOW * span;
    if rpm < window_start || redline <= window_start {
        return (0, 0);
    }
    let mut frac = (rpm - window_start) / (redline - window_start);
    frac = frac.clamp(0.0, 1.0);
    let continuous = frac * MOZA_LED_COUNT as f32;
    let raw = (continuous.round() as usize).min(MOZA_LED_COUNT);
    let lit = apply_hysteresis(continuous, raw, last_lit);
    (mask_from_lit(lit), lit)
}

fn frac_bits(frac: f32, last_lit: usize) -> (u32, usize) {
    if !frac.is_finite() || frac <= 0.0 {
        return (0, 0);
    }
    let frac = frac.min(1.0);
    let continuous = frac * MOZA_LED_COUNT as f32;
    let raw = (continuous.round() as usize).clamp(1, MOZA_LED_COUNT);
    let lit = apply_hysteresis(continuous, raw, last_lit).max(1);
    (mask_from_lit(lit), lit)
}

fn peak_slip(frame: &Telemetry) -> f32 {
    let mut peak = 0.0_f32;
    for index in 0..MOZA_CORNERS {
        let slip = frame.tyre_slip(index) as f32;
        if slip > peak {
            peak = slip;
        }
    }
    peak
}

fn alert_mode(state: &mut MozaNew, frame: &Telemetry) -> Bar {
    let speed_ms = frame.velocity() as f32 * MOZA_KM_H_TO_M_S;
    let lock_slip = peak_slip(frame);
    let locking = frame.brake() as f32 >= MOZA_MIN_BRAKE
        && speed_ms >= MOZA_MIN_SPEED_MS
        && lock_slip >= MOZA_LOCK_SLIP;
    let abs_on = frame.abs() > ABS_ON;
    if state.lock_latched {
        if !locking || lock_slip < MOZA_CLEAR_SLIP {
            state.lock_latched = false;
        }
    } else if locking {
        state.lock_latched = true;
    }
    if state.abs_latched {
        if !abs_on {
            state.abs_latched = false;
        }
    } else if abs_on {
        state.abs_latched = true;
    }
    if state.lock_latched {
        return Bar::Lock;
    }
    if state.abs_latched {
        return Bar::Abs;
    }
    Bar::Rpm
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn heat_rgb(heat: f32) -> Rgb {
    if !heat.is_finite() || heat < MOZA_HEAT_OFF {
        return Rgb { r: 0, g: 0, b: 0 };
    }
    if heat <= MOZA_YELLOW_FULL {
        let scale = clamp01((heat - MOZA_HEAT_OFF) / (MOZA_YELLOW_FULL - MOZA_HEAT_OFF));
        return Rgb {
            r: (f32::from(BRAKE_YELLOW[0]) * scale).round() as u8,
            g: (f32::from(BRAKE_YELLOW[1]) * scale).round() as u8,
            b: (f32::from(BRAKE_YELLOW[2]) * scale).round() as u8,
        };
    }
    let scale = clamp01((heat - MOZA_YELLOW_FULL) / (1.0 - MOZA_YELLOW_FULL));
    Rgb {
        r: lerp_channel(BRAKE_YELLOW[0], HEAT_RED[0], scale),
        g: lerp_channel(BRAKE_YELLOW[1], HEAT_RED[1], scale),
        b: lerp_channel(BRAKE_YELLOW[2], HEAT_RED[2], scale),
    }
}

fn lerp_channel(from: u8, to: u8, scale: f32) -> u8 {
    (f32::from(from) + (f32::from(to) - f32::from(from)) * scale).round() as u8
}

fn heat_scaled(temp_c: f32, acr: bool) -> f32 {
    if !temp_c.is_finite() || temp_c <= 0.0 {
        return 0.0;
    }
    if temp_c <= MOZA_TEMP_NORMAL_MAX {
        return clamp01(temp_c);
    }
    let (cold, hot) = if acr {
        (MOZA_ACR_COLD, MOZA_ACR_HOT)
    } else {
        (MOZA_DR2_COLD, MOZA_DR2_HOT)
    };
    clamp01((temp_c - cold) / (hot - cold))
}

fn temps_placeholder(frame: &Telemetry) -> bool {
    let mut min = frame.brake_temp(0);
    let mut max = min;
    for index in 1..MOZA_CORNERS {
        let temp = frame.brake_temp(index);
        min = min.min(temp);
        max = max.max(temp);
    }
    (max - min) < MOZA_PLACEHOLDER_SPAN
}

fn corners_from(frame: &Telemetry) -> [Rgb; MOZA_CORNERS] {
    let mut out = [Rgb { r: 0, g: 0, b: 0 }; MOZA_CORNERS];
    if frame.simexe() == ACR_SIMEXE && temps_placeholder(frame) {
        return out;
    }
    let acr = frame.simexe() == ACR_SIMEXE;
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = heat_rgb(heat_scaled(frame.brake_temp(index) as f32, acr));
    }
    out
}

fn corner_on(color: &Rgb) -> bool {
    color.r > 0 || color.g > 0 || color.b > 0
}

fn corners_mask(corners: &[Rgb; MOZA_CORNERS]) -> u16 {
    let mut mask = 0_u16;
    for (index, color) in corners.iter().enumerate() {
        if corner_on(color) {
            mask |= 1 << BRAKE_BUTTONS[index];
        }
    }
    mask
}

fn corners_table(corners: &[Rgb; MOZA_CORNERS]) -> [u8; MOZA_COLOUR_BYTES] {
    let mut table = [0; MOZA_COLOUR_BYTES];
    for index in 0..MOZA_LED_COUNT {
        table[index * RGB_STRIDE] = index as u8;
    }
    for (index, color) in corners.iter().enumerate() {
        let led = BRAKE_BUTTONS[index];
        if led >= MOZA_LED_COUNT {
            continue;
        }
        let base = led * RGB_STRIDE;
        table[base] = led as u8;
        table[base + 1] = color.r;
        table[base + 2] = color.g;
        table[base + 3] = color.b;
    }
    table
}

fn corners_same(left: &[Rgb; MOZA_CORNERS], right: &[Rgb; MOZA_CORNERS]) -> bool {
    left.iter()
        .zip(right)
        .all(|(a, b)| a.r == b.r && a.g == b.g && a.b == b.b)
}

fn apply_bar_colours(out: &mut dyn FrameSink, mode: Bar) -> bool {
    let mut table = [0; MOZA_COLOUR_BYTES];
    match mode {
        Bar::Brake | Bar::Lock => fill_solid(&mut table, PURPLE),
        Bar::Abs => fill_solid(&mut table, BLUE_ALERT),
        Bar::Rpm => fill_rpm(&mut table),
    }
    send_colours(out, 0x00, &table)
}

fn elapsed_ms(clock: &impl Clock, start_ns: u64) -> u64 {
    clock.monotonic_ns().saturating_sub(start_ns) / NS_PER_MS
}

fn blink_on(clock: &impl Clock, state: &MozaNew) -> bool {
    let periods = elapsed_ms(clock, state.started_ns) / MOZA_ALERT_MS;
    periods.is_multiple_of(2)
}

fn write_brake_buttons(out: &mut dyn FrameSink, corners: &[Rgb; MOZA_CORNERS]) -> bool {
    let table = corners_table(corners);
    send_colours(out, 0x01, &table) && send_buttons(out, corners_mask(corners))
}

fn blank_wheel(out: &mut dyn FrameSink, state: &mut MozaNew, clock: &impl Clock) {
    let off = [Rgb { r: 0, g: 0, b: 0 }; MOZA_CORNERS];
    let _ = write_brake_buttons(out, &off);
    let _ = send_rpm_mask(out, 0);
    if state.bar != Bar::Rpm {
        let _ = apply_bar_colours(out, Bar::Rpm);
    }
    state.reset(clock);
    state.armed = true;
}

fn desired_bar(state: &mut MozaNew, frame: &Telemetry, alert: Bar) -> Bar {
    if frame.brake() as f32 >= MOZA_BRAKE_SHOW {
        state.braking = true;
    } else if (frame.brake() as f32) < MOZA_BRAKE_HIDE {
        state.braking = false;
    }
    if alert != Bar::Rpm {
        return alert;
    }
    if state.braking {
        return Bar::Brake;
    }
    Bar::Rpm
}

fn bar_mask(state: &mut MozaNew, frame: &Telemetry, bar: Bar, clock: &impl Clock) -> u32 {
    match bar {
        Bar::Brake => {
            let (mask, lit) = frac_bits(frame.brake() as f32, state.last_brake_lit);
            state.last_brake_lit = lit;
            mask
        }
        Bar::Lock | Bar::Abs => {
            let mask = if frame.brake() as f32 >= MOZA_BRAKE_HIDE {
                let (mask, lit) = frac_bits(frame.brake() as f32, state.last_brake_lit);
                state.last_brake_lit = lit;
                mask
            } else {
                state.last_brake_lit = MOZA_LED_COUNT;
                mask_from_lit(MOZA_LED_COUNT)
            };
            if blink_on(clock, state) {
                mask
            } else {
                0
            }
        }
        Bar::Rpm => {
            state.last_brake_lit = 0;
            let (mask, lit) = rpm_bits(
                frame.rpms() as f32,
                frame.maxrpm() as f32,
                frame.idlerpm() as f32,
                state.last_rpm_lit,
            );
            state.last_rpm_lit = lit;
            mask
        }
    }
}

fn update_moza_new(
    out: &mut dyn FrameSink,
    state: &mut MozaNew,
    frame: &Telemetry,
    clock: &impl Clock,
) -> MozaSignals {
    let mut signals = MozaSignals::none();
    if !state.armed {
        if !arm_telemetry(out) {
            signals.arm_failed = true;
            return signals;
        }
        state.reset(clock);
        state.armed = true;
        signals.just_armed = true;
    }
    if frame.simstatus() != STATUS_ACTIVE {
        blank_wheel(out, state, clock);
        return signals;
    }
    let alert = alert_mode(state, frame);
    let bar = desired_bar(state, frame, alert);
    if bar != state.bar && apply_bar_colours(out, bar) {
        if bar == Bar::Rpm {
            state.last_brake_lit = 0;
        }
        state.bar = bar;
    }
    let corners = corners_from(frame);
    let changed = !corners_same(&corners, &state.last_corners);
    let on_off = corners_mask(&corners) != corners_mask(&state.last_corners);
    let due = !state.has_button_write || elapsed_ms(clock, state.last_button_ns) >= MOZA_BUTTON_MS;
    if changed && (on_off || due) && write_brake_buttons(out, &corners) {
        state.last_corners = corners;
        state.last_button_ns = clock.monotonic_ns();
        state.has_button_write = true;
    }
    let bits = bar_mask(state, frame, state.bar, clock);
    if !send_rpm_mask(out, bits) {
        state.armed = false;
        signals.rpm_failed = true;
        return signals;
    }
    signals.rpm_sent = true;
    signals
}

fn run_moza_new(log: &Log, frames: &[Telemetry]) {
    let baud = BAUD_DEFAULT.max(BAUD_R9_FLOOR);
    let id = log.open_port(CAPTURE_PORT, baud);
    let clock = Trace { log };
    let mut state = MozaNew::new();
    state.reset(&clock);
    let mut sink = LogSink { log, id };
    if arm_telemetry(&mut sink) {
        state.armed = true;
    }
    for (index, frame) in frames.iter().enumerate() {
        log.set_tick(index as u32);
        let _ = update_moza_new(&mut sink, &mut state, frame, &clock);
        log.advance_tick();
    }
    log.close_port(id);
}

/// Moza R9 / new-firmware wheel. `prepare` arms telemetry; `tick` emits the C frames.
pub struct MozaNewWheel {
    state: MozaNew,
}

#[derive(Clone, Debug, Default)]
pub struct MozaNewStep {
    pub frames: Vec<Vec<u8>>,
    pub arm_failed: bool,
    pub just_armed: bool,
    pub rpm_failed: bool,
    pub rpm_sent: bool,
}

impl Default for MozaNewWheel {
    fn default() -> Self {
        Self::new()
    }
}

impl MozaNewWheel {
    pub fn new() -> Self {
        Self {
            state: MozaNew::new(),
        }
    }

    pub fn armed(&self) -> bool {
        self.state.armed
    }

    pub fn disarm(&mut self) {
        self.state.armed = false;
    }

    pub fn prepare(&mut self, now_ns: u64) -> MozaNewStep {
        let clock = FixedNs(now_ns);
        self.state.reset(&clock);
        let mut frames = Vec::new();
        let armed = {
            let mut sink = VecSink {
                frames: &mut frames,
            };
            arm_telemetry(&mut sink)
        };
        self.state.armed = armed;
        MozaNewStep {
            frames,
            ..MozaNewStep::default()
        }
    }

    pub fn tick(&mut self, frame: &Telemetry, now_ns: u64) -> MozaNewStep {
        let clock = FixedNs(now_ns);
        let mut frames = Vec::new();
        let signals = {
            let mut sink = VecSink {
                frames: &mut frames,
            };
            update_moza_new(&mut sink, &mut self.state, frame, &clock)
        };
        MozaNewStep {
            frames,
            arm_failed: signals.arm_failed,
            just_armed: signals.just_armed,
            rpm_failed: signals.rpm_failed,
            rpm_sent: signals.rpm_sent,
        }
    }
}

pub fn moza_r9_open_baud(configured: i64) -> u32 {
    let configured = u32::try_from(configured).unwrap_or(0);
    let floor = u32::try_from(BAUD_R9_FLOOR).unwrap_or(0);
    configured.max(floor)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RPM: u32 = 4_000;
    const SAMPLE_MAX_RPM: u32 = 8_000;
    const SAMPLE_LIGHTS: i32 = 6;
    const SAMPLE_LIT: u8 = 3;

    #[test]
    fn shiftlights_byte_counts_leds_up_to_the_redline() {
        assert_eq!(
            shiftlights_byte(SAMPLE_RPM, SAMPLE_MAX_RPM, SAMPLE_LIGHTS),
            SAMPLE_LIT
        );
        assert_eq!(shiftlights_byte(0, SAMPLE_MAX_RPM, SAMPLE_LIGHTS), 0);
        assert_eq!(shiftlights_byte(SAMPLE_RPM, SAMPLE_MAX_RPM, 0), 0);
    }

    #[test]
    fn simwind_report_converts_kph_and_scales_the_fan() {
        const SAMPLE_KPH: u32 = 80;
        const SAMPLE_FAN: f64 = 0.6;
        const SAMPLE_MPH: u8 = 50;
        const SAMPLE_FAN_BYTE: u8 = 153;
        let report = simwind_report(SAMPLE_KPH, SAMPLE_FAN);
        assert_eq!(report[SIMWIND_BYTE_SPEED], SAMPLE_MPH);
        assert_eq!(report[SIMWIND_BYTE_FAN], SAMPLE_FAN_BYTE);
        assert_eq!(report.len(), SIMWIND_LEN);
    }

    #[test]
    fn haptic_keeps_effect_bytes_when_play_is_unchanged() {
        const FULL_SPEED: u8 = 255;
        let mut state = SerialHapticState::new();
        let first = state.tick(PLAY_LIMIT, AMP_FACTOR, MOTOR_1);
        assert_eq!(
            first.packet,
            [HAPTIC_MOTOR_FLAG, FULL_SPEED, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            first.channels[HAPTIC_SLOT_ONE].map(|channel| channel.speed),
            Some(FULL_SPEED)
        );
        let held = state.tick(PLAY_LIMIT, AMP_FACTOR, MOTOR_1);
        assert_eq!(held.packet, [0, FULL_SPEED, 0, 0, 0, 0, 0, 0]);
        assert!(held.channels.iter().all(Option::is_none));
    }

    #[test]
    fn haptic_default_motor_enables_no_channel() {
        const MOTOR_CONFIG_DEFAULT: u32 = 1;
        let mut state = SerialHapticState::new();
        let step = state.tick(PLAY_LIMIT, AMP_FACTOR, MOTOR_CONFIG_DEFAULT);
        assert_eq!(step.packet, [0; HAPTIC_PACKET]);
        assert!(step.channels.iter().all(Option::is_none));
    }

    #[test]
    fn haptic_front_axle_sets_both_channels() {
        const FULL_SPEED: u8 = 255;
        let mut state = SerialHapticState::new();
        let step = state.tick(PLAY_LIMIT, AMP_FACTOR, MOTOR_FRONT_AXLE);
        assert_eq!(
            step.packet,
            [
                HAPTIC_MOTOR_FLAG,
                FULL_SPEED,
                0,
                0,
                HAPTIC_MOTOR_FLAG,
                FULL_SPEED,
                0,
                0
            ]
        );
        assert_eq!(
            step.channels[HAPTIC_SLOT_THREE].map(|channel| channel.motor),
            Some(HAPTIC_LOG_MOTOR_THREE)
        );
    }

    #[test]
    fn haptic_stop_packet_enables_every_motor() {
        assert_eq!(
            haptic_stop_packet(),
            [
                HAPTIC_MOTOR_FLAG,
                0,
                HAPTIC_MOTOR_FLAG,
                0,
                HAPTIC_MOTOR_FLAG,
                0,
                HAPTIC_MOTOR_FLAG,
                0
            ]
        );
    }

    #[test]
    fn haptic_return_to_zero_still_flags_the_motor() {
        const SAMPLE_PLAY: f64 = 0.4;
        const SAMPLE_SPEED: u8 = 102;
        let mut state = SerialHapticState::new();
        let active = state.tick(SAMPLE_PLAY, AMP_FACTOR, MOTOR_1);
        assert_eq!(
            active.packet,
            [HAPTIC_MOTOR_FLAG, SAMPLE_SPEED, 0, 0, 0, 0, 0, 0]
        );
        let idle = state.tick(0.0, AMP_FACTOR, MOTOR_1);
        assert_eq!(idle.packet, [HAPTIC_MOTOR_FLAG, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn simled_report_paints_green_then_the_last_led_red() {
        const TOTAL: i32 = 8;
        const SAMPLE_RPM: u32 = 4_000;
        const SAMPLE_MAX: u32 = 8_000;
        const SAMPLE_LIT: i32 = 4;
        let report = simled_report(SAMPLE_RPM, SAMPLE_MAX, TOTAL, LED_FIRST, TOTAL).expect("span");
        assert_eq!(report.lit, SAMPLE_LIT);
        let mut expected = vec![MARK_BYTE; HEADER_MARKS];
        expected.extend_from_slice(SLED_TAG);
        for led in 0..TOTAL {
            if led < SAMPLE_LIT {
                expected.extend_from_slice(&[0, MARK_BYTE, 0]);
            } else {
                expected.extend_from_slice(&[0, 0, 0]);
            }
        }
        expected.extend_from_slice(&LED_PACKET_TAIL);
        assert_eq!(report.bytes, expected);
    }

    #[test]
    fn simled_endled_zero_uses_every_led() {
        const TOTAL: i32 = 8;
        let all = simled_report(8_000, 8_000, TOTAL, LED_FIRST, SIMLED_END_ALL).expect("all");
        let explicit = simled_report(8_000, 8_000, TOTAL, LED_FIRST, TOTAL).expect("explicit");
        assert_eq!(all.bytes, explicit.bytes);
    }

    #[test]
    fn simled_single_available_led_is_red() {
        const TOTAL: i32 = 6;
        const ONE: i32 = 1;
        const SAMPLE_RPM: u32 = 8_000;
        let report = simled_report(SAMPLE_RPM, SAMPLE_RPM, TOTAL, ONE, ONE).expect("one");
        assert_eq!(report.lit, ONE);
        assert_eq!(report.bytes[HEADER_MARKS + SLED_TAG.len()], MARK_BYTE);
        assert_eq!(
            report.bytes[HEADER_MARKS + SLED_TAG.len() + GREEN_CHANNEL],
            0
        );
    }

    #[test]
    fn simled_without_leds_does_not_build_a_frame() {
        assert!(simled_report(4_000, 8_000, 0, LED_FIRST, SIMLED_END_ALL).is_none());
    }

    #[test]
    fn simled_blank_clears_the_color_bytes() {
        const TOTAL: i32 = 8;
        let blank = simled_blank(TOTAL).expect("blank");
        let dark = simled_report(0, 8_000, TOTAL, LED_FIRST, TOTAL).expect("dark");
        assert_eq!(dark.lit, 0);
        assert_eq!(blank, dark.bytes);
    }

    #[test]
    fn simled_count_query_is_the_ledsc_packet() {
        let query = simled_count_query();
        assert_eq!(&query[..HEADER_MARKS], &[MARK_BYTE; HEADER_MARKS]);
        assert_eq!(&query[HEADER_MARKS..], LEDSC_TAG);
    }

    #[test]
    fn simled_count_parser_matches_strtol() {
        const EIGHT: i32 = 8;
        const ZERO: i32 = 0;
        const INT_MAX_TEXT: &[u8] = b"2147483647";
        const PAST_INT_MAX: &[u8] = b"2147483648";
        assert!(matches!(
            parse_simled_count(SIMLED_COUNT_REPLY),
            SimLedCount::Count(EIGHT)
        ));
        assert!(matches!(parse_simled_count(b"0"), SimLedCount::Count(ZERO)));
        assert!(matches!(
            parse_simled_count(b" 8\n"),
            SimLedCount::Count(EIGHT)
        ));
        assert!(matches!(
            parse_simled_count(b"8\0junk"),
            SimLedCount::Count(EIGHT)
        ));
        assert!(matches!(
            parse_simled_count(INT_MAX_TEXT),
            SimLedCount::Count(i32::MAX)
        ));
        assert!(matches!(parse_simled_count(b""), SimLedCount::Waiting));
        assert!(matches!(parse_simled_count(b"nope"), SimLedCount::Invalid));
        assert!(matches!(parse_simled_count(b"-1"), SimLedCount::Invalid));
        assert!(matches!(parse_simled_count(b"8x"), SimLedCount::Invalid));
        assert!(matches!(
            parse_simled_count(PAST_INT_MAX),
            SimLedCount::Invalid
        ));
        assert_eq!(simled_count_text(b"nope"), "nope");
    }
}
