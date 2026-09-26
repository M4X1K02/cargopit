//! USB device encoders. Output matches the C capture goldens, including quirks.

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString};

use crate::clock::{Clock, VirtualClock};
use crate::haptic::{HapticEffect, HapticSettings, TyreId, VibrationEffect};
use crate::lua_host::{LuaHost, LuaLedMode};
use crate::telemetry::Telemetry;

const ORIGIN_US: u64 = 1_000_000;
const TICK_US: u64 = 16_000;
const CLOCK_MONOTONIC: i32 = 1;
const TACH_POINTS: usize = 9;
pub const TACH_RPM_STEP: u32 = 1000;
pub const TACH_IDLE_RPM: u32 = 500;
const GRANULARITY_DIRECT: u32 = 0;
pub const REVBURNER_LEN: usize = 8;
const REVBURNER_PULSE_LOW: usize = 2;
const REVBURNER_PULSE_HIGH: usize = 3;
const PULSE_BYTE_MASK: u32 = 0xff;
const PULSE_HIGH_SHIFT: u32 = 8;
const REVBURNER_VID: u16 = 0x04d8;
const REVBURNER_PID: u16 = 0x0102;
pub const G29_LEN: usize = 7;
pub const G29_VID: u16 = 0x046d;
pub const G29_PID: u16 = 0xc24f;
pub const G29_BYTE_REPORT: usize = 0;
pub const G29_BYTE_CMD: usize = 1;
pub const G29_BYTE_LEDS: usize = 2;
pub const G29_BYTE_PAD: usize = 3;
pub const G29_BYTE_TAIL: usize = 6;
const G29_REPORT: u8 = 0xf8;
const G29_CMD: u8 = 0x12;
const G29_TAIL: u8 = 0x01;
const G29_LED5: f32 = 0.84;
const G29_LED4: f32 = 0.69;
const G29_LED3: f32 = 0.39;
const G29_LED2: f32 = 0.19;
const G29_LED1: f32 = 0.4;
pub const C5_LEN: usize = 14;
pub const C5_VID: u16 = 0x3416;
pub const C5_PID: u16 = 0x1021;
pub const C5_BYTE_REPORT: usize = 0;
pub const C5_BYTE_LEDS: usize = 1;
pub const C5_BYTE_VELOCITY_HIGH: usize = 2;
pub const C5_BYTE_VELOCITY_LOW: usize = 3;
pub const C5_BYTE_GEAR: usize = 4;
const C5_REPORT: u8 = 0xfc;
const C5_LEDS: i32 = 9;
const C5_LED_STEPS: i32 = C5_LEDS + 1;
const C5_VELOCITY_SHIFT: u32 = 8;
const C5_BYTE_MASK: u32 = 0xff;
pub const C12_LEN: usize = 16;
pub const C12_VID: u16 = 0x3416;
pub const C12_PID: u16 = 0x1023;
pub const C12_BYTE_MARK0: usize = 0;
pub const C12_BYTE_MARK1: usize = 1;
pub const C12_BYTE_MARK2: usize = 2;
pub const C12_BYTE_PERCENT: usize = 3;
pub const C12_BYTE_VELOCITY_LOW: usize = 4;
pub const C12_BYTE_VELOCITY_HIGH: usize = 5;
pub const C12_BYTE_GEAR: usize = 6;
/// Bytes the Lua path fills. C calls `hid_write` with `C12_LEN` and reads past this array.
pub const C12_LED_LEN: usize = 7;
pub const C12_LED_TOTAL: i64 = 23;
pub const C12_LED_FIRST: usize = 15;
pub const C12_BYTE_LED: usize = 3;
pub const C12_BYTE_RED: usize = 4;
pub const C12_BYTE_GREEN: usize = 5;
pub const C12_BYTE_BLUE: usize = 6;
const C12_LED_MODE: u8 = 0x02;
pub const C12_LED_NUMBER_OFFSET: usize = 1;
const C12_RGB_GREEN: usize = 1;
const C12_RGB_BLUE: usize = 2;
const C12_B0: u8 = 0xfa;
const C12_B1: u8 = 0xfb;
const C12_B2: u8 = 0xd4;
const C12_PERCENT: f64 = 100.0;
const C12_VELOCITY_SHIFT: u32 = 8;
const C12_BYTE_MASK: u32 = 0xff;
const REDLINE_MARGIN: f64 = 0.05;
pub const GT_NEO_VID: u16 = 0x3670;
pub const GT_NEO_PID: u16 = 0x0805;
pub const GT_NEO_LEDS: i64 = 73;
pub const GT_NEO_LUA_FIRST: i64 = 1;
pub const GT_NEO_PAYLOAD: usize = 64;
pub const GT_NEO_REPORT: u8 = 0xf0;
pub const GT_NEO_CONTROL: u8 = 0xec;
pub const GT_NEO_PREPARE: u8 = 0x02;
pub const GT_NEO_SET: u8 = 0x03;
const GT_NEO_PREPARE_COUNT: u8 = 0x01;
const GT_NEO_BYTE_REPORT: usize = 0;
const GT_NEO_BYTE_CONTROL: usize = 6;
const GT_NEO_BYTE_ACTION: usize = 7;
const GT_NEO_BYTE_COUNT: usize = 8;
const GT_NEO_BYTE_LED: usize = 9;
const GT_NEO_HEADER: usize = GT_NEO_BYTE_LED;
const GT_NEO_PACKET_RED: usize = 1;
const GT_NEO_PACKET_GREEN: usize = 2;
const GT_NEO_PACKET_BLUE: usize = 3;
const GT_NEO_COLOR_GREEN: usize = 1;
const GT_NEO_COLOR_BLUE: usize = 2;
const LED_STRIDE: usize = 4;
const RGB_CHANNELS: usize = 3;
pub const P1000_LEN: usize = 49;
pub const P1000_VID: u16 = 0x0483;
pub const P1000_PID: u16 = 0x0525;
pub const P1000_MARK: u8 = 241;
pub const P1000_BYTE_MARK: usize = 0;
pub const P1000_BYTE_CMD: usize = 1;
pub const P1000_BYTE_KIND: usize = 2;
pub const P1000_BYTE_FLAG0: usize = 3;
pub const P1000_BYTE_FLAG1: usize = 4;
pub const P1000_BYTE_FLAG2: usize = 5;
pub const P1000_BYTE_INIT0: usize = 6;
pub const P1000_BYTE_INIT1: usize = 7;
pub const P1000_CLEARED: u8 = 0;
pub const P1000_CMD_IDLE: u8 = P1000_CLEARED;
pub const P1000_CMD_ACTIVE: u8 = 0xec;
pub const P1000_CMD_INIT: u8 = 0xf1;
pub const P1000_KIND_LOCK: u8 = 0x01;
pub const P1000_KIND_SLIP: u8 = 0x02;
pub const P1000_FLAG0: u8 = 0x01;
pub const P1000_FLAG1: u8 = 0x0a;
pub const P1000_FLAG2: u8 = 0xff;
pub const P1000_INIT_MODE: u8 = 0x17;
pub const P1000_INIT_A: u8 = 0x01;
pub const P1000_INIT_B: u8 = 0x02;
pub const P1000_TRACE_LEN: usize = 7;
pub const P1000_REPORT_OK: i32 = 0;
pub const P1000_REPORT_FAILED: i32 = 1;
const SIMNET_LEN: usize = 64;
const SIMNET_VID: u16 = 0xcafe;
const SIMNET_PID: u16 = 0xa301;
const SIMNET_REPORT: u8 = 0x01;
const HAPTIC_HZ: u32 = 40;
const HAPTIC_AMP: u32 = 100;
const HAPTIC_THRESHOLD: f64 = 0.2;
const HAPTIC_DURATION_S: f64 = 0.10;
pub const CSL_RUMBLE_OFF: i32 = 0;
pub const CSL_RUMBLE_SLIP: i32 = 0x00ff_0000;
pub const CSL_RUMBLE_LOCK: i32 = 0x0000_ff00;
pub const CSL_RUMBLE_ABS: i32 = 0x00ff_ff00;
pub const CSL_SYSFS_GLOB: &str = "/sys/module/hid_fanatec/drivers/hid:f*/0003:0EB7:183B.*/rumble";
const CLOCK_OP: &str = concat!("clock_", "gettime");
const WALL_OP: &str = concat!("get", "timeofday");
const CSL_PATH: &str = "/sys/module/hid_fanatec/drivers/hid:fake/0003:0EB7:183B.0001/rumble";

pub const USB_DEVICES: &[&str] = &[
    "revburner",
    "revburner_granularity_2",
    "revburner_granularity_4",
    "revburner_pulses",
    "cammus_c5",
    "cammus_c12",
    "logitech_g29",
    "simagic_gt_neo",
    "csl_elite_v3",
    "simagic_p1000",
    "simnet_pedals",
];

struct Log {
    tick: Cell<u32>,
    lines: RefCell<String>,
    clock: RefCell<VirtualClock>,
    hid_id: Cell<u32>,
}

struct Trace<'a> {
    log: &'a Log,
}

impl Log {
    fn new() -> Self {
        let mut clock = VirtualClock::new();
        clock.advance_us(ORIGIN_US);
        Self {
            tick: Cell::new(0),
            lines: RefCell::new(String::new()),
            clock: RefCell::new(clock),
            hid_id: Cell::new(0),
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

    fn hid_open(&self, vid: u16, pid: u16) -> u32 {
        self.op("hid_init", "");
        self.hid_id.set(self.hid_id.get().saturating_add(1));
        let id = self.hid_id.get();
        self.op(
            "hid_open",
            &format!("id={id} vid={vid:#06x} pid={pid:#06x}"),
        );
        id
    }

    fn hid_write(&self, id: u32, data: &[u8]) {
        self.bytes(&format!("hid_write id={id}"), data);
    }

    fn hid_feature(&self, id: u32, data: &[u8]) {
        self.bytes(&format!("hid_send_feature_report id={id}"), data);
    }

    fn hid_close(&self, id: u32) {
        self.op("hid_close", &format!("id={id}"));
        self.op("hid_exit", "");
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

pub fn capture_usb(name: &str, frames: &[Telemetry], lua_source: &str) -> Option<String> {
    let log = Log::new();
    match name {
        "revburner" => run_tach(&log, frames, 1, false),
        "revburner_granularity_2" => run_tach(&log, frames, 2, false),
        "revburner_granularity_4" => run_tach(&log, frames, 4, false),
        "revburner_pulses" => run_tach(&log, frames, 1, true),
        "logitech_g29" => run_g29(&log, frames),
        "cammus_c5" => run_c5(&log, frames),
        "cammus_c12" => run_c12(&log, frames),
        "simagic_gt_neo" => run_gt_neo(&log, frames, lua_source)?,
        "csl_elite_v3" => run_csl(&log, frames),
        "simagic_p1000" => run_p1000(&log, frames),
        "simnet_pedals" => run_simnet(&log, frames),
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

fn tach_table(granularity: u32) -> [u32; TACH_POINTS] {
    let mut pulses = [0; TACH_POINTS];
    for (index, slot) in pulses.iter_mut().enumerate() {
        *slot = (index as u32 + 1) * granularity;
    }
    pulses
}

pub enum TachPulses {
    Frame(u32),
    Idle(u32),
    Indexed { pulses: u32, element: u32 },
}

impl TachPulses {
    pub fn pulses(&self) -> u32 {
        match self {
            Self::Frame(pulses) | Self::Idle(pulses) | Self::Indexed { pulses, .. } => *pulses,
        }
    }
}

pub fn lookup_tach_pulses(
    rpms: u32,
    frame_pulses: u32,
    granularity: u32,
    table: &[u32],
    use_pulses: bool,
) -> Option<TachPulses> {
    if use_pulses {
        return Some(TachPulses::Frame(frame_pulses));
    }
    let &idle = table.first()?;
    if rpms < TACH_IDLE_RPM {
        return Some(TachPulses::Idle(idle));
    }
    let element = tach_element(rpms, granularity, table.len());
    Some(TachPulses::Indexed {
        pulses: table[element as usize],
        element,
    })
}

fn tach_element(rpms: u32, granularity: u32, len: usize) -> u32 {
    let mut element = rpms / TACH_RPM_STEP;
    if granularity > GRANULARITY_DIRECT {
        let step = TACH_RPM_STEP / granularity;
        if let Some(indexed) = rpms.checked_div(step) {
            element = indexed;
        }
    }
    let last = (len - 1) as u32;
    if element >= last {
        return last;
    }
    element
}

pub fn revburner_report(pulses: u32) -> [u8; REVBURNER_LEN] {
    let mut bytes = [0; REVBURNER_LEN];
    if pulses > 0 {
        bytes[REVBURNER_PULSE_LOW] = (pulses & PULSE_BYTE_MASK) as u8;
        bytes[REVBURNER_PULSE_HIGH] = ((pulses >> PULSE_HIGH_SHIFT) & PULSE_BYTE_MASK) as u8;
    }
    bytes
}

fn tach_pulses(
    frame: &Telemetry,
    granularity: u32,
    table: &[u32; TACH_POINTS],
    use_pulses: bool,
) -> u32 {
    lookup_tach_pulses(frame.rpms(), frame.pulses(), granularity, table, use_pulses)
        .map(|sample| sample.pulses())
        .unwrap_or(0)
}

fn revburner_bytes(pulses: u32) -> [u8; REVBURNER_LEN] {
    revburner_report(pulses)
}

fn run_tach(log: &Log, frames: &[Telemetry], granularity: u32, use_pulses: bool) {
    let id = log.hid_open(REVBURNER_VID, REVBURNER_PID);
    let table = tach_table(granularity);
    each_frame(log, frames, |log, frame| {
        let pulses = tach_pulses(frame, granularity, &table, use_pulses);
        log.hid_write(id, &revburner_bytes(pulses));
    });
    log.hid_write(id, &revburner_bytes(0));
    log.hid_close(id);
}

fn g29_leds(rpm: u32, maxrpm: u32) -> u8 {
    if maxrpm == 0 {
        return 0;
    }
    let percent = rpm as f32 / maxrpm as f32;
    if percent > G29_LED5 {
        return 0b11111;
    }
    if percent > G29_LED4 {
        return 0b1111;
    }
    if percent > G29_LED3 {
        return 0b111;
    }
    if percent > G29_LED2 {
        return 0b11;
    }
    if percent > G29_LED1 {
        return 0b1;
    }
    0
}

pub fn g29_report(rpm: u32, maxrpm: u32) -> [u8; G29_LEN] {
    [
        G29_REPORT,
        G29_CMD,
        g29_leds(rpm, maxrpm),
        0,
        0,
        0,
        G29_TAIL,
    ]
}

fn run_g29(log: &Log, frames: &[Telemetry]) {
    let id = log.hid_open(G29_VID, G29_PID);
    each_frame(log, frames, |log, frame| {
        log.hid_write(id, &g29_report(frame.rpms(), frame.maxrpm()));
    });
    log.hid_write(id, &g29_report(0, 0));
    log.hid_close(id);
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
    let lit = rpm / interval;
    if lit > steps {
        return steps;
    }
    lit
}

pub fn c5_report(rpm: u32, maxrpm: u32, gear: u32, velocity: u32) -> [u8; C5_LEN] {
    let mut bytes = [0; C5_LEN];
    bytes[C5_BYTE_REPORT] = C5_REPORT;
    bytes[C5_BYTE_LEDS] = revlights_lit(rpm as i32, maxrpm as i32, C5_LED_STEPS) as u8;
    if velocity > 0 {
        bytes[C5_BYTE_VELOCITY_HIGH] = ((velocity >> C5_VELOCITY_SHIFT) & C5_BYTE_MASK) as u8;
        bytes[C5_BYTE_VELOCITY_LOW] = (velocity & C5_BYTE_MASK) as u8;
    }
    bytes[C5_BYTE_GEAR] = (gear as u8).wrapping_sub(1);
    bytes
}

fn run_c5(log: &Log, frames: &[Telemetry]) {
    let id = log.hid_open(C5_VID, C5_PID);
    each_frame(log, frames, |log, frame| {
        log.hid_write(
            id,
            &c5_report(frame.rpms(), frame.maxrpm(), frame.gear(), frame.velocity()),
        );
    });
    log.hid_write(id, &c5_report(0, 0, 0, 0));
    log.hid_close(id);
}

pub fn c12_report(rpm: u32, maxrpm: u32, gear: u32, velocity: u32) -> [u8; C12_LEN] {
    let mut bytes = [0; C12_LEN];
    bytes[C12_BYTE_MARK0] = C12_B0;
    bytes[C12_BYTE_MARK1] = C12_B1;
    bytes[C12_BYTE_MARK2] = C12_B2;
    if rpm > 0 && maxrpm > 0 {
        let percent = f64::from(rpm) / f64::from(maxrpm) * C12_PERCENT;
        bytes[C12_BYTE_PERCENT] = percent.round_ties_even() as u8;
    }
    if velocity > 0 {
        bytes[C12_BYTE_VELOCITY_LOW] = (velocity & C12_BYTE_MASK) as u8;
        bytes[C12_BYTE_VELOCITY_HIGH] = ((velocity >> C12_VELOCITY_SHIFT) & C12_BYTE_MASK) as u8;
    }
    bytes[C12_BYTE_GEAR] = (gear as u8).wrapping_sub(1);
    bytes
}

pub fn c12_led_color_len() -> usize {
    usize::try_from(C12_LED_TOTAL)
        .unwrap_or(0)
        .saturating_mul(RGB_CHANNELS)
}

pub fn c12_led_color_index(led_index: usize) -> usize {
    led_index.saturating_mul(RGB_CHANNELS)
}

pub fn c12_led_reports(colors: &[u8]) -> Vec<[u8; C12_LED_LEN]> {
    let total = usize::try_from(C12_LED_TOTAL).unwrap_or(0);
    let mut reports = Vec::new();
    let mut index = C12_LED_FIRST;
    while index < total {
        reports.push(c12_one_led(colors, index));
        index += 1;
    }
    reports
}

fn c12_one_led(colors: &[u8], index: usize) -> [u8; C12_LED_LEN] {
    let mut bytes = [0; C12_LED_LEN];
    bytes[C12_BYTE_MARK0] = C12_B0;
    bytes[C12_BYTE_MARK1] = C12_B1;
    bytes[C12_BYTE_MARK2] = C12_LED_MODE;
    let number = index.saturating_add(C12_LED_NUMBER_OFFSET);
    bytes[C12_BYTE_LED] = u8::try_from(number).unwrap_or(u8::MAX);
    let base = index.saturating_mul(RGB_CHANNELS);
    if let Some(red) = colors.get(base) {
        bytes[C12_BYTE_RED] = *red;
    }
    if let Some(green) = colors.get(base.saturating_add(C12_RGB_GREEN)) {
        bytes[C12_BYTE_GREEN] = *green;
    }
    if let Some(blue) = colors.get(base.saturating_add(C12_RGB_BLUE)) {
        bytes[C12_BYTE_BLUE] = *blue;
    }
    bytes
}

fn run_c12(log: &Log, frames: &[Telemetry]) {
    let id = log.hid_open(C12_VID, C12_PID);
    each_frame(log, frames, |log, frame| {
        log.hid_write(
            id,
            &c12_report(frame.rpms(), frame.maxrpm(), frame.gear(), frame.velocity()),
        );
    });
    log.hid_close(id);
}

pub fn gt_neo_color_len() -> usize {
    usize::try_from(GT_NEO_LEDS)
        .unwrap_or(0)
        .saturating_mul(RGB_CHANNELS)
}

pub fn gt_neo_reports(colors: &[u8]) -> Vec<[u8; GT_NEO_PAYLOAD]> {
    let mut reports = vec![gt_prepare()];
    let total = usize::try_from(GT_NEO_LEDS).unwrap_or(0);
    let mut led = 0;
    while led < total {
        let (report, count) = gt_chunk(colors, led);
        if count == 0 {
            break;
        }
        reports.push(report);
        led += count;
    }
    reports
}

fn gt_prepare() -> [u8; GT_NEO_PAYLOAD] {
    let mut report = [0; GT_NEO_PAYLOAD];
    report[GT_NEO_BYTE_REPORT] = GT_NEO_REPORT;
    report[GT_NEO_BYTE_CONTROL] = GT_NEO_CONTROL;
    report[GT_NEO_BYTE_ACTION] = GT_NEO_PREPARE;
    report[GT_NEO_BYTE_COUNT] = GT_NEO_PREPARE_COUNT;
    report
}

fn gt_chunk(colors: &[u8], start_led: usize) -> ([u8; GT_NEO_PAYLOAD], usize) {
    let room = (GT_NEO_PAYLOAD - 1 - GT_NEO_HEADER) / LED_STRIDE;
    let mut report = [0; GT_NEO_PAYLOAD];
    report[GT_NEO_BYTE_REPORT] = GT_NEO_REPORT;
    report[GT_NEO_BYTE_CONTROL] = GT_NEO_CONTROL;
    report[GT_NEO_BYTE_ACTION] = GT_NEO_SET;
    let total = usize::try_from(GT_NEO_LEDS).unwrap_or(0);
    let mut count = 0;
    while count < room && start_led + count < total {
        let led = start_led + count;
        let base = GT_NEO_BYTE_LED + count * LED_STRIDE;
        report[base] = u8::try_from(led).unwrap_or(u8::MAX);
        let color = led * RGB_CHANNELS;
        if color + GT_NEO_COLOR_BLUE < colors.len() {
            report[base + GT_NEO_PACKET_RED] = colors[color];
            report[base + GT_NEO_PACKET_GREEN] = colors[color + GT_NEO_COLOR_GREEN];
            report[base + GT_NEO_PACKET_BLUE] = colors[color + GT_NEO_COLOR_BLUE];
        }
        count += 1;
        report[GT_NEO_BYTE_COUNT] = u8::try_from(count).unwrap_or(u8::MAX);
    }
    (report, count)
}

fn run_gt_neo(log: &Log, frames: &[Telemetry], lua_source: &str) -> Option<()> {
    let id = log.hid_open(GT_NEO_VID, GT_NEO_PID);
    let mut host = LuaHost::load(lua_source, LuaLedMode::Usb).ok()?;
    let trace = Trace { log };
    each_frame(log, frames, |log, frame| {
        let mut sim = frame.clone_buf();
        let tick = host.call(&mut sim, GT_NEO_LEDS, &trace).ok();
        let colors = tick.map(|tick| tick.leds).unwrap_or_default();
        for report in gt_neo_reports(&colors) {
            log.hid_feature(id, &report);
        }
    });
    Some(())
}

pub enum CslLocate {
    Found(String),
    Missing,
    Permission,
}

pub fn csl_rumble_value(effect: VibrationEffect, play: f64) -> i32 {
    if play <= 0.0 {
        return CSL_RUMBLE_OFF;
    }
    match effect {
        VibrationEffect::TyreSlip => CSL_RUMBLE_SLIP,
        VibrationEffect::TyreLock => CSL_RUMBLE_LOCK,
        VibrationEffect::AbsBrakes => CSL_RUMBLE_ABS,
        VibrationEffect::EngineRpm | VibrationEffect::GearShift | VibrationEffect::Suspension => {
            CSL_RUMBLE_OFF
        }
    }
}

pub fn csl_rumble_text(effect: VibrationEffect, play: f64) -> String {
    format!("{}\n", csl_rumble_value(effect, play))
}

pub fn locate_csl_pedals() -> CslLocate {
    let Ok(pattern) = CString::new(CSL_SYSFS_GLOB) else {
        return CslLocate::Missing;
    };
    let mut paths: libc::glob_t = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::glob(pattern.as_ptr(), libc::GLOB_PERIOD, None, &mut paths) };
    let located = csl_from_glob(rc, &paths);
    unsafe { libc::globfree(&mut paths) };
    located
}

fn csl_from_glob(rc: i32, paths: &libc::glob_t) -> CslLocate {
    if rc == libc::GLOB_ABORTED {
        return CslLocate::Permission;
    }
    if rc != 0 || paths.gl_pathc == 0 || paths.gl_pathv.is_null() {
        return CslLocate::Missing;
    }
    let first = unsafe { *paths.gl_pathv };
    if first.is_null() {
        return CslLocate::Missing;
    }
    let Ok(path) = unsafe { CStr::from_ptr(first) }.to_str() else {
        return CslLocate::Missing;
    };
    if path.is_empty() {
        return CslLocate::Missing;
    }
    CslLocate::Found(path.to_string())
}

fn slip_effect() -> HapticEffect {
    HapticEffect::new(&HapticSettings {
        effect: VibrationEffect::TyreSlip,
        tyre: TyreId::AllFour,
        threshold: HAPTIC_THRESHOLD,
        frequency: HAPTIC_HZ,
        amplitude: HAPTIC_AMP,
        duration: HAPTIC_DURATION_S,
        ..HapticSettings::default()
    })
}

fn run_csl(log: &Log, frames: &[Telemetry]) {
    log.op("glob", CSL_PATH);
    log.op("fopen", CSL_PATH);
    let mut state = 0.0;
    let mut effect = slip_effect();
    let trace = Trace { log };
    each_frame(log, frames, |log, frame| {
        let play = effect.play_with_clock(frame, &trace);
        if play == state {
            return;
        }
        let text = csl_rumble_text(VibrationEffect::TyreSlip, play);
        log.op("sysfs_write", CSL_PATH);
        log.bytes("sysfs_value", text.as_bytes());
        state = play;
    });
    log.op("sysfs_close", CSL_PATH);
}

pub fn p1000_init_report() -> [u8; P1000_LEN] {
    let mut bytes = [0; P1000_LEN];
    bytes[P1000_BYTE_MARK] = P1000_MARK;
    bytes[P1000_BYTE_CMD] = P1000_CMD_INIT;
    bytes[P1000_BYTE_KIND] = P1000_INIT_MODE;
    bytes[P1000_BYTE_INIT0] = P1000_INIT_A;
    bytes[P1000_BYTE_INIT1] = P1000_INIT_B;
    bytes
}

pub fn p1000_active_report(kind: u8) -> [u8; P1000_LEN] {
    let mut bytes = [0; P1000_LEN];
    bytes[P1000_BYTE_MARK] = P1000_MARK;
    bytes[P1000_BYTE_CMD] = P1000_CMD_ACTIVE;
    bytes[P1000_BYTE_KIND] = kind;
    bytes[P1000_BYTE_FLAG0] = P1000_FLAG0;
    bytes[P1000_BYTE_FLAG1] = P1000_FLAG1;
    bytes[P1000_BYTE_FLAG2] = P1000_FLAG2;
    bytes
}

pub fn p1000_idle_report(kind: u8) -> [u8; P1000_LEN] {
    let mut bytes = [0; P1000_LEN];
    bytes[P1000_BYTE_MARK] = P1000_MARK;
    bytes[P1000_BYTE_CMD] = P1000_CMD_IDLE;
    bytes[P1000_BYTE_KIND] = kind;
    bytes
}

pub fn p1000_reports(effect: VibrationEffect, play: f64) -> Vec<[u8; P1000_LEN]> {
    if play <= 0.0 {
        return vec![
            p1000_idle_report(P1000_KIND_LOCK),
            p1000_idle_report(P1000_KIND_SLIP),
        ];
    }
    match effect {
        VibrationEffect::TyreSlip => vec![p1000_active_report(P1000_KIND_SLIP)],
        VibrationEffect::TyreLock => vec![p1000_active_report(P1000_KIND_LOCK)],
        VibrationEffect::AbsBrakes => vec![
            p1000_active_report(P1000_KIND_SLIP),
            p1000_active_report(P1000_KIND_LOCK),
        ],
        VibrationEffect::EngineRpm | VibrationEffect::GearShift | VibrationEffect::Suspension => {
            Vec::new()
        }
    }
}

fn run_p1000(log: &Log, frames: &[Telemetry]) {
    let id = log.hid_open(P1000_VID, P1000_PID);
    log.hid_feature(id, &p1000_init_report());
    let mut state = 0.0;
    let mut effect = slip_effect();
    let trace = Trace { log };
    each_frame(log, frames, |log, frame| {
        let play = effect.play_with_clock(frame, &trace);
        if play == state {
            return;
        }
        for report in p1000_reports(VibrationEffect::TyreSlip, play) {
            log.hid_feature(id, &report);
        }
        state = play;
    });
    log.hid_close(id);
}

fn simnet_bytes(play: bool) -> [u8; SIMNET_LEN] {
    let mut bytes = [0; SIMNET_LEN];
    bytes[0] = SIMNET_REPORT;
    if play {
        let multiple = 0u8.wrapping_sub(1) as usize;
        let index = 3 + multiple * 2;
        if index + 1 < SIMNET_LEN {
            bytes[index] = HAPTIC_HZ as u8;
            bytes[index + 1] = HAPTIC_AMP as u8;
        }
    }
    bytes
}

fn run_simnet(log: &Log, frames: &[Telemetry]) {
    let id = log.hid_open(SIMNET_VID, SIMNET_PID);
    let mut state = 0.0;
    let mut effect = slip_effect();
    let trace = Trace { log };
    each_frame(log, frames, |log, frame| {
        let play = effect.play_with_clock(frame, &trace);
        if play != state {
            log.hid_write(id, &simnet_bytes(play > 0.0));
            state = play;
        }
    });
    log.hid_close(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE_IDLE: u32 = 10;
    const TABLE_NEXT: u32 = 20;
    const FRAME_PULSES: u32 = 7;
    const RPM_IDLE: u32 = 0;
    const RPM_INDEXED: u32 = 1000;
    const GRANULARITY_ONE: u32 = 1;
    const REPORT_PULSES: u32 = 0x0102;
    const REPORT_LOW: u8 = 0x02;
    const REPORT_HIGH: u8 = 0x01;
    const INDEX_SECOND: u32 = 1;

    #[test]
    fn revburner_report_packs_pulses_like_the_c_update() {
        let idle = [TABLE_IDLE, TABLE_NEXT];
        let sample = lookup_tach_pulses(RPM_IDLE, FRAME_PULSES, GRANULARITY_ONE, &idle, false)
            .expect("idle");
        assert_eq!(sample.pulses(), TABLE_IDLE);
        let indexed = lookup_tach_pulses(RPM_INDEXED, FRAME_PULSES, GRANULARITY_ONE, &idle, false)
            .expect("index");
        assert!(matches!(
            indexed,
            TachPulses::Indexed {
                element: INDEX_SECOND,
                ..
            }
        ));
        let direct =
            lookup_tach_pulses(RPM_INDEXED, FRAME_PULSES, GRANULARITY_DIRECT, &idle, false)
                .expect("direct");
        assert!(matches!(
            direct,
            TachPulses::Indexed {
                element: INDEX_SECOND,
                ..
            }
        ));
        let from_frame =
            lookup_tach_pulses(RPM_INDEXED, FRAME_PULSES, GRANULARITY_ONE, &idle, true)
                .expect("frame");
        assert_eq!(from_frame.pulses(), FRAME_PULSES);
        assert!(lookup_tach_pulses(RPM_IDLE, RPM_IDLE, GRANULARITY_ONE, &[], false).is_none());
        let report = revburner_report(REPORT_PULSES);
        assert_eq!(report[REVBURNER_PULSE_LOW], REPORT_LOW);
        assert_eq!(report[REVBURNER_PULSE_HIGH], REPORT_HIGH);
        assert_eq!(revburner_report(0), [0; REVBURNER_LEN]);
    }

    #[test]
    fn c12_led_reports_start_at_the_shift_lights() {
        const LED_RED: u8 = 0xff;
        const LED_BLUE: u8 = 0xff;
        let total = usize::try_from(C12_LED_TOTAL).unwrap_or(0);
        let mut colors = vec![0u8; total * RGB_CHANNELS];
        let first = C12_LED_FIRST * RGB_CHANNELS;
        let last = (total - 1) * RGB_CHANNELS;
        colors[first] = LED_RED;
        colors[last + C12_RGB_BLUE] = LED_BLUE;
        let reports = c12_led_reports(&colors);
        assert_eq!(reports.len(), total - C12_LED_FIRST);
        assert_eq!(reports[0][C12_BYTE_MARK0], C12_B0);
        assert_eq!(reports[0][C12_BYTE_MARK1], C12_B1);
        assert_eq!(reports[0][C12_BYTE_MARK2], C12_LED_MODE);
        assert_eq!(
            reports[0][C12_BYTE_LED],
            u8::try_from(C12_LED_FIRST + C12_LED_NUMBER_OFFSET).unwrap_or(u8::MAX)
        );
        assert_eq!(reports[0][C12_BYTE_RED], LED_RED);
        assert_eq!(reports[0][C12_BYTE_GREEN], 0);
        assert_eq!(reports[0][C12_BYTE_BLUE], 0);
        let tail = reports.len() - 1;
        assert_eq!(reports[tail][C12_BYTE_BLUE], LED_BLUE);
        assert_eq!(
            reports[tail][C12_BYTE_LED],
            u8::try_from(total).unwrap_or(u8::MAX)
        );
        assert_eq!(reports[0].len(), C12_LED_LEN);
    }

    #[test]
    fn gt_neo_feature_reports_pack_every_led() {
        const LED_RED: u8 = 0xff;
        let mut colors = vec![0u8; gt_neo_color_len()];
        colors[0] = LED_RED;
        let reports = gt_neo_reports(&colors);
        assert!(reports.len() > 1);
        assert_eq!(reports[0][GT_NEO_BYTE_REPORT], GT_NEO_REPORT);
        assert_eq!(reports[0][GT_NEO_BYTE_CONTROL], GT_NEO_CONTROL);
        assert_eq!(reports[0][GT_NEO_BYTE_ACTION], GT_NEO_PREPARE);
        assert_eq!(reports[0][GT_NEO_BYTE_COUNT], GT_NEO_PREPARE_COUNT);
        assert_eq!(reports[1][GT_NEO_BYTE_ACTION], GT_NEO_SET);
        assert_eq!(reports[1][GT_NEO_BYTE_LED], 0);
        assert_eq!(reports[1][GT_NEO_BYTE_LED + GT_NEO_PACKET_RED], LED_RED);
        let total = usize::try_from(GT_NEO_LEDS).unwrap_or(0);
        let room = (GT_NEO_PAYLOAD - 1 - GT_NEO_HEADER) / LED_STRIDE;
        let chunks = total.div_ceil(room);
        assert_eq!(reports.len(), chunks + 1);
    }

    #[test]
    fn csl_rumble_values_follow_the_effect() {
        const PLAYING: f64 = 1.0;
        const IDLE: f64 = 0.0;
        assert_eq!(
            csl_rumble_value(VibrationEffect::TyreSlip, PLAYING),
            CSL_RUMBLE_SLIP
        );
        assert_eq!(
            csl_rumble_value(VibrationEffect::TyreLock, PLAYING),
            CSL_RUMBLE_LOCK
        );
        assert_eq!(
            csl_rumble_value(VibrationEffect::AbsBrakes, PLAYING),
            CSL_RUMBLE_ABS
        );
        assert_eq!(
            csl_rumble_value(VibrationEffect::TyreSlip, IDLE),
            CSL_RUMBLE_OFF
        );
        assert_eq!(
            csl_rumble_value(VibrationEffect::Suspension, PLAYING),
            CSL_RUMBLE_OFF
        );
        assert_eq!(
            csl_rumble_text(VibrationEffect::TyreSlip, PLAYING),
            format!("{CSL_RUMBLE_SLIP}\n")
        );
    }

    #[test]
    fn p1000_reports_follow_the_effect() {
        const PLAYING: f64 = 1.0;
        const IDLE: f64 = 0.0;
        let init = p1000_init_report();
        assert_eq!(init[P1000_BYTE_MARK], P1000_MARK);
        assert_eq!(init[P1000_BYTE_CMD], P1000_CMD_INIT);
        assert_eq!(init[P1000_BYTE_KIND], P1000_INIT_MODE);
        assert_eq!(init[P1000_BYTE_INIT0], P1000_INIT_A);
        assert_eq!(init[P1000_BYTE_INIT1], P1000_INIT_B);
        let slip = p1000_reports(VibrationEffect::TyreSlip, PLAYING);
        assert_eq!(slip.len(), 1);
        assert_eq!(slip[0][P1000_BYTE_CMD], P1000_CMD_ACTIVE);
        assert_eq!(slip[0][P1000_BYTE_KIND], P1000_KIND_SLIP);
        assert_eq!(slip[0][P1000_BYTE_FLAG2], P1000_FLAG2);
        let lock = p1000_reports(VibrationEffect::TyreLock, PLAYING);
        assert_eq!(lock[0][P1000_BYTE_KIND], P1000_KIND_LOCK);
        let abs = p1000_reports(VibrationEffect::AbsBrakes, PLAYING);
        assert_eq!(abs.len(), 2);
        assert_eq!(abs[0][P1000_BYTE_KIND], P1000_KIND_SLIP);
        assert_eq!(abs[1][P1000_BYTE_KIND], P1000_KIND_LOCK);
        let idle = p1000_reports(VibrationEffect::TyreSlip, IDLE);
        assert_eq!(idle.len(), 2);
        assert_eq!(idle[0][P1000_BYTE_CMD], P1000_CMD_IDLE);
        assert_eq!(idle[0][P1000_BYTE_KIND], P1000_KIND_LOCK);
        assert_eq!(idle[1][P1000_BYTE_KIND], P1000_KIND_SLIP);
        assert_eq!(idle[0][P1000_BYTE_FLAG0], P1000_CLEARED);
        assert!(p1000_reports(VibrationEffect::Suspension, PLAYING).is_empty());
    }
}
