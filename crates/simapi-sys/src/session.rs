use std::ffi::CString;

use crate::bindings;

const MAP_CREATE_FAILED: &str = "simapi_simmap_create returned null";
const CLEAR_FAILED: &str = "simapi_sim_clear failed";

pub struct GameSession {
    data: Box<bindings::SimData>,
    map: *mut bindings::SimMap,
}

impl Default for GameSession {
    fn default() -> Self {
        Self::new()
    }
}

impl GameSession {
    pub fn new() -> Self {
        let map = unsafe { bindings::simapi_simmap_create() };
        assert!(!map.is_null(), "{MAP_CREATE_FAILED}");
        unsafe {
            std::ptr::write_bytes(map, 0, 1);
            (*map).fd = -1;
        }
        Self {
            data: Box::new(bindings::SimData::default()),
            map,
        }
    }

    pub fn data_mut(&mut self) -> &mut bindings::SimData {
        &mut self.data
    }

    pub fn map(&self) -> *mut bindings::SimMap {
        self.map
    }

    pub fn game_id(name: &str) -> i32 {
        let token = CString::new(name).unwrap_or_else(|_| CString::new("").expect("empty"));
        unsafe { bindings::simapi_strtogame(token.as_ptr()) }
    }

    pub fn detect(&mut self, force_udp: bool, simd: bool) -> bindings::SimInfo {
        unsafe { bindings::simapi_get_sim(self.data.as_mut(), self.map, force_udp, None, simd) }
    }

    pub fn daemon_advancing(&mut self, probe: std::time::Duration) -> bool {
        if !self.data.simon {
            return false;
        }
        let first = self.data.mtick;
        std::thread::sleep(probe);
        unsafe {
            bindings::simapi_datamap(
                self.data.as_mut(),
                self.map,
                bindings::SimulatorAPI_SIMULATORAPI_SIMAPI_TEST,
                false,
                std::ptr::null_mut(),
            );
        }
        self.data.mtick != first
    }

    pub fn clear(&mut self, issimd: bool) -> Result<(), &'static str> {
        let rc = unsafe { bindings::simapi_sim_clear(self.data.as_mut(), self.map, issimd) };
        if rc != 0 {
            return Err(CLEAR_FAILED);
        }
        Ok(())
    }
}

impl Drop for GameSession {
    fn drop(&mut self) {
        if self.map.is_null() {
            return;
        }
        let fd = unsafe { (*self.map).fd };
        unsafe {
            bindings::simapi_universalmap_free(self.map);
        }
        if fd != -1 {
            unsafe { libc::free(self.map.cast()) };
        }
        self.map = std::ptr::null_mut();
    }
}
