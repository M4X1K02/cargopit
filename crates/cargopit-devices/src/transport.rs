//! HID, serial, sysfs, and Pulse transports. Fakes record writes. Real backends
//! report [`TransportError::Unavailable`] when the device or server is absent.
//! HID open is `hid_open(vid, pid, NULL)`; a device id is not part of selection.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;

use crate::sound::{ShakerVoice, SharedShaker};

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
pub const PULSE_CONTEXT_MISSING: &str = "pulseaudio context is not ready";

const SHAKER_LATENCY_S: f64 = 0.040;
const SHAKER_GEAR_LATENCY_S: f64 = 0.040;
const SHAKER_PERCENT_SCALE: f64 = 100.0;
const SHAKER_BYTES_PER_SAMPLE: u32 = 2;
const SHAKER_SINK_UNMUTED: bool = false;
const SHAKER_BUFFER_DEFAULT: u32 = u32::MAX;
const SHAKER_CHANNELS_STEREO: u8 = 2;
const SHAKER_CHANNELS_QUAD: u8 = 4;
const SHAKER_CHANNELS_SURROUND_51: u8 = 6;
const SHAKER_CHANNELS_SURROUND_71: u8 = 8;
const SHAKER_MAP_STEREO: &str = "front-left,front-right";
const SHAKER_MAP_QUAD: &str = "front-left,front-right,rear-left,rear-right";
const SHAKER_MAP_SURROUND_51: &str = "front-left,front-right,front-center,lfe,rear-left,rear-right";
const SHAKER_MAP_SURROUND_71: &str =
    "front-left,front-right,front-center,lfe,rear-left,rear-right,side-left,side-right";
const SHAKER_PROP_MEDIA_NAME: &str = "media.name";
const SHAKER_PROP_MEDIA_ROLE: &str = "media.role";
const SHAKER_PROP_APP_NAME: &str = "application.name";
const SHAKER_PROP_APP_ID: &str = "application.id";
const SHAKER_PROP_NODE: &str = "node.name";
const SHAKER_PROP_EFFECT: &str = "cargopit.effect";
const SHAKER_PROP_TYRE: &str = "cargopit.tyre";
const SHAKER_MEDIA_ROLE: &str = "game";
const SHAKER_APP_ID: &str = "io.github.M4X1K02.cargopit";
const SHAKER_CHANNEL_BIT: u32 = 1;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShakerRequest {
    pub sink: String,
    pub node: String,
    pub stream_name: String,
    pub effect_name: String,
    pub tyre_name: Option<String>,
    pub volume_percent: i64,
    pub channels: u8,
    pub mask: u32,
    pub gear: bool,
}

pub struct PulseSession {
    mainloop: Option<libpulse_binding::mainloop::threaded::Mainloop>,
    context: Option<libpulse_binding::context::Context>,
    // Boxed so the write-callback pointer stays valid when another stream is stored.
    #[allow(clippy::vec_box)]
    streams: Vec<Box<libpulse_binding::stream::Stream>>,
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

    pub fn not_ready() -> Self {
        Self::from_parts(None, None, PulseOutcome::ConnectFailed)
    }

    pub fn connect_shaker(
        &mut self,
        request: &ShakerRequest,
        voice: Option<SharedShaker>,
    ) -> Result<(), String> {
        if self.outcome != PulseOutcome::Ready {
            return Err(PULSE_CONTEXT_MISSING.to_string());
        }
        let connected = self.connect_shaker_while_locked(request, voice);
        match connected {
            Ok(stream) => {
                self.streams.push(stream);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    fn connect_shaker_while_locked(
        &mut self,
        request: &ShakerRequest,
        voice: Option<SharedShaker>,
    ) -> Result<Box<libpulse_binding::stream::Stream>, String> {
        let Some(mainloop) = self.mainloop.as_mut() else {
            return Err(PULSE_CONTEXT_MISSING.to_string());
        };
        let Some(context) = self.context.as_mut() else {
            return Err(PULSE_CONTEXT_MISSING.to_string());
        };
        mainloop.lock();
        let connected = connect_shaker_locked(mainloop, context, request, voice);
        let result = match connected {
            Ok(stream) => Ok(stream),
            Err(()) => Err(context_errno(context)),
        };
        mainloop.unlock();
        result
    }

    pub fn disconnect_shakers(&mut self) {
        let Some(mainloop) = self.mainloop.as_mut() else {
            self.streams.clear();
            return;
        };
        mainloop.lock();
        for stream in self.streams.drain(..) {
            release_stream(*stream);
        }
        mainloop.unlock();
    }

    fn from_parts(
        mainloop: Option<libpulse_binding::mainloop::threaded::Mainloop>,
        context: Option<libpulse_binding::context::Context>,
        outcome: PulseOutcome,
    ) -> Self {
        Self {
            mainloop,
            context,
            streams: Vec::new(),
            outcome,
        }
    }
}

impl Drop for PulseSession {
    fn drop(&mut self) {
        self.disconnect_shakers();
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

fn connect_shaker_locked(
    mainloop: &mut libpulse_binding::mainloop::threaded::Mainloop,
    context: &mut libpulse_binding::context::Context,
    request: &ShakerRequest,
    voice: Option<SharedShaker>,
) -> Result<Box<libpulse_binding::stream::Stream>, ()> {
    use libpulse_binding::proplist::Proplist;
    use libpulse_binding::sample::{Format, Spec};
    use libpulse_binding::stream::Stream;

    let spec = Spec {
        format: Format::S16le,
        channels: request.channels,
        rate: PULSE_RATE,
    };
    if !spec.is_valid() {
        return Err(());
    }
    let map = shaker_channel_map(request.channels);
    let Some(mut props) = Proplist::new() else {
        return Err(());
    };
    if fill_shaker_props(&mut props, request).is_err() {
        return Err(());
    }
    let Some(stream) =
        Stream::new_with_proplist(context, &request.stream_name, &spec, Some(&map), &mut props)
    else {
        return Err(());
    };
    let mut boxed = Box::new(stream);
    let stream_ptr = boxed.as_mut() as *mut Stream;
    let attr = shaker_buffer(request);
    let volume = shaker_volumes(request);
    if boxed
        .connect_playback(
            sink_arg(&request.sink),
            Some(&attr),
            shaker_flags(),
            Some(&volume),
            None,
        )
        .is_err()
    {
        release_stream(*boxed);
        return Err(());
    }
    if !wait_for_stream(mainloop, stream_ptr) {
        release_stream(*boxed);
        return Err(());
    }
    boxed.set_write_callback(Some(Box::new(move |nbytes| {
        write_playback(stream_ptr, nbytes, voice.as_ref());
    })));
    unmute_shaker(context, &boxed);
    Ok(boxed)
}

fn shaker_flags() -> libpulse_binding::stream::FlagSet {
    use libpulse_binding::stream::FlagSet;

    FlagSet::INTERPOLATE_TIMING
        | FlagSet::AUTO_TIMING_UPDATE
        | FlagSet::ADJUST_LATENCY
        | FlagSet::START_UNMUTED
        | FlagSet::DONT_MOVE
}

fn shaker_channel_map(channels: u8) -> libpulse_binding::channelmap::Map {
    use libpulse_binding::channelmap::{Map, MapDef};

    let mut map = Map::default();
    map.init_auto(channels, MapDef::AIFF);
    let Some(spec) = shaker_map_spec(channels) else {
        return map;
    };
    Map::new_from_string(spec).unwrap_or(map)
}

fn shaker_map_spec(channels: u8) -> Option<&'static str> {
    match channels {
        SHAKER_CHANNELS_STEREO => Some(SHAKER_MAP_STEREO),
        SHAKER_CHANNELS_QUAD => Some(SHAKER_MAP_QUAD),
        SHAKER_CHANNELS_SURROUND_51 => Some(SHAKER_MAP_SURROUND_51),
        SHAKER_CHANNELS_SURROUND_71 => Some(SHAKER_MAP_SURROUND_71),
        _ => None,
    }
}

fn fill_shaker_props(
    props: &mut libpulse_binding::proplist::Proplist,
    request: &ShakerRequest,
) -> Result<(), ()> {
    props.set_str(SHAKER_PROP_MEDIA_NAME, &request.effect_name)?;
    props.set_str(SHAKER_PROP_MEDIA_ROLE, SHAKER_MEDIA_ROLE)?;
    props.set_str(SHAKER_PROP_APP_NAME, PULSE_HOST_APP_NAME)?;
    props.set_str(SHAKER_PROP_APP_ID, SHAKER_APP_ID)?;
    props.set_str(SHAKER_PROP_NODE, &request.node)?;
    props.set_str(SHAKER_PROP_EFFECT, &request.effect_name)?;
    if let Some(tyre) = request.tyre_name.as_deref() {
        props.set_str(SHAKER_PROP_TYRE, tyre)?;
    }
    Ok(())
}

fn shaker_buffer(request: &ShakerRequest) -> libpulse_binding::def::BufferAttr {
    let latency = if request.gear {
        SHAKER_GEAR_LATENCY_S
    } else {
        SHAKER_LATENCY_S
    };
    libpulse_binding::def::BufferAttr {
        maxlength: SHAKER_BUFFER_DEFAULT,
        tlength: bytes_for_duration(request.channels, latency),
        prebuf: SHAKER_BUFFER_DEFAULT,
        minreq: SHAKER_BUFFER_DEFAULT,
        fragsize: SHAKER_BUFFER_DEFAULT,
    }
}

fn bytes_for_duration(channels: u8, seconds: f64) -> u32 {
    if seconds <= 0.0 || channels == 0 {
        return 0;
    }
    let bytes =
        f64::from(PULSE_RATE) * seconds * f64::from(channels) * f64::from(SHAKER_BYTES_PER_SAMPLE);
    if bytes >= f64::from(u32::MAX) {
        return u32::MAX;
    }
    bytes as u32
}

fn shaker_volumes(request: &ShakerRequest) -> libpulse_binding::volume::ChannelVolumes {
    use libpulse_binding::volume::ChannelVolumes;

    let mut volumes = ChannelVolumes::default();
    volumes.mute(request.channels);
    let level = shaker_volume(request.volume_percent);
    let active = active_channel_mask(request.mask, request.channels);
    let Some(slots) = volumes.get_mut().get_mut(..usize::from(request.channels)) else {
        return volumes;
    };
    for (index, slot) in slots.iter_mut().enumerate() {
        let bit = SHAKER_CHANNEL_BIT << index;
        if active & bit != 0 {
            *slot = level;
        }
    }
    volumes
}

fn shaker_volume(percent: i64) -> libpulse_binding::volume::Volume {
    use libpulse_binding::volume::Volume;

    if percent <= 0 {
        return Volume::MUTED;
    }
    let scaled = (percent as f64 / SHAKER_PERCENT_SCALE) * f64::from(Volume::NORMAL.0);
    if scaled >= f64::from(Volume::MAX.0) {
        return Volume::MAX;
    }
    Volume(scaled as u32)
}

fn active_channel_mask(mask: u32, channels: u8) -> u32 {
    let all = if channels == 0 || channels >= u32::BITS as u8 {
        0
    } else {
        (SHAKER_CHANNEL_BIT << channels) - SHAKER_CHANNEL_BIT
    };
    let bits = mask & all;
    if bits == 0 {
        return all;
    }
    bits
}

fn sink_arg(sink: &str) -> Option<&str> {
    if sink.is_empty() {
        return None;
    }
    Some(sink)
}

fn wait_for_stream(
    mainloop: &mut libpulse_binding::mainloop::threaded::Mainloop,
    stream: *mut libpulse_binding::stream::Stream,
) -> bool {
    use libpulse_binding::stream::State;

    for _ in 0..PULSE_POLL_ATTEMPTS {
        match stream_state(stream) {
            State::Ready => return true,
            State::Failed | State::Terminated => return false,
            _ => {}
        }
        mainloop.unlock();
        std::thread::sleep(Duration::from_millis(PULSE_POLL_MS));
        mainloop.lock();
    }
    stream_state(stream) == State::Ready
}

fn unmute_shaker(
    context: &mut libpulse_binding::context::Context,
    stream: &libpulse_binding::stream::Stream,
) {
    let Some(index) = stream.get_index() else {
        return;
    };
    context
        .introspect()
        .set_sink_input_mute(index, SHAKER_SINK_UNMUTED, None);
}

fn release_stream(mut stream: libpulse_binding::stream::Stream) {
    stream.set_state_callback(None);
    stream.set_write_callback(None);
    let _ = stream.disconnect();
}

fn write_playback(
    stream: *mut libpulse_binding::stream::Stream,
    nbytes: usize,
    voice: Option<&SharedShaker>,
) {
    let Some(voice) = voice else {
        write_silence(stream, nbytes);
        return;
    };
    let bytes = render_voice(voice, nbytes);
    write_bytes(stream, &bytes);
}

fn render_voice(voice: &Mutex<ShakerVoice>, nbytes: usize) -> Vec<u8> {
    if nbytes == 0 {
        return Vec::new();
    }
    let mut voice = voice.lock().unwrap_or_else(|poison| poison.into_inner());
    voice.render(nbytes)
}

fn write_bytes(stream: *mut libpulse_binding::stream::Stream, bytes: &[u8]) {
    use libpulse_binding::stream::SeekMode;

    if stream.is_null() || bytes.is_empty() {
        return;
    }
    let _ = unsafe { (*stream).write_copy(bytes, 0, SeekMode::Relative) };
}

fn write_silence(stream: *mut libpulse_binding::stream::Stream, nbytes: usize) {
    use libpulse_binding::stream::SeekMode;

    if stream.is_null() || nbytes == 0 {
        return;
    }
    let bytes = vec![0u8; nbytes];
    let _ = unsafe { (*stream).write_copy(&bytes, 0, SeekMode::Relative) };
}

fn stream_state(stream: *mut libpulse_binding::stream::Stream) -> libpulse_binding::stream::State {
    use libpulse_binding::stream::State;

    if stream.is_null() {
        return State::Failed;
    }
    unsafe { (*stream).get_state() }
}

fn context_errno(context: &libpulse_binding::context::Context) -> String {
    context
        .errno()
        .to_string()
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| PULSE_CONTEXT_MISSING.to_string())
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

    #[test]
    fn shaker_volume_scales_the_percent_the_way_c_does() {
        use libpulse_binding::volume::Volume;

        assert_eq!(shaker_volume(0), Volume::MUTED);
        assert_eq!(shaker_volume(100), Volume::NORMAL);
        let forty = (40.0 / SHAKER_PERCENT_SCALE) * f64::from(Volume::NORMAL.0);
        assert_eq!(shaker_volume(40), Volume(forty as u32));
        assert_eq!(
            bytes_for_duration(SHAKER_CHANNELS_STEREO, SHAKER_LATENCY_S),
            (f64::from(PULSE_RATE)
                * SHAKER_LATENCY_S
                * f64::from(SHAKER_CHANNELS_STEREO)
                * f64::from(SHAKER_BYTES_PER_SAMPLE)) as u32
        );
    }

    #[test]
    fn shaker_connect_reports_a_missing_context() {
        let mut session = PulseSession::not_ready();
        let err = session
            .connect_shaker(
                &ShakerRequest {
                    sink: "alsa_output.test".to_string(),
                    node: "cargopit.Gear".to_string(),
                    stream_name: "Gear".to_string(),
                    effect_name: "Gear".to_string(),
                    tyre_name: None,
                    volume_percent: 40,
                    channels: SHAKER_CHANNELS_STEREO,
                    mask: (SHAKER_CHANNEL_BIT << SHAKER_CHANNELS_STEREO) - SHAKER_CHANNEL_BIT,
                    gear: true,
                },
                None,
            )
            .expect_err("missing context");
        assert_eq!(err, PULSE_CONTEXT_MISSING);
    }

    #[test]
    fn shaker_stream_connects_when_pulse_is_ready() {
        const SAMPLE_VOLUME: i64 = 40;
        let mut session = PulseSession::open();
        if session.outcome() != PulseOutcome::Ready {
            return;
        }
        let request = ShakerRequest {
            sink: String::new(),
            node: "cargopit.Gear".to_string(),
            stream_name: "Gear".to_string(),
            effect_name: "Gear".to_string(),
            tyre_name: None,
            volume_percent: SAMPLE_VOLUME,
            channels: SHAKER_CHANNELS_STEREO,
            mask: (SHAKER_CHANNEL_BIT << SHAKER_CHANNELS_STEREO) - SHAKER_CHANNEL_BIT,
            gear: true,
        };
        session
            .connect_shaker(&request, None)
            .expect("shaker playback");
        session.disconnect_shakers();
    }
}
