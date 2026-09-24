//! Field access for the submodule `SimData` layout. Devices borrow one buffer per tick.

use simapi_sys::{
    SimDataBuf, F64_SIZE, GEARC_BYTES, OFF_BRAKE, OFF_FUEL, OFF_GAS, OFF_GEAR, OFF_GEARC,
    OFF_MAXRPM, OFF_MTICK, OFF_PLAYER_FLAG, OFF_PROXIMITY, OFF_PROX_RADIUS, OFF_PROX_THETA,
    OFF_PULSES, OFF_RPMS, OFF_SIMAPI, OFF_SUSP_VELOCITY, OFF_TURBOBOOST, OFF_TYRE_DIAMETER,
    OFF_TYRE_RPS, OFF_TYRE_SLIP_RATIO, OFF_TYRE_TEMP, OFF_VELOCITY, OFF_XVELOCITY, OFF_YVELOCITY,
    OFF_ZVELOCITY, PROXIMITY_STRIDE, WHEEL_COUNT,
};

pub const PROXIMITY_CARS: usize = 6;

pub struct Telemetry {
    buf: SimDataBuf,
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            buf: SimDataBuf::new(),
        }
    }

    pub fn from_buf(buf: SimDataBuf) -> Self {
        Self { buf }
    }

    pub fn clone_buf(&self) -> Self {
        Self {
            buf: self.buf.clone(),
        }
    }

    pub fn pulses(&self) -> u32 {
        self.buf.get_u32(OFF_PULSES)
    }

    pub fn set_pulses(&mut self, value: u32) {
        self.buf.set_u32(OFF_PULSES, value);
    }

    pub fn mtick(&self) -> u64 {
        self.buf.get_u64(OFF_MTICK)
    }

    pub fn set_mtick(&mut self, value: u64) {
        self.buf.set_u64(OFF_MTICK, value);
    }

    pub fn velocity(&self) -> u32 {
        self.buf.get_u32(OFF_VELOCITY)
    }

    pub fn set_velocity(&mut self, value: u32) {
        self.buf.set_u32(OFF_VELOCITY, value);
    }

    pub fn rpms(&self) -> u32 {
        self.buf.get_u32(OFF_RPMS)
    }

    pub fn set_rpms(&mut self, value: u32) {
        self.buf.set_u32(OFF_RPMS, value);
    }

    pub fn gear(&self) -> u32 {
        self.buf.get_u32(OFF_GEAR)
    }

    pub fn set_gear(&mut self, value: u32) {
        self.buf.set_u32(OFF_GEAR, value);
    }

    pub fn maxrpm(&self) -> u32 {
        self.buf.get_u32(OFF_MAXRPM)
    }

    pub fn set_maxrpm(&mut self, value: u32) {
        self.buf.set_u32(OFF_MAXRPM, value);
    }

    pub fn player_flag(&self) -> u8 {
        self.buf.get_u8(OFF_PLAYER_FLAG)
    }

    pub fn set_player_flag(&mut self, value: u8) {
        self.buf.set_u8(OFF_PLAYER_FLAG, value);
    }

    pub fn simapi(&self) -> u8 {
        self.buf.get_u8(OFF_SIMAPI)
    }

    pub fn set_simapi(&mut self, value: u8) {
        self.buf.set_u8(OFF_SIMAPI, value);
    }

    pub fn gearc(&self) -> String {
        let bytes = self.buf.get_bytes(OFF_GEARC, GEARC_BYTES);
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }

    pub fn set_gearc(&mut self, text: &str) {
        let mut bytes = [0u8; GEARC_BYTES];
        let copy = text.len().min(GEARC_BYTES);
        bytes[..copy].copy_from_slice(&text.as_bytes()[..copy]);
        self.buf.set_bytes(OFF_GEARC, &bytes);
    }

    pub fn gas(&self) -> f64 {
        self.buf.get_f64(OFF_GAS)
    }

    pub fn set_gas(&mut self, value: f64) {
        self.buf.set_f64(OFF_GAS, value);
    }

    pub fn brake(&self) -> f64 {
        self.buf.get_f64(OFF_BRAKE)
    }

    pub fn set_brake(&mut self, value: f64) {
        self.buf.set_f64(OFF_BRAKE, value);
    }

    pub fn fuel(&self) -> f64 {
        self.buf.get_f64(OFF_FUEL)
    }

    pub fn set_fuel(&mut self, value: f64) {
        self.buf.set_f64(OFF_FUEL, value);
    }

    pub fn turboboost(&self) -> f64 {
        self.buf.get_f64(OFF_TURBOBOOST)
    }

    pub fn set_turboboost(&mut self, value: f64) {
        self.buf.set_f64(OFF_TURBOBOOST, value);
    }

    pub fn x_velocity(&self) -> f64 {
        self.buf.get_f64(OFF_XVELOCITY)
    }

    pub fn set_x_velocity(&mut self, value: f64) {
        self.buf.set_f64(OFF_XVELOCITY, value);
    }

    pub fn y_velocity(&self) -> f64 {
        self.buf.get_f64(OFF_YVELOCITY)
    }

    pub fn set_y_velocity(&mut self, value: f64) {
        self.buf.set_f64(OFF_YVELOCITY, value);
    }

    pub fn z_velocity(&self) -> f64 {
        self.buf.get_f64(OFF_ZVELOCITY)
    }

    pub fn set_z_velocity(&mut self, value: f64) {
        self.buf.set_f64(OFF_ZVELOCITY, value);
    }

    pub fn tyre_rps(&self, index: usize) -> f64 {
        self.wheel(OFF_TYRE_RPS, index)
    }

    pub fn set_tyre_rps(&mut self, index: usize, value: f64) {
        self.set_wheel(OFF_TYRE_RPS, index, value);
    }

    pub fn tyre_diameter(&self, index: usize) -> f64 {
        self.wheel(OFF_TYRE_DIAMETER, index)
    }

    pub fn set_tyre_diameter(&mut self, index: usize, value: f64) {
        self.set_wheel(OFF_TYRE_DIAMETER, index, value);
    }

    pub fn tyre_slip(&self, index: usize) -> f64 {
        self.wheel(OFF_TYRE_SLIP_RATIO, index)
    }

    pub fn set_tyre_slip(&mut self, index: usize, value: f64) {
        self.set_wheel(OFF_TYRE_SLIP_RATIO, index, value);
    }

    pub fn tyre_temp(&self, index: usize) -> f64 {
        self.wheel(OFF_TYRE_TEMP, index)
    }

    pub fn set_tyre_temp(&mut self, index: usize, value: f64) {
        self.set_wheel(OFF_TYRE_TEMP, index, value);
    }

    pub fn susp_velocity(&self, index: usize) -> f64 {
        self.wheel(OFF_SUSP_VELOCITY, index)
    }

    pub fn set_susp_velocity(&mut self, index: usize, value: f64) {
        self.set_wheel(OFF_SUSP_VELOCITY, index, value);
    }

    pub fn prox_radius(&self, index: usize) -> f64 {
        self.buf.get_f64(self.prox_offset(index, OFF_PROX_RADIUS))
    }

    pub fn set_prox_radius(&mut self, index: usize, value: f64) {
        self.buf
            .set_f64(self.prox_offset(index, OFF_PROX_RADIUS), value);
    }

    pub fn prox_theta(&self, index: usize) -> f64 {
        self.buf.get_f64(self.prox_offset(index, OFF_PROX_THETA))
    }

    pub fn set_prox_theta(&mut self, index: usize, value: f64) {
        self.buf
            .set_f64(self.prox_offset(index, OFF_PROX_THETA), value);
    }

    fn wheel(&self, base: usize, index: usize) -> f64 {
        if index >= WHEEL_COUNT {
            return 0.0;
        }
        self.buf.get_f64_wheel(base, index)
    }

    fn set_wheel(&mut self, base: usize, index: usize, value: f64) {
        if index >= WHEEL_COUNT {
            return;
        }
        self.buf.set_f64_wheel(base, index, value);
    }

    fn prox_offset(&self, index: usize, field: usize) -> usize {
        OFF_PROXIMITY + index * PROXIMITY_STRIDE + field
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn wheel_bytes() -> usize {
    WHEEL_COUNT * F64_SIZE
}
