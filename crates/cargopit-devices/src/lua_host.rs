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
    mode: LuaLedMode,
}

pub struct LuaTick {
    pub message: Option<String>,
    pub leds: Vec<u8>,
}

impl LuaHost {
    pub fn load(source: &str, mode: LuaLedMode) -> LuaResult<Self> {
        let lua = Lua::new();
        let chunk: Function = lua.load(source).into_function()?;
        lua.globals().set("myFunc", chunk)?;
        Ok(Self {
            lua,
            leds: Rc::new(RefCell::new(Vec::new())),
            mode,
        })
    }

    pub fn call(
        &mut self,
        sim: &mut Telemetry,
        total_leds: i64,
        clock: &impl Clock,
    ) -> LuaResult<LuaTick> {
        if sim.mtick() == 0 {
            sim.set_mtick(clock.wall_ms());
        }
        let led_count = led_limit(total_leds);
        self.leds.borrow_mut().clear();
        self.leds.borrow_mut().resize(led_count * RGB_CHANNELS, 0);
        self.publish_simdata(sim)?;
        self.lua.globals().set("TotalLeds", total_leds)?;
        self.register_leds()?;
        self.set_colors()?;
        let func: Function = self.lua.globals().get("myFunc")?;
        func.call::<()>(())?;
        let message = match self.lua.globals().get(MESSAGE_GLOBAL)? {
            Value::String(text) => Some(text.to_str()?.to_string()),
            _ => None,
        };
        Ok(LuaTick {
            message,
            leds: self.leds.borrow().clone(),
        })
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
        self.lua.globals().set(
            "set_led_to_color",
            self.lua
                .create_function(move |lua, (led, color): (f64, f64)| {
                    paint_one(&leds, lua, led, named_rgb(color as i32));
                    Ok(())
                })?,
        )?;
        let leds = self.leds.clone();
        self.lua.globals().set(
            "set_led_range_to_color",
            self.lua
                .create_function(move |lua, (start, end, color): (f64, f64, f64)| {
                    paint_range(&leds, lua, start, end, named_rgb(color as i32));
                    Ok(())
                })?,
        )?;
        let leds = self.leds.clone();
        self.lua.globals().set(
            "set_led_to_rgb_color",
            self.lua
                .create_function(move |lua, (led, color): (f64, f64)| {
                    paint_one(&leds, lua, led, packed_rgb(color as i32));
                    Ok(())
                })?,
        )?;
        let leds = self.leds.clone();
        self.lua.globals().set(
            "set_led_range_to_rgb_color",
            self.lua
                .create_function(move |lua, (start, end, color): (f64, f64, f64)| {
                    paint_range(&leds, lua, start, end, packed_rgb(color as i32));
                    Ok(())
                })?,
        )?;
        let leds = self.leds.clone();
        let clear_all = self.lua.create_function(move |lua, (): ()| {
            clear_count(&leds, lua, 0, led_count(lua));
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

fn paint_one(leds: &Rc<RefCell<Vec<u8>>>, lua: &Lua, led: f64, rgb: [u8; RGB_CHANNELS]) {
    let index = (led as i32) - 1;
    let count = led_count(lua);
    if index < 0 || index as usize >= count {
        return;
    }
    write_led(&mut leds.borrow_mut(), index as usize, rgb);
}

fn paint_range(
    leds: &Rc<RefCell<Vec<u8>>>,
    lua: &Lua,
    start: f64,
    end: f64,
    rgb: [u8; RGB_CHANNELS],
) {
    let mut range_start = (start as i32) - 1;
    let mut range_end = end as i32;
    let count = led_count(lua) as i32;
    if range_end > count {
        range_end = count;
    }
    if range_start < 0 || range_end <= range_start {
        return;
    }
    let mut buf = leds.borrow_mut();
    while range_start < range_end {
        write_led(&mut buf, range_start as usize, rgb);
        range_start += 1;
    }
}

fn clear_count(leds: &Rc<RefCell<Vec<u8>>>, lua: &Lua, _ignored_start: i32, count: usize) {
    let _ = lua;
    let mut buf = leds.borrow_mut();
    let leds_to_clear = count.min(buf.len() / RGB_CHANNELS);
    for index in 0..leds_to_clear {
        write_led(&mut buf, index, [0, 0, 0]);
    }
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
    }
}
