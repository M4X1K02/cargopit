//! HID, serial, sysfs, and Pulse transports. Fakes record writes. Real backends
//! report [`TransportError::Unavailable`] when the device or server is absent.
//! HID open is `hid_open(vid, pid, NULL)`; a device id is not part of selection.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportError {
    Unavailable,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IoRecord {
    pub op: &'static str,
    pub detail: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct FakeHid {
    next_id: u32,
    open: Option<HidEndpoint>,
    pub log: Vec<IoRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HidEndpoint {
    id: u32,
    vendor: u16,
    product: u16,
}

impl FakeHid {
    pub fn open(&mut self, vendor: u16, product: u16) -> Result<(), TransportError> {
        self.next_id = self.next_id.saturating_add(1);
        let endpoint = HidEndpoint {
            id: self.next_id,
            vendor,
            product,
        };
        self.open = Some(endpoint);
        self.log.push(IoRecord {
            op: "hid_open",
            detail: format!("id={} vid={vendor:#06x} pid={product:#06x}", endpoint.id),
            bytes: Vec::new(),
        });
        Ok(())
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, TransportError> {
        let endpoint = self.open.ok_or(TransportError::Unavailable)?;
        self.log.push(IoRecord {
            op: "hid_write",
            detail: format!("id={}", endpoint.id),
            bytes: data.to_vec(),
        });
        Ok(data.len())
    }

    pub fn send_feature(&mut self, data: &[u8]) -> Result<usize, TransportError> {
        let endpoint = self.open.ok_or(TransportError::Unavailable)?;
        self.log.push(IoRecord {
            op: "hid_send_feature_report",
            detail: format!("id={}", endpoint.id),
            bytes: data.to_vec(),
        });
        Ok(data.len())
    }

    pub fn close(&mut self) {
        if let Some(endpoint) = self.open.take() {
            self.log.push(IoRecord {
                op: "hid_close",
                detail: format!("id={}", endpoint.id),
                bytes: Vec::new(),
            });
        }
    }
}

pub struct RealHid {
    device: *mut std::ffi::c_void,
}

impl RealHid {
    /// Opens the first HID device with this vendor and product. `devid` is not used.
    pub fn open(vendor: u16, product: u16) -> Result<Self, TransportError> {
        if unsafe { hid_init() } != 0 {
            return Err(TransportError::Unavailable);
        }
        let device = unsafe { hid_open(vendor, product, std::ptr::null()) };
        if device.is_null() {
            return Err(TransportError::Unavailable);
        }
        Ok(Self { device })
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, TransportError> {
        if data.is_empty() {
            return Err(TransportError::Failed);
        }
        let wrote = unsafe { hid_write(self.device, data.as_ptr(), data.len()) };
        if wrote < 0 {
            return Err(TransportError::Failed);
        }
        Ok(wrote as usize)
    }
}

impl Drop for RealHid {
    fn drop(&mut self) {
        if !self.device.is_null() {
            unsafe { hid_close(self.device) };
            self.device = std::ptr::null_mut();
        }
    }
}

#[link(name = "hidapi-hidraw")]
unsafe extern "C" {
    fn hid_init() -> i32;
    fn hid_open(
        vendor_id: u16,
        product_id: u16,
        serial_number: *const libc::wchar_t,
    ) -> *mut std::ffi::c_void;
    fn hid_write(device: *mut std::ffi::c_void, data: *const u8, length: usize) -> i32;
    fn hid_close(device: *mut std::ffi::c_void);
}

#[derive(Clone, Debug, Default)]
pub struct FakeSerial {
    pub log: Vec<IoRecord>,
    open: bool,
}

impl FakeSerial {
    pub fn open(&mut self, path: &str, baud: u32) -> Result<(), TransportError> {
        if path.is_empty() {
            return Err(TransportError::Unavailable);
        }
        self.open = true;
        self.log.push(IoRecord {
            op: "serial_open",
            detail: format!("{path} baud={baud}"),
            bytes: Vec::new(),
        });
        Ok(())
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, TransportError> {
        if !self.open {
            return Err(TransportError::Unavailable);
        }
        self.log.push(IoRecord {
            op: "serial_write",
            detail: format!("len={}", data.len()),
            bytes: data.to_vec(),
        });
        Ok(data.len())
    }
}

pub struct RealSerial {
    port: serialport::TTYPort,
}

pub enum ShareWarning {
    NativeHandle,
    Exclusive,
    Hupcl,
}

impl RealSerial {
    pub fn open(path: &str, baud: u32) -> Result<Self, TransportError> {
        if path.is_empty() {
            return Err(TransportError::Unavailable);
        }
        let port = serialport::new(path, baud)
            .timeout(Duration::from_millis(SERIAL_TIMEOUT_MS))
            .open_native()
            .map_err(|_| TransportError::Unavailable)?;
        Ok(Self { port })
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, TransportError> {
        self.port.write(data).map_err(|_| TransportError::Failed)
    }

    /// Match `cargopit_serial_share_port`: drop exclusive open and hang-up-on-close.
    pub fn share(&self) -> Vec<ShareWarning> {
        use std::os::fd::AsRawFd;

        let fd = self.port.as_raw_fd();
        if fd < 0 {
            return vec![ShareWarning::NativeHandle];
        }
        let mut warnings = Vec::new();
        if unsafe { libc::ioctl(fd, TIOCNXCL) != 0 } {
            warnings.push(ShareWarning::Exclusive);
        }
        if !clear_hupcl(fd) {
            warnings.push(ShareWarning::Hupcl);
        }
        warnings
    }
}

fn clear_hupcl(fd: i32) -> bool {
    let mut term: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut term) != 0 } {
        return true;
    }
    term.c_cflag &= !libc::HUPCL;
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &term) == 0 }
}

const TIOCNXCL: libc::Ioctl = 0x5429;

const SERIAL_TIMEOUT_MS: u64 = 100;

#[derive(Clone, Debug, Default)]
pub struct FakeSysfs {
    pub log: Vec<IoRecord>,
}

impl FakeSysfs {
    pub fn write(&mut self, path: &str, data: &[u8]) -> Result<(), TransportError> {
        if path.is_empty() {
            return Err(TransportError::Unavailable);
        }
        self.log.push(IoRecord {
            op: "sysfs_write",
            detail: path.to_string(),
            bytes: data.to_vec(),
        });
        Ok(())
    }
}

pub fn sysfs_write(path: &str, data: &[u8]) -> Result<(), TransportError> {
    if path.is_empty() {
        return Err(TransportError::Unavailable);
    }
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|_| TransportError::Unavailable)?;
    file.write_all(data).map_err(|_| TransportError::Failed)?;
    file.flush().map_err(|_| TransportError::Failed)?;
    Ok(())
}

const PULSE_RATE: u32 = 48_000;
const PULSE_CHANNELS: u8 = 2;
const PULSE_SMOKE_FRAMES: usize = 48;
const PULSE_POLL_ATTEMPTS: u32 = 50;
const PULSE_POLL_MS: u64 = 20;
const PULSE_APP_NAME: &str = "cargopit";
pub const PULSE_HOST_APP_NAME: &str = "Cargopit";
pub const PULSE_CONTEXT_UNCONNECTED: i32 = 0;
pub const PULSE_CONTEXT_CONNECTING: i32 = 1;
pub const PULSE_CONTEXT_AUTHORIZING: i32 = 2;
pub const PULSE_CONTEXT_SETTING_NAME: i32 = 3;
pub const PULSE_CONTEXT_READY: i32 = 4;
pub const PULSE_CONTEXT_FAILED: i32 = 5;
pub const PULSE_CONTEXT_TERMINATED: i32 = 6;

#[derive(Clone, Debug, Default)]
pub struct FakePulse {
    pub log: Vec<IoRecord>,
    locked: bool,
    open: bool,
}

impl FakePulse {
    pub fn open(&mut self) -> Result<(), TransportError> {
        self.open = true;
        self.log.push(IoRecord {
            op: "pa_context_new",
            detail: PULSE_APP_NAME.to_string(),
            bytes: Vec::new(),
        });
        Ok(())
    }

    pub fn lock(&mut self) {
        self.locked = true;
        self.log.push(IoRecord {
            op: "pa_threaded_mainloop_lock",
            detail: String::new(),
            bytes: Vec::new(),
        });
    }

    pub fn unlock(&mut self) {
        self.locked = false;
        self.log.push(IoRecord {
            op: "pa_threaded_mainloop_unlock",
            detail: String::new(),
            bytes: Vec::new(),
        });
    }

    pub fn set_mute(&mut self, muted: bool) -> Result<(), TransportError> {
        if !self.open {
            return Err(TransportError::Unavailable);
        }
        self.log.push(IoRecord {
            op: "pa_context_set_sink_input_mute",
            detail: format!("mute={muted}"),
            bytes: Vec::new(),
        });
        Ok(())
    }

    pub fn write(&mut self, pcm: &[u8]) -> Result<usize, TransportError> {
        if !self.open || !self.locked {
            return Err(TransportError::Failed);
        }
        self.log.push(IoRecord {
            op: "pa_stream_write",
            detail: format!("len={}", pcm.len()),
            bytes: pcm.to_vec(),
        });
        Ok(pcm.len())
    }
}

pub struct RealPulse {
    mainloop: libpulse_binding::mainloop::threaded::Mainloop,
    context: libpulse_binding::context::Context,
}

impl RealPulse {
    pub fn connect() -> Result<Self, TransportError> {
        let mut mainloop = libpulse_binding::mainloop::threaded::Mainloop::new()
            .ok_or(TransportError::Unavailable)?;
        let mut context = libpulse_binding::context::Context::new(&mainloop, PULSE_APP_NAME)
            .ok_or(TransportError::Unavailable)?;
        context
            .connect(None, libpulse_binding::context::FlagSet::NOFLAGS, None)
            .map_err(|_| TransportError::Unavailable)?;
        mainloop.start().map_err(|_| TransportError::Unavailable)?;
        if !wait_ready(&mut mainloop, &context) {
            return Err(TransportError::Unavailable);
        }
        Ok(Self { mainloop, context })
    }

    pub fn lock(&mut self) {
        self.mainloop.lock();
    }

    pub fn unlock(&mut self) {
        self.mainloop.unlock();
    }

    pub fn smoke_write_and_mute(&mut self) -> Result<(), TransportError> {
        self.lock();
        let result = self.write_silence_and_mute();
        self.unlock();
        result
    }

    fn write_silence_and_mute(&mut self) -> Result<(), TransportError> {
        use libpulse_binding::sample::{Format, Spec};
        use libpulse_binding::stream::Stream;

        let spec = Spec {
            format: Format::S16le,
            channels: PULSE_CHANNELS,
            rate: PULSE_RATE,
        };
        if !spec.is_valid() {
            return Err(TransportError::Failed);
        }
        let mut stream = Stream::new(&mut self.context, PULSE_APP_NAME, &spec, None)
            .ok_or(TransportError::Unavailable)?;
        stream
            .connect_playback(
                None,
                None,
                libpulse_binding::stream::FlagSet::START_CORKED,
                None,
                None,
            )
            .map_err(|_| TransportError::Unavailable)?;
        let bytes = vec![0u8; PULSE_SMOKE_FRAMES * usize::from(PULSE_CHANNELS) * 2];
        stream
            .write_copy(&bytes, 0, libpulse_binding::stream::SeekMode::Relative)
            .map_err(|_| TransportError::Failed)?;
        if let Some(index) = stream.get_index() {
            self.context
                .introspect()
                .set_sink_input_mute(index, true, None);
        }
        Ok(())
    }
}

fn wait_ready(
    mainloop: &mut libpulse_binding::mainloop::threaded::Mainloop,
    context: &libpulse_binding::context::Context,
) -> bool {
    use libpulse_binding::context::State;

    poll_settled(mainloop, context) == Some(State::Ready)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PulseOutcome {
    Ready,
    ConnectFailed,
    ContextFailed { state: i32 },
    NotCreated,
}

pub struct PulseSession {
    mainloop: Option<libpulse_binding::mainloop::threaded::Mainloop>,
    context: Option<libpulse_binding::context::Context>,
    outcome: PulseOutcome,
}

impl PulseSession {
    pub fn open() -> Self {
        let Some(mut mainloop) = libpulse_binding::mainloop::threaded::Mainloop::new() else {
            return Self::from_parts(None, None, PulseOutcome::NotCreated);
        };
        let Some(mut context) =
            libpulse_binding::context::Context::new(&mainloop, PULSE_HOST_APP_NAME)
        else {
            return Self::from_parts(Some(mainloop), None, PulseOutcome::NotCreated);
        };
        if let Some(outcome) = reject_context(&mut mainloop, &mut context) {
            return Self::from_parts(Some(mainloop), Some(context), outcome);
        }
        let outcome = outcome_from_state(poll_settled(&mut mainloop, &context));
        Self::from_parts(Some(mainloop), Some(context), outcome)
    }

    pub fn outcome(&self) -> PulseOutcome {
        self.outcome
    }

    pub fn context_created(&self) -> bool {
        self.context.is_some()
    }

    fn from_parts(
        mainloop: Option<libpulse_binding::mainloop::threaded::Mainloop>,
        context: Option<libpulse_binding::context::Context>,
        outcome: PulseOutcome,
    ) -> Self {
        Self {
            mainloop,
            context,
            outcome,
        }
    }
}

impl Drop for PulseSession {
    fn drop(&mut self) {
        let Some(mainloop) = self.mainloop.as_mut() else {
            self.context.take();
            return;
        };
        mainloop.lock();
        drop(self.context.take());
        mainloop.unlock();
        self.mainloop.take();
    }
}

fn reject_context(
    mainloop: &mut libpulse_binding::mainloop::threaded::Mainloop,
    context: &mut libpulse_binding::context::Context,
) -> Option<PulseOutcome> {
    mainloop.lock();
    if mainloop.start().is_err() {
        mainloop.unlock();
        return Some(PulseOutcome::ConnectFailed);
    }
    if context
        .connect(None, libpulse_binding::context::FlagSet::NOFLAGS, None)
        .is_err()
    {
        mainloop.unlock();
        return Some(PulseOutcome::ConnectFailed);
    }
    mainloop.unlock();
    None
}

fn outcome_from_state(state: Option<libpulse_binding::context::State>) -> PulseOutcome {
    use libpulse_binding::context::State;

    let Some(state) = state else {
        return PulseOutcome::ConnectFailed;
    };
    if state == State::Ready {
        return PulseOutcome::Ready;
    }
    if state == State::Failed || state == State::Terminated {
        return PulseOutcome::ContextFailed {
            state: pulse_state_code(state),
        };
    }
    PulseOutcome::ConnectFailed
}

fn poll_settled(
    mainloop: &mut libpulse_binding::mainloop::threaded::Mainloop,
    context: &libpulse_binding::context::Context,
) -> Option<libpulse_binding::context::State> {
    use libpulse_binding::context::State;

    for _ in 0..PULSE_POLL_ATTEMPTS {
        mainloop.lock();
        let state = context.get_state();
        mainloop.unlock();
        if state == State::Ready || state == State::Failed || state == State::Terminated {
            return Some(state);
        }
        std::thread::sleep(Duration::from_millis(PULSE_POLL_MS));
    }
    None
}

fn pulse_state_code(state: libpulse_binding::context::State) -> i32 {
    use libpulse_binding::context::State;

    match state {
        State::Unconnected => PULSE_CONTEXT_UNCONNECTED,
        State::Connecting => PULSE_CONTEXT_CONNECTING,
        State::Authorizing => PULSE_CONTEXT_AUTHORIZING,
        State::SettingName => PULSE_CONTEXT_SETTING_NAME,
        State::Ready => PULSE_CONTEXT_READY,
        State::Failed => PULSE_CONTEXT_FAILED,
        State::Terminated => PULSE_CONTEXT_TERMINATED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_transports_record_writes() {
        let mut hid = FakeHid::default();
        hid.open(0x046d, 0xc24f).unwrap();
        let report = [0u8, 1, 2, 3];
        assert_eq!(hid.write(&report).unwrap(), report.len());
        hid.send_feature(&report).unwrap();
        hid.close();
        assert_eq!(hid.log[1].op, "hid_write");
        assert_eq!(hid.log[1].bytes, report);

        let mut serial = FakeSerial::default();
        serial.open("/dev/ttyACM0", 115_200).unwrap();
        serial.write(b"leds").unwrap();
        assert_eq!(serial.log[1].bytes, b"leds");

        let mut sysfs = FakeSysfs::default();
        sysfs
            .write("/sys/module/hid_fanatec/rumble", b"0\n")
            .unwrap();
        assert_eq!(sysfs.log[0].op, "sysfs_write");

        let mut pulse = FakePulse::default();
        pulse.open().unwrap();
        pulse.lock();
        pulse.set_mute(true).unwrap();
        let pcm = vec![0u8; 8];
        pulse.write(&pcm).unwrap();
        pulse.unlock();
        assert!(pulse
            .log
            .iter()
            .any(|record| record.op == "pa_stream_write"));
    }

    #[test]
    fn missing_hid_serial_and_sysfs_are_unavailable() {
        assert_eq!(RealHid::open(0, 0).err(), Some(TransportError::Unavailable));
        assert_eq!(
            RealSerial::open("/dev/cargopit-missing", 115_200).err(),
            Some(TransportError::Unavailable)
        );
        assert_eq!(
            sysfs_write("/sys/cargopit-missing", b"0\n").err(),
            Some(TransportError::Unavailable)
        );
    }

    #[test]
    fn pulse_context_state_codes_match_c() {
        use libpulse_binding::context::State;

        assert_eq!(State::Unconnected as i32, PULSE_CONTEXT_UNCONNECTED);
        assert_eq!(State::Connecting as i32, PULSE_CONTEXT_CONNECTING);
        assert_eq!(State::Authorizing as i32, PULSE_CONTEXT_AUTHORIZING);
        assert_eq!(State::SettingName as i32, PULSE_CONTEXT_SETTING_NAME);
        assert_eq!(State::Ready as i32, PULSE_CONTEXT_READY);
        assert_eq!(State::Failed as i32, PULSE_CONTEXT_FAILED);
        assert_eq!(State::Terminated as i32, PULSE_CONTEXT_TERMINATED);
        assert_eq!(pulse_state_code(State::Failed), PULSE_CONTEXT_FAILED);
    }

    #[test]
    fn pulse_smoke_accepts_a_missing_server() {
        match RealPulse::connect() {
            Ok(mut pulse) => pulse.smoke_write_and_mute().expect("pulse smoke"),
            Err(TransportError::Unavailable) => {}
            Err(TransportError::Failed) => panic!("pulse failed after the server answered"),
        }
        let session = PulseSession::open();
        match session.outcome() {
            PulseOutcome::Ready | PulseOutcome::ConnectFailed => {
                assert!(session.context_created());
            }
            PulseOutcome::ContextFailed { state } => {
                assert!(session.context_created());
                assert!(state == PULSE_CONTEXT_FAILED || state == PULSE_CONTEXT_TERMINATED);
            }
            PulseOutcome::NotCreated => panic!("pulse context was not created"),
        }
    }
}
