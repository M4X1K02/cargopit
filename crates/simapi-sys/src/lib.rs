//! Offsets and size of `SimData` taken from the simapi submodule at build time.
//! Callers write bytes at these offsets. They do not declare a Rust layout.

include!(concat!(env!("OUT_DIR"), "/layout.rs"));

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

    fn require(&self, offset: usize, len: usize) {
        if offset.checked_add(len).is_none_or(|end| end > self.bytes.len()) {
            panic!("SimData field write is outside the submodule layout");
        }
    }
}

impl Default for SimDataBuf {
    fn default() -> Self {
        Self::new()
    }
}
