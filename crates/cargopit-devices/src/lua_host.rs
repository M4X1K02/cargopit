//! Shared Lua LED host. Serial registers `led_clear_range` as `led_clear_all`.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, Result as LuaResult, Value};

use crate::clock::Clock;
use crate::haptic::proximity_car_count;
use crate::telemetry::Telemetry;
use simapi_sys::WHEEL_COUNT;

const COLOR_RED: i64 = 1;
const COLOR_GREEN: i64 = 2;
const COLOR_BLUE: i64 = 3;
const COLOR_YELLOW: i64 = 4;
const COLOR_ORANGE: i64 = 5;
const RGB_RED: usize = 0;
const RGB_GREEN: usize = 1;
const RGB_BLUE: usize = 2;
const RGB_CHANNELS: usize = 3;
const LED_FULL: u8 = u8::MAX;
const LED_ORANGE_GREEN: u8 = 165;
const MESSAGE_GLOBAL: &str = "Message";
const FN_SET_LED_RANGE_TO_COLOR: &str = "set_led_range_to_color";
const FN_SET_LED_RANGE_TO_RGB: &str = "set_led_range_to_rgb_color";
const FN_SET_LED_TO_RGB: &str = "set_led_to_rgb_color";
const FN_LED_CLEAR_ALL: &str = "led_clear_all";
const MSG_INVALID_RANGE: &str = "Invalid range, doing nothing";
const EMPTY_LED_BYTE: u8 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LuaLedMode {
    /// USB wheels register `led_clear_all` only.
    Usb,
    /// Serial devices register `led_clear_range` as the clear-all function.
    Serial,
}

pub struct LuaHost {
    lua: Lua,
    leds: Rc<RefCell<Vec<u8>>>,
    logs: Rc<RefCell<Vec<LuaLedLog>>>,
    mode: LuaLedMode,
}

pub struct LuaTick {
    pub message: Option<String>,
    pub leds: Vec<u8>,
    pub slog: Vec<LuaLedLog>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LuaLedLevel {
    Trace,
    Debug,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LuaLedLog {
    pub level: LuaLedLevel,
    pub message: String,
}

pub fn lua_called_message(name: &str) -> String {
    format!("lua called c function {name}")
}

pub fn lua_range_start_message(start: i32) -> String {
    format!("lua range start is {start}")
}

pub fn lua_range_end_message(end: i32) -> String {
    format!("lua range end is {end}")
}

pub fn lua_color_message(color: i32) -> String {
    format!("lua color is {color}")
}

pub fn lua_color_channel_message(index: i32, value: i32) -> String {
    format!("lua color{index} is {value}")
}

pub fn lua_led_message(led: i32) -> String {
    format!("lua led is {led}")
}

pub fn lua_num_leds_message(count: i32) -> String {
    format!("num leds is {count}")
}

pub fn lua_first_byte_message(byte: u8) -> String {
    format!("first byte of buff is x{byte:02x}")
}

impl LuaHost {
    pub fn load(source: &str, mode: LuaLedMode) -> LuaResult<Self> {
        let lua = Lua::new();
        let chunk: Function = lua.load(source).into_function()?;
        Self::install(lua, chunk, mode)
    }

    pub fn load_file(path: &std::path::Path, mode: LuaLedMode) -> Result<Self, String> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => return Err(cannot_open(path, &err)),
        };
        let lua = Lua::new();
        let name = format!("@{}", path.display());
        let chunk = match lua.load(&bytes).set_name(name).into_function() {
            Ok(chunk) => chunk,
            Err(err) => return Err(lua_detail(&err)),
        };
        Self::install(lua, chunk, mode).map_err(|err| lua_detail(&err))
    }

    pub fn call(
        &mut self,
        sim: &mut Telemetry,
        total_leds: i64,
        clock: &impl Clock,
    ) -> LuaResult<LuaTick> {
        let (tick, failure) = self.call_script(sim, total_leds, clock)?;
        match failure {
            Some(err) => Err(err),
            None => Ok(tick),
        }
    }

    pub fn call_script(
        &mut self,
        sim: &mut Telemetry,
        total_leds: i64,
        clock: &impl Clock,
    ) -> LuaResult<(LuaTick, Option<mlua::Error>)> {
        if sim.mtick() == 0 {
            sim.set_mtick(clock.wall_ms());
        }
        let led_count = led_limit(total_leds);
        self.leds.borrow_mut().clear();
        self.leds.borrow_mut().resize(led_count * RGB_CHANNELS, 0);
        self.logs.borrow_mut().clear();
        self.publish_simdata(sim)?;
        self.lua.globals().set("TotalLeds", total_leds)?;
        self.register_leds()?;
        self.set_colors()?;
        let func: Function = self.lua.globals().get("myFunc")?;
        let invoked = func.call::<()>(());
        let message = self.script_message()?;
        let leds = self.leds.borrow().clone();
        let slog = self.logs.borrow().clone();
        Ok((
            LuaTick {
                message,
                leds,
                slog,
            },
            invoked.err(),
        ))
    }

    fn install(lua: Lua, chunk: Function, mode: LuaLedMode) -> LuaResult<Self> {
        lua.globals().set("myFunc", chunk)?;
        Ok(Self {
            lua,
            leds: Rc::new(RefCell::new(Vec::new())),
            logs: Rc::new(RefCell::new(Vec::new())),
            mode,
        })
    }

    fn script_message(&self) -> LuaResult<Option<String>> {
        match self.lua.globals().get(MESSAGE_GLOBAL)? {
            Value::String(text) => Ok(Some(text.to_str()?.to_string())),
            _ => Ok(None),
        }
    }

    fn publish_simdata(&self, sim: &Telemetry) -> LuaResult<()> {
        let table = self.lua.create_table()?;
        table.set("gearc", sim.gearc())?;
        table.set("playerflag", i64::from(sim.player_flag()))?;
        table.set("rpm", i64::from(sim.rpms()))?;
        table.set("gear", i64::from(sim.gear()))?;
        table.set("velocity", i64::from(sim.velocity()))?;
        table.set("mtick", sim.mtick() as i64)?;
        table.set("maxrpm", i64::from(sim.maxrpm()))?;
        table.set("proxcars", proximity_car_count() as i64)?;
        table.set("pd", self.proximity_table(sim)?)?;
        table.set("gas", sim.gas())?;
        table.set("fuel", sim.fuel())?;
        table.set("turboboost", sim.turboboost())?;
        table.set("tyreRPS", self.wheel_table(sim, Telemetry::tyre_rps)?)?;
        table.set(
            "tyrediameter",
            self.wheel_table(sim, Telemetry::tyre_diameter)?,
        )?;
        table.set("tyretemp", self.wheel_table(sim, Telemetry::tyre_temp)?)?;
        self.lua.globals().set("simdata", table)?;
        Ok(())
    }

    fn proximity_table(&self, sim: &Telemetry) -> LuaResult<mlua::Table> {
        let table = self.lua.create_table()?;
        for index in 0..proximity_car_count() {
            let car = self.lua.create_table()?;
            car.set("radius", sim.prox_radius(index) as i64)?;
            car.set("theta", sim.prox_theta(index) as i64)?;
            table.set(index + 1, car)?;
        }
        Ok(table)
    }

    fn wheel_table(
        &self,
        sim: &Telemetry,
        read: fn(&Telemetry, usize) -> f64,
    ) -> LuaResult<mlua::Table> {
        let table = self.lua.create_table()?;
        for index in 0..WHEEL_COUNT {
            table.set(index + 1, read(sim, index))?;
        }
        Ok(table)
    }

    fn register_leds(&self) -> LuaResult<()> {
        let leds = self.leds.clone();
        let logs = self.logs.clone();
        self.lua.globals().set(
            "set_led_to_color",
            self.lua
                .create_function(move |lua, (led, color): (f64, f64)| {
                    set_led_named(&leds, &logs, lua, led, color);
                    Ok(())
                })?,
        )?;
        let leds = self.leds.clone();
        let logs = self.logs.clone();
        self.lua.globals().set(
            "set_led_range_to_color",
            self.lua
                .create_function(move |lua, (start, end, color): (f64, f64, f64)| {
                    set_range_named(&leds, &logs, lua, start, end, color);
                    Ok(())
                })?,
        )?;
        let leds = self.leds.clone();
        let logs = self.logs.clone();
        self.lua.globals().set(
            "set_led_to_rgb_color",
            self.lua
                .create_function(move |lua, (led, color): (f64, f64)| {
                    set_led_packed(&leds, &logs, lua, led, color);
                    Ok(())
                })?,
        )?;
        let leds = self.leds.clone();
        let logs = self.logs.clone();
        self.lua.globals().set(
            "set_led_range_to_rgb_color",
            self.lua
                .create_function(move |lua, (start, end, color): (f64, f64, f64)| {
                    set_range_packed(&leds, &logs, lua, start, end, color);
                    Ok(())
                })?,
        )?;
        let leds = self.leds.clone();
        let logs = self.logs.clone();
        let clear_all = self.lua.create_function(move |lua, (): ()| {
            clear_all_leds(&leds, &logs, lua);
            Ok(())
        })?;
        self.lua.globals().set("led_clear_all", clear_all.clone())?;
        if self.mode == LuaLedMode::Serial {
            self.lua.globals().set("led_clear_range", clear_all)?;
        }
        Ok(())
    }

    fn set_colors(&self) -> LuaResult<()> {
        let globals = self.lua.globals();
        globals.set("RED", COLOR_RED)?;
        globals.set("GREEN", COLOR_GREEN)?;
        globals.set("BLUE", COLOR_BLUE)?;
        globals.set("YELLOW", COLOR_YELLOW)?;
        globals.set("ORANGE", COLOR_ORANGE)?;
        Ok(())
    }
}

fn led_limit(total: i64) -> usize {
    if total <= 0 || total > i64::from(i32::MAX) {
        return 0;
    }
    total as usize
}

fn led_count(lua: &Lua) -> usize {
    let total = lua.globals().get::<i64>("TotalLeds").unwrap_or(0);
    led_limit(total)
}

fn named_rgb(color: i32) -> [u8; RGB_CHANNELS] {
    [
        channel(color, RGB_RED),
        channel(color, RGB_GREEN),
        channel(color, RGB_BLUE),
    ]
}

fn channel(color: i32, rgb: usize) -> u8 {
    match color {
        value if value == COLOR_RED as i32 => full_if(rgb == RGB_RED),
        value if value == COLOR_GREEN as i32 => full_if(rgb == RGB_GREEN),
        value if value == COLOR_BLUE as i32 => full_if(rgb == RGB_BLUE),
        value if value == COLOR_YELLOW as i32 => full_if(rgb == RGB_RED || rgb == RGB_GREEN),
        value if value == COLOR_ORANGE as i32 => orange(rgb),
        _ => 0,
    }
}

fn full_if(on: bool) -> u8 {
    if on {
        LED_FULL
    } else {
        0
    }
}

fn orange(rgb: usize) -> u8 {
    if rgb == RGB_RED {
        return LED_FULL;
    }
    if rgb == RGB_GREEN {
        return LED_ORANGE_GREEN;
    }
    0
}

fn packed_rgb(color: i32) -> [u8; RGB_CHANNELS] {
    [
        ((color >> 16) & 0xff) as u8,
        ((color >> 8) & 0xff) as u8,
        (color & 0xff) as u8,
    ]
}

fn lua_i32(value: f64) -> i32 {
    value as i32
}

fn num_leds(lua: &Lua) -> i32 {
    i32::try_from(led_count(lua)).unwrap_or(i32::MAX)
}

fn push_trace(logs: &Rc<RefCell<Vec<LuaLedLog>>>, message: String) {
    logs.borrow_mut().push(LuaLedLog {
        level: LuaLedLevel::Trace,
        message,
    });
}

fn push_debug(logs: &Rc<RefCell<Vec<LuaLedLog>>>, message: String) {
    logs.borrow_mut().push(LuaLedLog {
        level: LuaLedLevel::Debug,
        message,
    });
}

fn slog_called(logs: &Rc<RefCell<Vec<LuaLedLog>>>, name: &str) {
    push_trace(logs, lua_called_message(name));
}

fn slog_first_byte(logs: &Rc<RefCell<Vec<LuaLedLog>>>, leds: &Rc<RefCell<Vec<u8>>>) {
    let byte = leds.borrow().first().copied().unwrap_or(EMPTY_LED_BYTE);
    push_trace(logs, lua_first_byte_message(byte));
}

fn set_led_named(
    leds: &Rc<RefCell<Vec<u8>>>,
    logs: &Rc<RefCell<Vec<LuaLedLog>>>,
    lua: &Lua,
    led: f64,
    color: f64,
) {
    slog_called(logs, FN_SET_LED_TO_RGB);
    let led = lua_i32(led);
    let color = lua_i32(color);
    push_debug(logs, lua_led_message(led));
    push_debug(logs, lua_color_message(color));
    let index = led - 1;
    let count = num_leds(lua);
    push_debug(logs, lua_num_leds_message(count));
    if index < 0 || index >= count {
        return;
    }
    slog_first_byte(logs, leds);
    write_led(&mut leds.borrow_mut(), index as usize, named_rgb(color));
}

fn set_led_packed(
    leds: &Rc<RefCell<Vec<u8>>>,
    logs: &Rc<RefCell<Vec<LuaLedLog>>>,
    lua: &Lua,
    led: f64,
    color: f64,
) {
    slog_called(logs, FN_SET_LED_TO_RGB);
    let led = lua_i32(led);
    let rgb = packed_rgb(lua_i32(color));
    push_debug(logs, lua_led_message(led));
    slog_rgb_channels(logs, rgb);
    let index = led - 1;
    let count = num_leds(lua);
    push_debug(logs, lua_num_leds_message(count));
    if index < 0 || index >= count {
        return;
    }
    slog_first_byte(logs, leds);
    write_led(&mut leds.borrow_mut(), index as usize, rgb);
}

const COLOR_CHANNEL_0: i32 = 0;
const COLOR_CHANNEL_1: i32 = 1;
const COLOR_CHANNEL_2: i32 = 2;

fn slog_rgb_channels(logs: &Rc<RefCell<Vec<LuaLedLog>>>, rgb: [u8; RGB_CHANNELS]) {
    push_debug(
        logs,
        lua_color_channel_message(COLOR_CHANNEL_0, i32::from(rgb[RGB_RED])),
    );
    push_debug(
        logs,
        lua_color_channel_message(COLOR_CHANNEL_1, i32::from(rgb[RGB_GREEN])),
    );
    push_debug(
        logs,
        lua_color_channel_message(COLOR_CHANNEL_2, i32::from(rgb[RGB_BLUE])),
    );
}

fn set_range_named(
    leds: &Rc<RefCell<Vec<u8>>>,
    logs: &Rc<RefCell<Vec<LuaLedLog>>>,
    lua: &Lua,
    start: f64,
    end: f64,
    color: f64,
) {
    slog_called(logs, FN_SET_LED_RANGE_TO_COLOR);
    let start = lua_i32(start);
    let end = lua_i32(end);
    let color = lua_i32(color);
    push_debug(logs, lua_range_start_message(start));
    push_debug(logs, lua_range_end_message(end));
    push_debug(logs, lua_color_message(color));
    let range_start = start - 1;
    let count = num_leds(lua);
    push_debug(logs, lua_num_leds_message(count));
    let Some((from, to)) = clamp_led_range(range_start, end, count) else {
        push_trace(logs, MSG_INVALID_RANGE.to_string());
        return;
    };
    slog_first_byte(logs, leds);
    paint_span(leds, from, to, named_rgb(color));
}

fn set_range_packed(
    leds: &Rc<RefCell<Vec<u8>>>,
    logs: &Rc<RefCell<Vec<LuaLedLog>>>,
    lua: &Lua,
    start: f64,
    end: f64,
    color: f64,
) {
    slog_called(logs, FN_SET_LED_RANGE_TO_RGB);
    let start = lua_i32(start);
    let end = lua_i32(end);
    let rgb = packed_rgb(lua_i32(color));
    push_debug(logs, lua_range_start_message(start));
    push_debug(logs, lua_range_end_message(end));
    slog_rgb_channels(logs, rgb);
    let range_start = start - 1;
    let count = num_leds(lua);
    push_debug(logs, lua_num_leds_message(count));
    let Some((from, to)) = clamp_led_range(range_start, end, count) else {
        return;
    };
    slog_first_byte(logs, leds);
    paint_span(leds, from, to, rgb);
}

fn clear_all_leds(leds: &Rc<RefCell<Vec<u8>>>, logs: &Rc<RefCell<Vec<LuaLedLog>>>, lua: &Lua) {
    slog_called(logs, FN_LED_CLEAR_ALL);
    let count = num_leds(lua);
    push_debug(logs, lua_num_leds_message(count));
    slog_first_byte(logs, leds);
    paint_span(leds, 0, count, [0, 0, 0]);
}

fn clamp_led_range(range_start: i32, mut range_end: i32, count: i32) -> Option<(i32, i32)> {
    if range_start < 0 || range_end <= range_start || range_end > count {
        if range_end > count {
            range_end = count;
        }
        if range_start < 0 || range_end <= range_start {
            return None;
        }
    }
    Some((range_start, range_end))
}

fn paint_span(leds: &Rc<RefCell<Vec<u8>>>, from: i32, to: i32, rgb: [u8; RGB_CHANNELS]) {
    let mut buf = leds.borrow_mut();
    let mut index = from;
    while index < to {
        write_led(&mut buf, index as usize, rgb);
        index += 1;
    }
}

const LUA_SYNTAX_PREFIX: &str = "syntax error: ";
const LUA_RUNTIME_PREFIX: &str = "runtime error: ";
const LUA_CANNOT_OPEN: &str = "cannot open";

fn cannot_open(path: &std::path::Path, err: &std::io::Error) -> String {
    format!("{LUA_CANNOT_OPEN} {}: {}", path.display(), os_reason(err))
}

fn os_reason(err: &std::io::Error) -> String {
    let Some(code) = err.raw_os_error() else {
        return err.to_string();
    };
    let reason = unsafe { libc::strerror(code) };
    if reason.is_null() {
        return err.to_string();
    }
    unsafe { std::ffi::CStr::from_ptr(reason) }
        .to_string_lossy()
        .into_owned()
}

pub fn lua_detail(err: &mlua::Error) -> String {
    let text = err.to_string();
    let text = text.strip_prefix(LUA_SYNTAX_PREFIX).unwrap_or(&text);
    text.strip_prefix(LUA_RUNTIME_PREFIX)
        .unwrap_or(text)
        .to_string()
}

fn write_led(buf: &mut [u8], index: usize, rgb: [u8; RGB_CHANNELS]) {
    let base = index * RGB_CHANNELS;
    if base + RGB_BLUE >= buf.len() {
        return;
    }
    buf[base + RGB_RED] = rgb[RGB_RED];
    buf[base + RGB_GREEN] = rgb[RGB_GREEN];
    buf[base + RGB_BLUE] = rgb[RGB_BLUE];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::VirtualClock;

    fn sample() -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/parity/fixtures/leds.lua"
        );
        std::fs::read_to_string(path).expect("leds.lua")
    }

    #[test]
    fn serial_sample_sets_green_and_message() {
        let mut host = LuaHost::load(&sample(), LuaLedMode::Serial).expect("load");
        let mut sim = Telemetry::new();
        sim.set_rpms(4500);
        sim.set_gas(0.25);
        let mut clock = VirtualClock::new();
        clock.advance_us(16_000);
        let first = host.call(&mut sim, 8, &clock).expect("define");
        assert!(first.message.is_none());
        assert_eq!(sim.mtick(), 16);
        let second = host.call(&mut sim, 8, &clock).expect("run");
        assert_eq!(second.message.as_deref(), Some("parity"));
        assert_eq!(&second.leds[..RGB_CHANNELS], &[0, LED_FULL, 0]);
        let table: mlua::Table = host.lua.globals().get("simdata").expect("simdata");
        assert!(matches!(
            table.get::<Value>("rpm").unwrap(),
            Value::Integer(4500)
        ));
        assert!(matches!(
            table.get::<Value>("gas").unwrap(),
            Value::Number(_)
        ));
    }

    #[test]
    fn serial_led_clear_range_clears_every_led() {
        let source = "function myFunc() set_led_to_color(1, RED) led_clear_range() end";
        let mut host = LuaHost::load(source, LuaLedMode::Serial).expect("load");
        let mut sim = Telemetry::new();
        sim.set_mtick(5);
        let clock = VirtualClock::new();
        host.call(&mut sim, 4, &clock).expect("define");
        let tick = host.call(&mut sim, 4, &clock).expect("run");
        assert!(tick.leds.iter().all(|byte| *byte == 0));
        assert!(tick
            .slog
            .iter()
            .any(|log| log.message == lua_called_message(FN_LED_CLEAR_ALL)));
    }

    #[test]
    fn lua_led_slog_matches_the_c_host() {
        const LED_COUNT: i64 = 4;
        const NAMED_LED: i32 = 1;
        const RANGE_END: i32 = 3;
        const PACKED_RED: i32 = 0x00ff0000;
        let named = run_lua("set_led_to_color(1, RED)", LuaLedMode::Usb, LED_COUNT);
        assert_eq!(
            named.slog,
            vec![
                log_trace(lua_called_message(FN_SET_LED_TO_RGB)),
                log_debug(lua_led_message(NAMED_LED)),
                log_debug(lua_color_message(COLOR_RED as i32)),
                log_debug(lua_num_leds_message(LED_COUNT as i32)),
                log_trace(lua_first_byte_message(EMPTY_LED_BYTE)),
            ]
        );
        let range = run_lua(
            "set_led_range_to_color(1, 3, GREEN)",
            LuaLedMode::Usb,
            LED_COUNT,
        );
        assert!(range
            .slog
            .iter()
            .any(|log| log.message == lua_called_message(FN_SET_LED_RANGE_TO_COLOR)));
        assert!(range
            .slog
            .iter()
            .any(|log| log.message == lua_range_start_message(NAMED_LED)));
        assert!(range
            .slog
            .iter()
            .any(|log| log.message == lua_range_end_message(RANGE_END)));
        let invalid = run_lua(
            "set_led_range_to_color(3, 1, RED)",
            LuaLedMode::Usb,
            LED_COUNT,
        );
        assert!(invalid
            .slog
            .iter()
            .any(|log| log.message == MSG_INVALID_RANGE));
        let packed = run_lua(
            "set_led_to_rgb_color(1, 0x00ff0000)",
            LuaLedMode::Usb,
            LED_COUNT,
        );
        assert!(packed
            .slog
            .iter()
            .any(|log| log.message == lua_called_message(FN_SET_LED_TO_RGB)));
        assert!(packed.slog.iter().any(|log| {
            log.message == lua_color_channel_message(COLOR_CHANNEL_0, PACKED_RED >> 16)
        }));
        let clear = run_lua("led_clear_range()", LuaLedMode::Serial, LED_COUNT);
        assert!(clear
            .slog
            .iter()
            .any(|log| log.message == lua_called_message(FN_LED_CLEAR_ALL)));
    }

    fn run_lua(source: &str, mode: LuaLedMode, leds: i64) -> LuaTick {
        let mut host = LuaHost::load(source, mode).expect("load");
        let mut sim = Telemetry::new();
        sim.set_mtick(1);
        host.call(&mut sim, leds, &VirtualClock::new())
            .expect("run")
    }

    fn log_trace(message: String) -> LuaLedLog {
        LuaLedLog {
            level: LuaLedLevel::Trace,
            message,
        }
    }

    fn log_debug(message: String) -> LuaLedLog {
        LuaLedLog {
            level: LuaLedLevel::Debug,
            message,
        }
    }

    #[test]
    fn load_file_matches_luas_missing_path_message() {
        let path = std::path::Path::new("/tmp/cargopit-c12-missing.lua");
        let Err(err) = LuaHost::load_file(path, LuaLedMode::Usb) else {
            panic!("missing file loaded");
        };
        assert_eq!(
            err,
            format!("cannot open {}: No such file or directory", path.display())
        );
    }
}
