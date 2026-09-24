//! Single source of telemetry streams for C capture and the later Rust runner.
//! Numeric values match the C tester fixtures in `src/cargopit/gameloop/tester.c`.

use simapi_sys::{
    SimDataBuf, BOOL_SIZE, CAR_NAME_BYTES, F64_SIZE, GEAR_CHAR_BYTES, OFF_ABS, OFF_BRAKE,
    OFF_BRAKE_TEMP, OFF_CAR, OFF_COURSE_FLAG, OFF_GAS, OFF_GEAR, OFF_GEARC, OFF_IDLERPM,
    OFF_MAXRPM, OFF_MTICK, OFF_PLAYER_FLAG, OFF_PROXIMITY, OFF_PROX_RADIUS, OFF_PROX_THETA,
    OFF_PULSES, OFF_RPMS, OFF_SIMON, OFF_SIMSTATUS, OFF_SUSP_VELOCITY, OFF_TYRE_DIAMETER,
    OFF_TYRE_RPS, OFF_TYRE_SLIP_RATIO, OFF_VELOCITY, OFF_XVELOCITY, OFF_YVELOCITY, OFF_ZVELOCITY,
    PROXIMITY_STRIDE, SIMDATA_SIZE, WHEEL_COUNT,
};

pub const STREAM_MAGIC: &[u8; 8] = b"CPITSCN1";

const TEST_RPM_IDLE: u32 = 1000;
const TEST_RPM_MID_LOW: u32 = 2000;
const TEST_RPM_MID: u32 = 4000;
const TEST_RPM_HIGH: u32 = 7000;
const TEST_RPM_MAX: u32 = 8000;
const TEST_VELOCITY_CRUISE: u32 = 160;
const TEST_VELOCITY_SLOW: u32 = 100;
const TEST_VELOCITY_FAST: u32 = 200;
const TEST_VELOCITY_TOP: u32 = 300;
const TEST_VELOCITY_SPIN: u32 = 15;
const TEST_VELOCITY_LOCK: u32 = 150;
const TEST_YVELOCITY: f64 = 100.0;
const TEST_PEDAL_APPLIED: f64 = 0.85;
const TEST_SLIP_SPIN: f64 = -0.45;
const TEST_SLIP_LOCK: f64 = 0.85;
const TEST_SLIP_ABS_A: f64 = 0.40;
const TEST_SLIP_ABS_B: f64 = 0.72;
const TEST_ABS_OFF: f64 = 0.0;
const TEST_ABS_ACTIVE: f64 = 1.0;
const TEST_BRAKE_TEMP_HOT: f64 = 0.90;
const TEST_SUSP_VEL_A: f64 = 2.0;
const TEST_SUSP_VEL_B: f64 = 18.0;
const TEST_TYRE_DIAMETER_UNSET: f64 = -1.0;
const TEST_TYRE_DIAMETER_FL: f64 = 0.638636385206394;
const TEST_TYRE_DIAMETER_FR: f64 = 0.633384434597093;
const TEST_TYRE_DIAMETER_RL: f64 = 0.710475735564615;
const TEST_TYRE_DIAMETER_RR: f64 = 0.710475735564615;
const TEST_TYRE_RPS_SPIN: f64 = 50.0;
const TEST_TYRE_RPS_LOCK: f64 = 25.0;
const TEST_CAR_NAME: &str = "CAR";
const TEST_GEAR_CHAR_NEUTRAL: u8 = b'N';
const GEAR_NEUTRAL: u32 = 1;
const GEAR_FIRST: u32 = 2;
const GEAR_SECOND: u32 = 3;
const STATUS_OFF: u32 = 0;
const STATUS_ACTIVE: u32 = 2;
const FLAG_GREEN: u8 = 0;
const FLAG_YELLOW: u8 = 1;
const FLAG_BLUE: u8 = 4;
const TICK_STEP: u64 = 16;
const RADAR_RADIUS_M: f64 = 12.5;
const RADAR_THETA_DEG: f64 = 35.0;
const FROZEN_VELOCITY: u32 = 0;

pub struct Scenario {
    pub name: &'static str,
    pub frames: Vec<SimDataBuf>,
}

pub fn all() -> Vec<Scenario> {
    vec![
        scenario("rpm", rpm_frames()),
        scenario("gear", gear_frames()),
        scenario("slip", slip_frames()),
        scenario("lock", lock_frames()),
        scenario("abs", abs_frames()),
        scenario("suspension", suspension_frames()),
        scenario("flags", flag_frames()),
        scenario("brake_temperature", brake_temperature_frames()),
        scenario("velocity", velocity_frames()),
        scenario("blink", blink_frames()),
        scenario("radar", radar_frames()),
        scenario("sim_on", vec![sim_power(true)]),
        scenario("sim_off", vec![sim_power(false)]),
        scenario("frozen_telemetry", frozen_frames()),
        scenario("basic", vec![basic_frame()]),
        scenario("wheel_spin", vec![wheel_spin_frame()]),
        scenario("wheel_lock", vec![wheel_lock_frame()]),
    ]
}

fn scenario(name: &'static str, frames: Vec<SimDataBuf>) -> Scenario {
    Scenario { name, frames }
}

fn blank() -> SimDataBuf {
    let mut frame = SimDataBuf::new();
    frame.set_u32(OFF_MAXRPM, TEST_RPM_MAX);
    frame.set_u32(OFF_IDLERPM, TEST_RPM_IDLE);
    frame.set_u32(OFF_SIMSTATUS, STATUS_ACTIVE);
    frame.set_bool(OFF_SIMON, true);
    set_car(&mut frame, TEST_CAR_NAME);
    set_gear(&mut frame, GEAR_NEUTRAL, TEST_GEAR_CHAR_NEUTRAL);
    fill_wheels(&mut frame, OFF_TYRE_DIAMETER, TEST_TYRE_DIAMETER_UNSET);
    frame
}

fn basic_frame() -> SimDataBuf {
    let mut frame = blank();
    frame.set_u32(OFF_VELOCITY, TEST_VELOCITY_CRUISE);
    frame.set_u32(OFF_RPMS, TEST_RPM_IDLE);
    frame.set_f64(OFF_XVELOCITY, 0.0);
    frame.set_f64(OFF_YVELOCITY, TEST_YVELOCITY);
    frame.set_f64(OFF_ZVELOCITY, 0.0);
    frame
}

fn wheel_spin_frame() -> SimDataBuf {
    let mut frame = basic_frame();
    frame.set_u32(OFF_VELOCITY, TEST_VELOCITY_SPIN);
    frame.set_f64(OFF_GAS, TEST_PEDAL_APPLIED);
    frame.set_f64(OFF_BRAKE, 0.0);
    frame.set_f64(OFF_ABS, TEST_ABS_OFF);
    fill_wheels(&mut frame, OFF_TYRE_SLIP_RATIO, TEST_SLIP_SPIN);
    fill_wheels(&mut frame, OFF_BRAKE_TEMP, 0.0);
    set_tyre_fallback(&mut frame, TEST_TYRE_RPS_SPIN);
    frame
}

fn wheel_lock_frame() -> SimDataBuf {
    let mut frame = basic_frame();
    frame.set_u32(OFF_VELOCITY, TEST_VELOCITY_LOCK);
    frame.set_f64(OFF_GAS, 0.0);
    frame.set_f64(OFF_BRAKE, TEST_PEDAL_APPLIED);
    frame.set_f64(OFF_ABS, TEST_ABS_OFF);
    fill_wheels(&mut frame, OFF_TYRE_SLIP_RATIO, TEST_SLIP_LOCK);
    fill_wheels(&mut frame, OFF_BRAKE_TEMP, TEST_BRAKE_TEMP_HOT);
    set_tyre_fallback(&mut frame, TEST_TYRE_RPS_LOCK);
    frame
}

fn rpm_frames() -> Vec<SimDataBuf> {
    [TEST_RPM_IDLE, TEST_RPM_MID, TEST_RPM_HIGH, TEST_RPM_MAX]
        .into_iter()
        .enumerate()
        .map(|(index, rpm)| {
            let mut frame = basic_frame();
            frame.set_u64(OFF_MTICK, (index as u64 + 1) * TICK_STEP);
            frame.set_u32(OFF_RPMS, rpm);
            frame.set_u32(OFF_PULSES, rpm / 10);
            frame
        })
        .collect()
}

fn gear_frames() -> Vec<SimDataBuf> {
    [(GEAR_NEUTRAL, TEST_GEAR_CHAR_NEUTRAL), (GEAR_FIRST, b'1'), (GEAR_SECOND, b'2')]
        .into_iter()
        .enumerate()
        .map(|(index, (gear, gear_char))| {
            let mut frame = basic_frame();
            frame.set_u64(OFF_MTICK, (index as u64 + 1) * TICK_STEP);
            frame.set_u32(OFF_RPMS, TEST_RPM_MID_LOW);
            set_gear(&mut frame, gear, gear_char);
            frame
        })
        .collect()
}

fn slip_frames() -> Vec<SimDataBuf> {
    vec![wheel_spin_frame()]
}

fn lock_frames() -> Vec<SimDataBuf> {
    vec![wheel_lock_frame()]
}

fn abs_frames() -> Vec<SimDataBuf> {
    [TEST_SLIP_ABS_A, TEST_SLIP_ABS_B]
        .into_iter()
        .enumerate()
        .map(|(index, slip)| {
            let mut frame = wheel_lock_frame();
            frame.set_u64(OFF_MTICK, (index as u64 + 1) * TICK_STEP);
            frame.set_f64(OFF_ABS, TEST_ABS_ACTIVE);
            fill_wheels(&mut frame, OFF_TYRE_SLIP_RATIO, slip);
            frame
        })
        .collect()
}

fn suspension_frames() -> Vec<SimDataBuf> {
    [TEST_SUSP_VEL_A, TEST_SUSP_VEL_B]
        .into_iter()
        .enumerate()
        .map(|(index, velocity)| {
            let mut frame = basic_frame();
            frame.set_u64(OFF_MTICK, (index as u64 + 1) * TICK_STEP);
            frame.set_u32(OFF_VELOCITY, TEST_VELOCITY_FAST);
            fill_wheels(&mut frame, OFF_SUSP_VELOCITY, velocity);
            frame
        })
        .collect()
}

fn flag_frames() -> Vec<SimDataBuf> {
    [FLAG_GREEN, FLAG_YELLOW, FLAG_BLUE]
        .into_iter()
        .enumerate()
        .map(|(index, flag)| {
            let mut frame = basic_frame();
            frame.set_u64(OFF_MTICK, (index as u64 + 1) * TICK_STEP);
            frame.set_u8(OFF_COURSE_FLAG, flag);
            frame.set_u8(OFF_PLAYER_FLAG, flag);
            frame
        })
        .collect()
}

fn brake_temperature_frames() -> Vec<SimDataBuf> {
    let mut frame = basic_frame();
    frame.set_u32(OFF_VELOCITY, TEST_VELOCITY_LOCK);
    frame.set_f64(OFF_BRAKE, TEST_PEDAL_APPLIED);
    fill_wheels(&mut frame, OFF_BRAKE_TEMP, TEST_BRAKE_TEMP_HOT);
    vec![frame]
}

fn velocity_frames() -> Vec<SimDataBuf> {
    [TEST_VELOCITY_SLOW, TEST_VELOCITY_CRUISE, TEST_VELOCITY_FAST, TEST_VELOCITY_TOP]
        .into_iter()
        .enumerate()
        .map(|(index, velocity)| {
            let mut frame = basic_frame();
            frame.set_u64(OFF_MTICK, (index as u64 + 1) * TICK_STEP);
            frame.set_u32(OFF_VELOCITY, velocity);
            frame
        })
        .collect()
}

fn blink_frames() -> Vec<SimDataBuf> {
    [0_u64, 80, 160]
        .into_iter()
        .map(|tick| {
            let mut frame = basic_frame();
            frame.set_u64(OFF_MTICK, tick);
            frame.set_u8(OFF_COURSE_FLAG, FLAG_YELLOW);
            frame.set_u32(OFF_RPMS, TEST_RPM_HIGH);
            frame
        })
        .collect()
}

fn radar_frames() -> Vec<SimDataBuf> {
    let mut frame = basic_frame();
    let prox = OFF_PROXIMITY;
    frame.set_f64(prox + OFF_PROX_RADIUS, RADAR_RADIUS_M);
    frame.set_f64(prox + OFF_PROX_THETA, RADAR_THETA_DEG);
    let _ = PROXIMITY_STRIDE;
    vec![frame]
}

fn sim_power(on: bool) -> SimDataBuf {
    let mut frame = basic_frame();
    frame.set_bool(OFF_SIMON, on);
    frame.set_u32(OFF_SIMSTATUS, if on { STATUS_ACTIVE } else { STATUS_OFF });
    frame
}

fn frozen_frames() -> Vec<SimDataBuf> {
    let mut frame = basic_frame();
    frame.set_u32(OFF_VELOCITY, FROZEN_VELOCITY);
    frame.set_f64(OFF_YVELOCITY, 0.0);
    fill_wheels(&mut frame, OFF_SUSP_VELOCITY, TEST_SUSP_VEL_B);
    set_tyre_fallback(&mut frame, 0.0);
    vec![frame]
}

fn set_gear(frame: &mut SimDataBuf, gear: u32, gear_char: u8) {
    frame.set_u32(OFF_GEAR, gear);
    let mut chars = [0_u8; GEAR_CHAR_BYTES];
    chars[0] = gear_char;
    frame.set_bytes(OFF_GEARC, &chars);
}

fn set_car(frame: &mut SimDataBuf, name: &str) {
    let mut bytes = [0_u8; CAR_NAME_BYTES];
    let raw = name.as_bytes();
    bytes[..raw.len()].copy_from_slice(raw);
    frame.set_bytes(OFF_CAR, &bytes);
}

fn fill_wheels(frame: &mut SimDataBuf, base: usize, value: f64) {
    for index in 0..WHEEL_COUNT {
        frame.set_f64_wheel(base, index, value);
    }
}

fn set_tyre_fallback(frame: &mut SimDataBuf, rps: f64) {
    fill_wheels(frame, OFF_TYRE_RPS, rps);
    frame.set_f64_wheel(OFF_TYRE_DIAMETER, 0, TEST_TYRE_DIAMETER_FL);
    frame.set_f64_wheel(OFF_TYRE_DIAMETER, 1, TEST_TYRE_DIAMETER_FR);
    frame.set_f64_wheel(OFF_TYRE_DIAMETER, 2, TEST_TYRE_DIAMETER_RL);
    frame.set_f64_wheel(OFF_TYRE_DIAMETER, 3, TEST_TYRE_DIAMETER_RR);
}

pub fn frame_size() -> usize {
    SIMDATA_SIZE
}

pub fn bool_size() -> usize {
    BOOL_SIZE
}

pub fn f64_size() -> usize {
    F64_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scenario_frame_matches_simdata_size() {
        for scenario in all() {
            assert!(!scenario.frames.is_empty(), "{}", scenario.name);
            for frame in &scenario.frames {
                assert_eq!(frame.as_bytes().len(), SIMDATA_SIZE, "{}", scenario.name);
            }
        }
    }

    #[test]
    fn scenario_list_is_stable() {
        let names: Vec<_> = all().into_iter().map(|scenario| scenario.name).collect();
        assert_eq!(
            names,
            [
                "rpm",
                "gear",
                "slip",
                "lock",
                "abs",
                "suspension",
                "flags",
                "brake_temperature",
                "velocity",
                "blink",
                "radar",
                "sim_on",
                "sim_off",
                "frozen_telemetry",
                "basic",
                "wheel_spin",
                "wheel_lock",
            ]
        );
    }
}
