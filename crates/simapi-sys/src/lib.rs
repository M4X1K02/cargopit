//! Bindings and safe wrappers for the checked-out simapi submodule.

mod session;
mod telemetry;

#[allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    clippy::all
)]
pub mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

include!(concat!(env!("OUT_DIR"), "/layout.rs"));

pub use session::GameSession;
pub use telemetry::{
    read_telemetry, write_telemetry, TelemetrySnapshot, TELEMETRY_GEARC_LEN, TELEMETRY_NAME_LEN,
};

const EXPECTED_SIMDATA_SIZE: usize = 46044;
const EXPECTED_SIMAPI_VERSION: u32 = 1;

const _: () = assert!(SIMDATA_SIZE == EXPECTED_SIMDATA_SIZE);
const _: () = assert!(SIMAPI_VERSION_VALUE == EXPECTED_SIMAPI_VERSION);

#[cfg(test)]
mod tests {
    use super::*;

    const ASSETTO_CORSA_TOKEN: &str = "ac";

    #[test]
    fn layout_matches_submodule() {
        assert_eq!(SIMDATA_SIZE, EXPECTED_SIMDATA_SIZE);
        assert_eq!(std::mem::size_of::<bindings::SimData>(), SIMDATA_SIZE);
        assert_eq!(SIMAPI_VERSION_VALUE, EXPECTED_SIMAPI_VERSION);
    }

    #[test]
    fn game_token_and_clear_do_not_touch_shm() {
        assert_eq!(
            GameSession::game_id(ASSETTO_CORSA_TOKEN),
            bindings::SimulatorEXE_SIMULATOREXE_ASSETTO_CORSA as i32
        );
        let mut session = GameSession::new();
        session.clear(false).expect("clear");
    }

    #[test]
    fn telemetry_roundtrip_preserves_view_fields() {
        let mut bytes = vec![0u8; SIMDATA_SIZE];
        let mut view = TelemetrySnapshot {
            mtick: 9,
            rpms: 4500,
            velocity: 142,
            gear: 4,
            gearc: *b"3\0\0\0",
            gas: 0.5,
            simon: 1,
            ..TelemetrySnapshot::default()
        };
        view.car[..4].copy_from_slice(b"mx5\0");
        assert!(write_telemetry(&mut bytes, &view));
        let parsed = read_telemetry(&bytes).expect("telemetry");
        assert_eq!(parsed.mtick, 9);
        assert_eq!(parsed.rpms, 4500);
        assert_eq!(parsed.velocity, 142);
        assert_eq!(parsed.gear, 4);
        assert_eq!(parsed.gearc[0], b'3');
        assert_eq!(parsed.car[0], b'm');
        assert_eq!(parsed.gas, 0.5);
        assert_eq!(parsed.simon, 1);
        assert_eq!(parsed.simapiversion, SIMAPI_VERSION_VALUE as u8);
        assert_eq!(parsed.valid, 1);
    }
}

pub const WHEEL_COUNT: usize = 4;
pub const CAR_NAME_BYTES: usize = 128;
pub const GEAR_CHAR_BYTES: usize = 3;

#[derive(Clone)]
pub struct SimDataBuf {
    bytes: Vec<u8>,
}

impl SimDataBuf {
    pub fn new() -> Self {
        Self {
            bytes: vec![0; SIMDATA_SIZE],
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn set_u8(&mut self, offset: usize, value: u8) {
        self.require(offset, 1);
        self.bytes[offset] = value;
    }

    pub fn set_u32(&mut self, offset: usize, value: u32) {
        self.require(offset, 4);
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub fn set_u64(&mut self, offset: usize, value: u64) {
        self.require(offset, 8);
        self.bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    pub fn set_f64(&mut self, offset: usize, value: f64) {
        self.require(offset, F64_SIZE);
        self.bytes[offset..offset + F64_SIZE].copy_from_slice(&value.to_le_bytes());
    }

    pub fn set_bool(&mut self, offset: usize, value: bool) {
        self.require(offset, BOOL_SIZE);
        self.bytes[offset] = u8::from(value);
    }

    pub fn set_f64_wheel(&mut self, base: usize, index: usize, value: f64) {
        self.set_f64(base + index * F64_SIZE, value);
    }

    pub fn set_bytes(&mut self, offset: usize, value: &[u8]) {
        self.require(offset, value.len());
        self.bytes[offset..offset + value.len()].copy_from_slice(value);
    }

    pub fn get_u8(&self, offset: usize) -> u8 {
        self.require(offset, 1);
        self.bytes[offset]
    }

    pub fn get_u32(&self, offset: usize) -> u32 {
        self.require(offset, 4);
        u32::from_le_bytes(self.bytes[offset..offset + 4].try_into().expect("u32"))
    }

    pub fn get_u64(&self, offset: usize) -> u64 {
        self.require(offset, 8);
        u64::from_le_bytes(self.bytes[offset..offset + 8].try_into().expect("u64"))
    }

    pub fn get_f64(&self, offset: usize) -> f64 {
        self.require(offset, F64_SIZE);
        f64::from_le_bytes(
            self.bytes[offset..offset + F64_SIZE]
                .try_into()
                .expect("f64"),
        )
    }

    pub fn get_f64_wheel(&self, base: usize, index: usize) -> f64 {
        self.get_f64(base + index * F64_SIZE)
    }

    pub fn get_bytes(&self, offset: usize, len: usize) -> &[u8] {
        self.require(offset, len);
        &self.bytes[offset..offset + len]
    }

    fn require(&self, offset: usize, len: usize) {
        if offset
            .checked_add(len)
            .is_none_or(|end| end > self.bytes.len())
        {
            panic!("SimData field write is outside the submodule layout");
        }
    }
}

impl Default for SimDataBuf {
    fn default() -> Self {
        Self::new()
    }
}
