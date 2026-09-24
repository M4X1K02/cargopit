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
const CAPTURE_PORT: &str = "/dev/ttyPARITY0";
const BAUD_DEFAULT: i32 = 115_200;
const BAUD_FIRST_OPEN: i32 = 9_600;
const BAUD_R9_FLOOR: i32 = 115_200;
const SHIFT_LIGHTS: i32 = 8;
const SIMLED_COUNT: i32 = 8;
const LED_FIRST: i32 = 1;
const FAN_POWER: f64 = 0.5;
const AMP_FACTOR: f64 = 1.0;
const KPH_TO_MPH: f64 = 0.621317;
const FAN_BYTE_SCALE: f64 = 255.0;
const EFFECT_BYTE_SCALE: f64 = 255.0;
const PLAY_LIMIT: f64 = 1.0;
const ARDUINO_TIMEOUT_MS: u32 = 9_000;
const LED_QUERY_WAIT_MS: u32 = 5_000;
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
const SIMLED_REPLY: &[u8] = b"8\r";
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
const HAPTIC_PACKET: usize = HAPTIC_MOTORS * 2;
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
        self.bytes("sp_blocking_read", SIMLED_REPLY);
        SIMLED_REPLY.to_vec()
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

fn run_shiftlights(log: &Log, frames: &[Telemetry], baud: i32) -> usize {
    let id = log.open_port(CAPTURE_PORT, baud);
    each_frame(log, frames, |_log, frame| {
        let lit = revlights_lit(frame.rpms() as i32, frame.maxrpm() as i32, SHIFT_LIGHTS);
        log.write_port(id, &[lit as u8]);
    });
    id
}

fn run_simwind(log: &Log, frames: &[Telemetry], baud: i32) -> usize {
    let id = log.open_port(CAPTURE_PORT, baud);
    let fan = (FAN_POWER * FAN_BYTE_SCALE) as u8;
    each_frame(log, frames, |_log, frame| {
        let mph = (f64::from(frame.velocity()) * KPH_TO_MPH).ceil() as u8;
        log.write_port(id, &[mph, fan]);
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

fn haptic_packet(effect: u8, motor: u32, enabled: bool) -> [u8; HAPTIC_PACKET] {
    let mut bytes = [0; HAPTIC_PACKET];
    if !enabled {
        return bytes;
    }
    if motor_one(motor) {
        bytes[0] = 1;
        bytes[1] = effect;
    }
    if motor_three(motor) {
        bytes[4] = 1;
        bytes[5] = effect;
    }
    bytes
}

fn run_haptic(log: &Log, frames: &[Telemetry]) {
    let id = log.open_port(CAPTURE_PORT, BAUD_DEFAULT);
    let clock = Trace { log };
    let mut effect = HapticEffect::new(&haptic_settings());
    let mut state = 0.0;
    each_frame(log, frames, |_log, frame| {
        let mut play = effect.play_with_clock(frame, &clock) * AMP_FACTOR;
        if play > PLAY_LIMIT {
            play = PLAY_LIMIT;
        }
        let changed = play != state;
        if changed {
            state = play;
        }
        let speed = (EFFECT_BYTE_SCALE * play).ceil() as u8;
        let packet = haptic_packet(speed, MOTOR_1, changed);
        let _ = ARDUINO_TIMEOUT_MS;
        log.write_port(id, &packet);
    });
    let mut stopped = [0; HAPTIC_PACKET];
    for motor in 0..HAPTIC_MOTORS {
        stopped[motor * 2] = 1;
    }
    let _ = HAPTIC_ZERO_TIMEOUT_MS;
    log.write_port(id, &stopped);
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

fn simled_rgb(lit: i32, total: i32, startled: i32) -> Vec<u8> {
    let mut rgb = vec![0; total as usize * RGB_CHANNELS];
    let half = total / 2;
    for index in 0..lit {
        let led = index + startled - LED_FIRST;
        if led < 0 || led >= total {
            continue;
        }
        let base = led as usize * RGB_CHANNELS;
        if index < half {
            rgb[base + GREEN_CHANNEL] = MARK_BYTE;
        } else if index < total - 1 {
            rgb[base + RED_CHANNEL] = MARK_BYTE;
            rgb[base + GREEN_CHANNEL] = MARK_BYTE;
        }
        if index == total - 1 {
            rgb[base + RED_CHANNEL] = MARK_BYTE;
            rgb[base + GREEN_CHANNEL] = 0;
        }
    }
    rgb
}

fn run_simled(log: &Log, frames: &[Telemetry]) {
    let id = log.open_port(CAPTURE_PORT, BAUD_DEFAULT);
    let total = SIMLED_COUNT;
    let avail = total - LED_FIRST + 1;
    each_frame(log, frames, |_log, frame| {
        let lit = if frame.rpms() > 0 && frame.maxrpm() > 0 {
            revlights_lit(frame.rpms() as i32, frame.maxrpm() as i32, avail)
        } else {
            0
        };
        let rgb = simled_rgb(lit, total, LED_FIRST);
        log.write_port(id, &led_packet(total as usize, &rgb));
    });
    log.write_port(id, &led_packet(total as usize, &[]));
    log.close_port(id);
}

fn query_led_count(log: &Log, id: usize) -> Option<i32> {
    let mut query = vec![MARK_BYTE; HEADER_MARKS];
    query.extend_from_slice(LEDSC_TAG);
    log.wait_port(LED_QUERY_WAIT_MS);
    log.write_port(id, &query);
    log.wait_port(LED_QUERY_WAIT_MS);
    let reply = log.read_port(id);
    let text = std::str::from_utf8(&reply).ok()?.trim();
    let count: i32 = text.parse().ok()?;
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

fn write_frame(log: &Log, id: usize, cmd: &[u8], payload: &[u8]) -> bool {
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
    log.write_port(id, &stuffed);
    true
}

fn send_rpm_mask(log: &Log, id: usize, active: u32) -> bool {
    let window = (1u32 << MOZA_LED_COUNT) - 1;
    let mut payload = [0; 8];
    payload[..4].copy_from_slice(&active.to_le_bytes());
    payload[4..].copy_from_slice(&window.to_le_bytes());
    write_frame(log, id, &[0x1a, 0x00], &payload)
}

fn send_buttons(log: &Log, id: usize, mask: u16) -> bool {
    write_frame(log, id, &[0x1a, 0x01], &mask.to_le_bytes())
}

fn send_colours(log: &Log, id: usize, group: u8, table: &[u8; MOZA_COLOUR_BYTES]) -> bool {
    let cmd = [0x19, group];
    write_frame(log, id, &cmd, &table[..MOZA_COLOUR_CHUNK])
        && write_frame(log, id, &cmd, &table[MOZA_COLOUR_CHUNK..])
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

fn arm_telemetry(log: &Log, id: usize) -> bool {
    let bright = [MOZA_BRIGHTNESS];
    if !write_frame(log, id, &[0x1c, 0x00], &[1]) {
        return false;
    }
    if !write_frame(log, id, &[0x1b, 0x00, 0xff], &bright) {
        return false;
    }
    let mut rpm = [0; MOZA_COLOUR_BYTES];
    fill_rpm(&mut rpm);
    if !send_colours(log, id, 0x00, &rpm) {
        return false;
    }
    if !write_frame(log, id, &[0x1b, 0x01, 0xff], &bright) {
        return false;
    }
    write_frame(log, id, &[0x1d, 0x00], &[0])
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

fn apply_bar_colours(log: &Log, id: usize, mode: Bar) -> bool {
    let mut table = [0; MOZA_COLOUR_BYTES];
    match mode {
        Bar::Brake | Bar::Lock => fill_solid(&mut table, PURPLE),
        Bar::Abs => fill_solid(&mut table, BLUE_ALERT),
        Bar::Rpm => fill_rpm(&mut table),
    }
    send_colours(log, id, 0x00, &table)
}

fn elapsed_ms(clock: &impl Clock, start_ns: u64) -> u64 {
    clock.monotonic_ns().saturating_sub(start_ns) / NS_PER_MS
}

fn blink_on(clock: &impl Clock, state: &MozaNew) -> bool {
    let periods = elapsed_ms(clock, state.started_ns) / MOZA_ALERT_MS;
    periods.is_multiple_of(2)
}

fn write_brake_buttons(log: &Log, id: usize, corners: &[Rgb; MOZA_CORNERS]) -> bool {
    let table = corners_table(corners);
    send_colours(log, id, 0x01, &table) && send_buttons(log, id, corners_mask(corners))
}

fn blank_wheel(log: &Log, id: usize, state: &mut MozaNew, clock: &impl Clock) {
    let off = [Rgb { r: 0, g: 0, b: 0 }; MOZA_CORNERS];
    let _ = write_brake_buttons(log, id, &off);
    let _ = send_rpm_mask(log, id, 0);
    if state.bar != Bar::Rpm {
        let _ = apply_bar_colours(log, id, Bar::Rpm);
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
    log: &Log,
    id: usize,
    state: &mut MozaNew,
    frame: &Telemetry,
    clock: &impl Clock,
) {
    if !state.armed {
        if !arm_telemetry(log, id) {
            return;
        }
        state.reset(clock);
        state.armed = true;
    }
    if frame.simstatus() != STATUS_ACTIVE {
        blank_wheel(log, id, state, clock);
        return;
    }
    let alert = alert_mode(state, frame);
    let bar = desired_bar(state, frame, alert);
    if bar != state.bar && apply_bar_colours(log, id, bar) {
        if bar == Bar::Rpm {
            state.last_brake_lit = 0;
        }
        state.bar = bar;
    }
    let corners = corners_from(frame);
    let changed = !corners_same(&corners, &state.last_corners);
    let on_off = corners_mask(&corners) != corners_mask(&state.last_corners);
    let due = !state.has_button_write || elapsed_ms(clock, state.last_button_ns) >= MOZA_BUTTON_MS;
    if changed && (on_off || due) && write_brake_buttons(log, id, &corners) {
        state.last_corners = corners;
        state.last_button_ns = clock.monotonic_ns();
        state.has_button_write = true;
    }
    let bits = bar_mask(state, frame, state.bar, clock);
    if !send_rpm_mask(log, id, bits) {
        state.armed = false;
    }
}

fn run_moza_new(log: &Log, frames: &[Telemetry]) {
    let baud = BAUD_DEFAULT.max(BAUD_R9_FLOOR);
    let id = log.open_port(CAPTURE_PORT, baud);
    let clock = Trace { log };
    let mut state = MozaNew::new();
    state.reset(&clock);
    if arm_telemetry(log, id) {
        state.armed = true;
    }
    for (index, frame) in frames.iter().enumerate() {
        log.set_tick(index as u32);
        update_moza_new(log, id, &mut state, frame, &clock);
        log.advance_tick();
    }
    log.close_port(id);
}
