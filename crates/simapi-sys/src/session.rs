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
        self.detect_with(force_udp, simd, None)
    }

    pub fn detect_with(
        &mut self,
        force_udp: bool,
        simd: bool,
        setup_udp: Option<unsafe extern "C" fn(i32) -> i32>,
    ) -> bindings::SimInfo {
        unsafe {
            bindings::simapi_get_sim(self.data.as_mut(), self.map, force_udp, setup_udp, simd)
        }
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

    pub fn map_live(&mut self, map_api: i32, udp: bool) {
        unsafe {
            bindings::simapi_datamap(
                self.data.as_mut(),
                self.map,
                map_api as bindings::SimulatorAPI,
                udp,
                std::ptr::null_mut(),
            );
        }
    }

    pub fn open_publish_map(&mut self) {
        unsafe {
            if (*self.map).addr.is_null() {
                bindings::simapi_universalmap_open(self.map, self.data.as_mut());
            }
        }
    }

    pub fn map_packet(&mut self, map_api: i32, packet: &mut [u8]) {
        if packet.is_empty() {
            return;
        }
        unsafe {
            bindings::simapi_datamap(
                self.data.as_mut(),
                self.map,
                map_api as bindings::SimulatorAPI,
                true,
                packet.as_mut_ptr().cast(),
            );
        }
    }

    pub fn write_frame(&mut self, bytes: &[u8]) -> bool {
        let len = std::mem::size_of::<bindings::SimData>();
        if bytes.len() < len {
            return false;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                (&mut *self.data as *mut bindings::SimData).cast(),
                len,
            );
        }
        true
    }

    pub fn frame_bytes(&self) -> &[u8] {
        let len = std::mem::size_of::<bindings::SimData>();
        unsafe {
            std::slice::from_raw_parts((&*self.data as *const bindings::SimData).cast::<u8>(), len)
        }
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
