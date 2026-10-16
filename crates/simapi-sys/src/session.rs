use std::ffi::CString;

use crate::bindings;

const MAP_CREATE_FAILED: &str = "simapi_simmap_create returned null";
const CLEAR_FAILED: &str = "simapi_sim_clear failed";
const CLOSED_FD: libc::c_int = -1;
const PUBLISH_MAP_PROTECTION: libc::c_int = libc::PROT_READ | libc::PROT_WRITE;

fn map_error(code: bindings::SimAPIError) -> i32 {
    code as i32
}

fn attached_addr(map: *mut bindings::SimMap) -> bool {
    if map.is_null() {
        return false;
    }
    let addr = unsafe { (*map).addr };
    !addr.is_null() && addr != libc::MAP_FAILED
}

fn fd_allows_write(fd: libc::c_int) -> bool {
    if fd < 0 {
        return false;
    }
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return false;
    }
    let access = flags & libc::O_ACCMODE;
    access == libc::O_RDWR || access == libc::O_WRONLY
}

fn publish_map_is_writable(map: *mut bindings::SimMap) -> bool {
    if !attached_addr(map) {
        return false;
    }
    fd_allows_write(unsafe { (*map).fd })
}

fn publish_map_is_read_write(map: *mut bindings::SimMap) -> bool {
    if !publish_map_is_writable(map) {
        return false;
    }
    let addr = unsafe { (*map).addr };
    unsafe {
        libc::mprotect(
            addr,
            std::mem::size_of::<bindings::SimData>(),
            PUBLISH_MAP_PROTECTION,
        ) == 0
    }
}

fn map_api_is_test(map_api: i32) -> bool {
    map_api == bindings::SimulatorAPI_SIMULATORAPI_SIMAPI_TEST as i32
}

fn detach_publish_view(map: *mut bindings::SimMap) {
    if map.is_null() {
        return;
    }
    unsafe {
        let addr = (*map).addr;
        if !addr.is_null() && addr != libc::MAP_FAILED {
            libc::munmap(addr, std::mem::size_of::<bindings::SimData>());
        }
        (*map).addr = std::ptr::null_mut();
        let fd = (*map).fd;
        if fd >= 0 {
            libc::close(fd);
            (*map).fd = CLOSED_FD;
        }
        (*map).hasSimApiDat = false;
    }
}

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
            (*map).fd = CLOSED_FD;
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
        self.datamap(map_api, udp, std::ptr::null_mut());
    }

    pub fn open_publish_map(&mut self) {
        let _ = self.try_open_publish_map();
    }

    pub fn try_open_publish_map(&mut self) -> i32 {
        if self.map.is_null() {
            return map_error(bindings::SimAPIError_SIMAPI_ERROR_UNKNOWN);
        }
        if publish_map_is_read_write(self.map) {
            return map_error(bindings::SimAPIError_SIMAPI_ERROR_NONE);
        }
        // A detected simd record is mapped read-only. Publishing into it
        // faults, so drop that view and open a writable SIMAPI.DAT.
        detach_publish_view(self.map);
        let result = unsafe { bindings::simapi_universalmap_open(self.map, self.data.as_mut()) };
        if result != bindings::SimAPIError_SIMAPI_ERROR_NONE as i32 {
            return result;
        }
        if publish_map_is_read_write(self.map) {
            return result;
        }
        map_error(bindings::SimAPIError_SIMAPI_ERROR_UNKNOWN)
    }

    pub fn map_open(&self) -> bool {
        !self.map.is_null() && unsafe { !(*self.map).addr.is_null() }
    }

    pub fn publish_bytes(&mut self, bytes: &[u8]) -> bool {
        if !publish_map_is_read_write(self.map) || !self.write_frame(bytes) {
            return false;
        }
        self.copy_data_to_map()
    }

    pub fn published_bytes(&self) -> Option<&[u8]> {
        if !self.map_open() {
            return None;
        }
        let len = std::mem::size_of::<bindings::SimData>();
        Some(unsafe { std::slice::from_raw_parts((*self.map).addr.cast(), len) })
    }

    fn copy_data_to_map(&mut self) -> bool {
        if !publish_map_is_read_write(self.map) {
            return false;
        }
        let len = std::mem::size_of::<bindings::SimData>();
        unsafe {
            std::ptr::copy_nonoverlapping(
                (&*self.data as *const bindings::SimData).cast::<u8>(),
                (*self.map).addr.cast(),
                len,
            );
        }
        true
    }

    pub fn map_packet(&mut self, map_api: i32, packet: &mut [u8]) {
        if packet.is_empty() {
            return;
        }
        self.datamap(map_api, true, packet.as_mut_ptr().cast());
    }

    fn datamap(&mut self, map_api: i32, udp: bool, base: *mut libc::c_char) {
        if self.map.is_null() {
            return;
        }
        // Game mappers read their own segments, then copy into `addr`.
        // Hide a view that cannot be made writable so that copy is skipped
        // and the read still updates this session.
        let hidden = self.hide_readonly_publish_view(map_api);
        unsafe {
            bindings::simapi_datamap(
                self.data.as_mut(),
                self.map,
                map_api as bindings::SimulatorAPI,
                udp,
                base,
            );
        }
        self.restore_publish_view(hidden);
    }

    fn hide_readonly_publish_view(&mut self, map_api: i32) -> Option<*mut libc::c_void> {
        if map_api_is_test(map_api) || publish_map_is_read_write(self.map) {
            return None;
        }
        let addr = unsafe { (*self.map).addr };
        unsafe {
            (*self.map).addr = std::ptr::null_mut();
        }
        Some(addr)
    }

    fn restore_publish_view(&mut self, hidden: Option<*mut libc::c_void>) {
        let Some(addr) = hidden else {
            return;
        };
        if self.map.is_null() {
            return;
        }
        unsafe {
            (*self.map).addr = addr;
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

    pub fn clear_status(&mut self, issimd: bool) -> i32 {
        unsafe { bindings::simapi_sim_clear(self.data.as_mut(), self.map, issimd) }
    }

    pub fn clear(&mut self, issimd: bool) -> Result<(), &'static str> {
        if self.clear_status(issimd) != 0 {
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
        if fd != CLOSED_FD {
            unsafe { libc::free(self.map.cast()) };
        }
        self.map = std::ptr::null_mut();
    }
}

#[cfg(test)]
mod tests {
    use super::{fd_allows_write, publish_map_is_read_write, GameSession, CLOSED_FD};
    use crate::bindings;

    const NULL_DEVICE: &std::ffi::CStr = c"/dev/null";
    const MEMFD_NAME: &std::ffi::CStr = c"cargopit-publish-test";

    fn view_len() -> usize {
        std::mem::size_of::<bindings::SimData>()
    }

    fn writable_memfd() -> libc::c_int {
        let fd = unsafe { libc::memfd_create(MEMFD_NAME.as_ptr(), 0) };
        assert!(fd >= 0);
        let len = libc::off_t::try_from(view_len()).expect("view length");
        assert_eq!(unsafe { libc::ftruncate(fd, len) }, 0);
        fd
    }

    fn map_view(fd: libc::c_int, prot: libc::c_int) -> bindings::SimMap {
        let addr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                view_len(),
                prot,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        assert_ne!(addr, libc::MAP_FAILED);
        let mut map: bindings::SimMap = unsafe { std::mem::zeroed() };
        map.fd = fd;
        map.addr = addr;
        map
    }

    fn unmap_view(map: &bindings::SimMap) {
        unsafe { libc::munmap(map.addr, view_len()) };
        unsafe { libc::close(map.fd) };
    }

    #[test]
    fn read_only_view_of_a_writable_fd_is_upgraded_for_publishing() {
        let fd = writable_memfd();
        let mut map = map_view(fd, libc::PROT_READ);
        assert!(publish_map_is_read_write(&mut map));
        // Upgraded mapping must accept a write without faulting.
        unsafe { map.addr.cast::<u8>().write(1) };
        unmap_view(&map);
    }

    #[test]
    fn view_of_a_read_only_fd_cannot_publish() {
        let writable = writable_memfd();
        let path = format!("/proc/self/fd/{writable}");
        let c_path = std::ffi::CString::new(path).expect("path");
        let readonly = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY) };
        assert!(readonly >= 0);
        let mut map = map_view(readonly, libc::PROT_READ);
        assert!(!publish_map_is_read_write(&mut map));
        unsafe { libc::munmap(map.addr, view_len()) };
        unsafe { libc::close(readonly) };
        unsafe { libc::close(writable) };
    }

    const MARKED_VELOCITY: u32 = 4_242;
    const PUBLISH_MARK: u8 = 0x5a;

    struct FdGuard(libc::c_int);

    impl Drop for FdGuard {
        fn drop(&mut self) {
            if self.0 < 0 {
                return;
            }
            unsafe { libc::close(self.0) };
            self.0 = CLOSED_FD;
        }
    }

    struct PublishView {
        session: GameSession,
    }

    impl PublishView {
        fn open(prot: libc::c_int) -> Self {
            let fd = writable_memfd();
            Self::from_fd(fd, prot)
        }

        fn open_readonly() -> (Self, FdGuard) {
            let backing = writable_memfd();
            let path = format!("/proc/self/fd/{backing}");
            let c_path = std::ffi::CString::new(path).expect("path");
            let readonly = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY) };
            assert!(readonly >= 0);
            (Self::from_fd(readonly, libc::PROT_READ), FdGuard(backing))
        }

        fn from_fd(fd: libc::c_int, prot: libc::c_int) -> Self {
            let session = GameSession::new();
            let view = map_view(fd, prot);
            unsafe {
                (*session.map).fd = view.fd;
                (*session.map).addr = view.addr;
            }
            Self { session }
        }
    }

    impl Drop for PublishView {
        fn drop(&mut self) {
            unsafe {
                let addr = (*self.session.map).addr;
                let fd = (*self.session.map).fd;
                (*self.session.map).addr = std::ptr::null_mut();
                (*self.session.map).fd = CLOSED_FD;
                (*self.session.map).hasSimApiDat = false;
                if !addr.is_null() && addr != libc::MAP_FAILED {
                    libc::munmap(addr, view_len());
                }
                if fd >= 0 {
                    libc::close(fd);
                }
            }
        }
    }

    fn lmu_map_api() -> i32 {
        bindings::SimulatorAPI_SIMULATORAPI_LMU as i32
    }

    #[test]
    fn readonly_fd_still_maps_telemetry_without_writing_the_view() {
        let (mut view, _backing) = PublishView::open_readonly();
        let before = view.session.published_bytes().expect("view").to_vec();
        view.session.data_mut().velocity = MARKED_VELOCITY;
        view.session.map_live(lmu_map_api(), false);
        let after = view.session.published_bytes().expect("view").to_vec();
        let velocity = view.session.data_mut().velocity;
        let mtick = view.session.data_mut().mtick;
        assert_eq!(after, before);
        assert_eq!(velocity, MARKED_VELOCITY);
        assert_ne!(mtick, 0);
    }

    #[test]
    fn writable_fd_still_receives_the_datamap_publish() {
        let mut view = PublishView::open(libc::PROT_READ);
        view.session.data_mut().velocity = MARKED_VELOCITY;
        view.session.map_live(lmu_map_api(), false);
        let published = view.session.published_bytes().expect("published").to_vec();
        let frame = view.session.frame_bytes().to_vec();
        let velocity = view.session.data_mut().velocity;
        assert_eq!(published, frame);
        assert_eq!(velocity, MARKED_VELOCITY);
    }

    #[test]
    fn publish_bytes_upgrades_a_read_only_view_of_a_writable_fd() {
        let mut view = PublishView::open(libc::PROT_READ);
        let mut bytes = vec![0u8; view_len()];
        bytes[0] = PUBLISH_MARK;
        assert!(view.session.publish_bytes(&bytes));
        let published = view.session.published_bytes().expect("published");
        assert_eq!(published[0], PUBLISH_MARK);
    }

    #[test]
    fn readonly_descriptor_cannot_publish() {
        let readonly = unsafe { libc::open(NULL_DEVICE.as_ptr(), libc::O_RDONLY) };
        assert!(readonly >= 0);
        assert!(!fd_allows_write(readonly));
        unsafe { libc::close(readonly) };

        let readwrite = unsafe { libc::open(NULL_DEVICE.as_ptr(), libc::O_RDWR) };
        assert!(readwrite >= 0);
        assert!(fd_allows_write(readwrite));
        unsafe { libc::close(readwrite) };
    }
}
