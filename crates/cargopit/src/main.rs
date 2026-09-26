//! Rust host. `play` discovers a live sim after simd is running.

use std::env;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::process::ExitCode;
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;
use std::time::Duration;

use cargopit::acr;
use cargopit::cli::{self, ProgramAction};
use cargopit::control::{self, ControlEffect, SessionStatus};
use cargopit::devices::LoadedDevices;
use cargopit::games::{self, PlayAction, PlayPhase, SeenSim};
use cargopit::log::{self, Level};
use cargopit::scheduler::{self, TimerKind};
use cargopit::simd::{self, EnsureStatus};
use cargopit::tach;
use cargopit::testmode;
use cargopit::tyres::{self, TyreSimFlags};
use cargopit::udp;
use cargopit_devices::clock::{Clock, SystemClock};
use cargopit_devices::telemetry::Telemetry;
use simapi_sys::GameSession;

const STOP_SIGNAL_NONE: i32 = 0;
static STOP_SIGNAL: AtomicI32 = AtomicI32::new(STOP_SIGNAL_NONE);

/// # Safety
/// Installed as a POSIX signal handler. It only stores the signal number.
unsafe extern "C" fn on_stop_signal(signum: i32) {
    STOP_SIGNAL.store(signum, Ordering::Relaxed);
}

fn install_stop_signals() {
    unsafe {
        libc::signal(
            libc::SIGINT,
            on_stop_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            on_stop_signal as *const () as libc::sighandler_t,
        );
    }
}

fn read_quit_key() -> Option<u8> {
    let stdin = io::stdin();
    let flags = unsafe { libc::fcntl(stdin.as_raw_fd(), libc::F_GETFL) };
    if flags >= 0 {
        unsafe {
            libc::fcntl(stdin.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }
    let mut byte = [0u8; 1];
    match stdin.lock().read(&mut byte) {
        Ok(1) => Some(byte[0]),
        _ => None,
    }
}
use std::net::UdpSocket;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let parsed = cli::parse(&args);
    if let Some(line) = &parsed.version_line {
        println!("{line}");
        return ExitCode::SUCCESS;
    }
    if parsed.usage || parsed.action == ProgramAction::Exit {
        print!("{}", cli::usage_text());
        return ExitCode::SUCCESS;
    }
    match parsed.action {
        ProgramAction::Play => play(&parsed),
        ProgramAction::Test => test_mode(&parsed),
        ProgramAction::ConfigTach => config_tach(&parsed),
        ProgramAction::Exit => ExitCode::SUCCESS,
    }
}

fn play(parsed: &cli::Invocation) -> ExitCode {
    if !prepare_host(parsed) {
        return ExitCode::SUCCESS;
    }
    slog(parsed, Level::Info, games::MSG_GAMELOOP_MODE);
    let code = match simd::ensure() {
        EnsureStatus::Ok => {
            let _ = run_discovery(parsed);
            games::ERROR_NONE
        }
        EnsureStatus::NotInstalled => games::ERROR_SIMD_REQUIRED,
        EnsureStatus::StartFailed => games::ERROR_UNKNOWN,
    };
    log_action_exit(
        parsed,
        code,
        games::game_loop_exit_message,
        games::game_loop_fail_message,
    );
    ExitCode::SUCCESS
}

struct PlayLoop {
    phase: PlayPhase,
    use_udp: bool,
    map_api: i32,
    wait_ms: u64,
    user_stopped: bool,
    releasing: bool,
    sim_exe: u64,
}

struct DeviceLoop {
    loaded: LoadedDevices,
    scheduler: scheduler::Scheduler,
    pending: bool,
    tyres: TyreWatch,
}

struct TyreWatch {
    active: bool,
    config_checked: bool,
    calculates_diameter: bool,
    supports_haptics: bool,
    calculates_slip: bool,
}

impl TyreWatch {
    fn new() -> Self {
        Self {
            active: false,
            config_checked: false,
            calculates_diameter: false,
            supports_haptics: false,
            calculates_slip: false,
        }
    }

    fn note(&mut self, flags: TyreSimFlags) {
        self.calculates_diameter = flags.calculates_diameter;
        self.supports_haptics = flags.supports_haptics;
        self.calculates_slip = flags.calculates_slip;
    }
}

struct Observed {
    seen: SeenSim,
    flags: TyreSimFlags,
}

fn run_discovery(parsed: &cli::Invocation) -> ExitCode {
    let mut session = GameSession::new();
    let mut snapshot = games::FrameSnapshot::new();
    let mut socket: Option<UdpSocket> = None;
    install_stop_signals();
    let clock = SystemClock::new();
    let control_path = control::socket_path(
        std::env::var(control::RUNTIME_DIR_ENV).ok().as_deref(),
        control::current_uid(),
    );
    let control = control::bind_listener(&control_path).ok();
    let mut devices = DeviceLoop {
        loaded: LoadedDevices::empty(),
        scheduler: scheduler::Scheduler::new(),
        pending: false,
        tyres: TyreWatch::new(),
    };
    let mut play = PlayLoop {
        phase: PlayPhase::Searching,
        use_udp: parsed.force_udp,
        map_api: games::MAP_API_SIMD,
        wait_ms: games::CHECK_INTERVAL_MS,
        user_stopped: false,
        releasing: false,
        sim_exe: 0,
    };
    println!("{}", games::MSG_SEARCHING);
    loop {
        play = poll_once(
            &mut PlayParts {
                session: &mut session,
                snapshot: &mut snapshot,
                socket: &mut socket,
                clock: &clock,
                devices: &mut devices,
                control: control.as_ref(),
            },
            play,
            parsed,
        );
        if play.phase == PlayPhase::Exiting {
            println!();
            return ExitCode::SUCCESS;
        }
        thread::sleep(Duration::from_millis(play.wait_ms));
    }
}

struct PlayParts<'a> {
    session: &'a mut GameSession,
    snapshot: &'a mut games::FrameSnapshot,
    socket: &'a mut Option<UdpSocket>,
    clock: &'a SystemClock,
    devices: &'a mut DeviceLoop,
    control: Option<&'a std::os::unix::net::UnixListener>,
}

fn poll_once(parts: &mut PlayParts<'_>, play: PlayLoop, parsed: &cli::Invocation) -> PlayLoop {
    let force_udp = parsed.force_udp;
    let fps = parsed.fps;
    let mut observed = observe(parts.session, force_udp, false);
    if play.phase == PlayPhase::Searching && games::simd_map_is_stale(observed.seen.map_api, false)
    {
        let advancing = parts
            .session
            .daemon_advancing(Duration::from_micros(games::DAEMON_PROBE_US));
        if games::simd_map_is_stale(observed.seen.map_api, advancing) {
            observed = observe(parts.session, force_udp, true);
        }
    }
    parts.devices.tyres.note(observed.flags);
    let seen = bridge_if_needed(observed.seen);
    let play = note_sim(play, seen.sim_exe);
    let play = apply_quit(play, parsed, parts.devices.loaded.len());
    if play.phase == PlayPhase::Exiting {
        return play;
    }
    let play = apply_control(parts.control, parts.devices, play, parsed);
    if play.phase == PlayPhase::Exiting {
        return play;
    }
    if play.releasing {
        return finish_release(parts, play, parsed);
    }
    match play.phase {
        PlayPhase::Searching => match games::search_tick(seen, force_udp, play.user_stopped) {
            PlayAction::StartMapping { use_udp } => {
                parts.devices.pending = true;
                begin_mapping(parts.session, parts.snapshot, parts.socket, seen, use_udp)
            }
            PlayAction::Wait | PlayAction::Release => searching(play),
        },
        PlayPhase::Mapping => {
            let next = map_or_release(
                parts.session,
                parts.snapshot,
                parts.socket,
                parts.clock,
                play,
                seen,
                fps,
            );
            if next.phase != PlayPhase::Mapping {
                announce_release(parsed);
                release_configured(parts.devices);
                if next.user_stopped {
                    slog(parsed, Level::Info, games::MSG_STOPPED_MAPPING);
                } else {
                    slog(parsed, Level::Info, games::MSG_RESTART_CHECK);
                }
            }
            if next.phase == PlayPhase::Mapping {
                tick_devices(parts, parsed);
            }
            next
        }
        PlayPhase::Exiting => play,
    }
}

fn apply_quit(play: PlayLoop, parsed: &cli::Invocation, device_count: usize) -> PlayLoop {
    let mapping = play.phase == PlayPhase::Mapping;
    let signum = STOP_SIGNAL.swap(STOP_SIGNAL_NONE, Ordering::Relaxed);
    let release_devices = mapping && device_count > 0 && !play.releasing;
    match games::quit_action(read_quit_key(), signum != STOP_SIGNAL_NONE, mapping) {
        games::QuitAction::Continue => play,
        games::QuitAction::Exit => {
            announce_exit(parsed, signum, release_devices);
            PlayLoop {
                phase: PlayPhase::Exiting,
                ..play
            }
        }
        games::QuitAction::Release => {
            println!("{}", games::MSG_USER_STOP);
            slog(parsed, Level::Info, games::MSG_USER_STOP);
            announce_release(parsed);
            PlayLoop {
                user_stopped: true,
                releasing: true,
                ..play
            }
        }
    }
}

fn announce_exit(parsed: &cli::Invocation, signum: i32, release_devices: bool) {
    if signum != STOP_SIGNAL_NONE {
        slog(parsed, Level::Info, &games::signal_stop_message(signum));
    }
    slog(parsed, Level::Info, games::MSG_EXITING);
    if !release_devices {
        return;
    }
    announce_release(parsed);
}

fn announce_release(parsed: &cli::Invocation) {
    slog(parsed, Level::Info, games::MSG_RELEASE_LOOP);
    slog(parsed, Level::Info, games::MSG_RELEASING_DEVICES);
}

fn apply_control(
    control: Option<&std::os::unix::net::UnixListener>,
    devices: &mut DeviceLoop,
    play: PlayLoop,
    parsed: &cli::Invocation,
) -> PlayLoop {
    let Some(listener) = control else {
        return play;
    };
    let status = session_view(&play, devices, parsed);
    match control::try_accept(listener, &status) {
        Some(ControlEffect::Stop) => {
            let release_devices =
                play.phase == PlayPhase::Mapping && !devices.loaded.is_empty() && !play.releasing;
            announce_exit(parsed, STOP_SIGNAL_NONE, release_devices);
            PlayLoop {
                phase: PlayPhase::Exiting,
                ..play
            }
        }
        Some(ControlEffect::Reload) => {
            if play.phase == PlayPhase::Mapping {
                slog(parsed, Level::Info, games::MSG_RELOAD);
                announce_release(parsed);
                slog(parsed, Level::Info, games::MSG_RESTART_CHECK);
            }
            release_configured(devices);
            devices.pending = true;
            searching(PlayLoop {
                user_stopped: false,
                ..play
            })
        }
        Some(ControlEffect::None) | None => play,
    }
}

fn session_view(play: &PlayLoop, devices: &DeviceLoop, parsed: &cli::Invocation) -> SessionStatus {
    let updates = (0..devices.loaded.len())
        .map(|index| devices.loaded.updates(index))
        .sum();
    let state = match play.phase {
        PlayPhase::Searching => games::STATE_SEARCHING,
        PlayPhase::Mapping => games::STATE_MAPPING,
        PlayPhase::Exiting => games::STATE_EXITING,
    };
    SessionStatus {
        state: state.to_string(),
        releasing: play.releasing,
        paused: play.user_stopped,
        config_index: parsed.config_index,
        sim: games::sim_display_name(play.sim_exe, play.phase == PlayPhase::Exiting),
        devices: devices.loaded.len() as u64,
        updates,
        overruns: 0,
    }
}

fn note_sim(play: PlayLoop, sim_exe: u64) -> PlayLoop {
    PlayLoop { sim_exe, ..play }
}

fn searching(play: PlayLoop) -> PlayLoop {
    PlayLoop {
        phase: PlayPhase::Searching,
        wait_ms: games::CHECK_INTERVAL_MS,
        releasing: false,
        ..play
    }
}

fn finish_release(parts: &mut PlayParts<'_>, play: PlayLoop, parsed: &cli::Invocation) -> PlayLoop {
    let _ = parts.session.clear(false);
    release_configured(parts.devices);
    if play.user_stopped {
        slog(parsed, Level::Info, games::MSG_STOPPED_MAPPING);
    }
    searching(play)
}

fn release_configured(devices: &mut DeviceLoop) {
    devices.loaded = LoadedDevices::empty();
    devices.scheduler.clear();
    devices.pending = false;
    devices.tyres.active = false;
    devices.tyres.config_checked = false;
}

fn begin_mapping(
    session: &mut GameSession,
    snapshot: &mut games::FrameSnapshot,
    socket: &mut Option<UdpSocket>,
    seen: SeenSim,
    use_udp: bool,
) -> PlayLoop {
    if seen.map_api != games::MAP_API_SIMD {
        session.open_publish_map();
    }
    if use_udp {
        session.map_live(seen.map_api, true);
        snapshot.publish(session.frame_bytes());
        if socket.is_none() {
            *socket = udp::bind_requested();
        }
    }
    PlayLoop {
        phase: PlayPhase::Mapping,
        use_udp,
        map_api: seen.map_api,
        wait_ms: if use_udp {
            games::CHECK_INTERVAL_MS
        } else {
            games::MAPPING_START_MS
        },
        user_stopped: false,
        releasing: false,
        sim_exe: seen.sim_exe,
    }
}

fn map_or_release(
    session: &mut GameSession,
    snapshot: &mut games::FrameSnapshot,
    socket: &mut Option<UdpSocket>,
    clock: &impl Clock,
    play: PlayLoop,
    seen: SeenSim,
    fps: i32,
) -> PlayLoop {
    if games::mapping_tick(seen) == PlayAction::Release || games::mapping_should_stop(seen) {
        let _ = session.clear(false);
        return searching(play);
    }
    if play.use_udp {
        recv_udp(session, snapshot, socket, clock, play.map_api);
        return PlayLoop {
            wait_ms: games::CHECK_INTERVAL_MS,
            ..play
        };
    }
    session.map_live(play.map_api, false);
    snapshot.publish(session.frame_bytes());
    PlayLoop {
        wait_ms: games::map_interval_ms(fps),
        ..play
    }
}

fn bridge_if_needed(seen: SeenSim) -> SeenSim {
    let physics = std::fs::read(acr::PHYSICS_SHM_PATH).ok();
    if games::use_acr_bridge(
        seen.sim_exe,
        games::TelemetrySource::Auto,
        physics.as_deref(),
    ) {
        return games::bridged_acr(seen);
    }
    seen
}

fn tick_devices(parts: &mut PlayParts<'_>, parsed: &cli::Invocation) {
    if parts.devices.pending {
        parts.devices.pending = false;
        parts.devices.scheduler.clear();
        parts.devices.loaded = load_configured(parsed);
        for index in 0..parts.devices.loaded.len() {
            let fps = parts
                .devices
                .loaded
                .device(index)
                .map(cargopit_devices::SimDevice::fps);
            parts
                .devices
                .scheduler
                .add_device(index, fps.unwrap_or(0) as i32);
        }
        arm_tyre_check(parts.devices);
    }
    let due = parts.devices.scheduler.poll(parts.clock.monotonic_ms());
    for event in due {
        match event.kind {
            TimerKind::Device => parts.devices.loaded.tick(event.device_index),
            TimerKind::TyreDiameter => run_tyre_check(parts),
            TimerKind::Discovery | TimerKind::Mapping => {}
        }
    }
}

fn arm_tyre_check(devices: &mut DeviceLoop) {
    let path = cargopit_config::paths::diameters_path();
    devices.tyres.config_checked = false;
    devices.tyres.active = false;
    let need = tyres::TyreSimNeed {
        calculates_diameter: devices.tyres.calculates_diameter,
        supports_haptics: devices.tyres.supports_haptics,
        calculates_slip: devices.tyres.calculates_slip,
        use_config: tyres::USECONFIG_PLAY,
        has_config_path: tyres::config_path_present(&path),
    };
    if !devices.loaded.needs_tyre_diameter() || !tyres::sim_needs_tyre_diameter(&need) {
        return;
    }
    devices.scheduler.add_tyre_check();
    devices.tyres.active = true;
}

fn run_tyre_check(parts: &mut PlayParts<'_>) {
    if !parts.devices.tyres.active {
        return;
    }
    let path = cargopit_config::paths::diameters_path();
    let car = tyres::car_bytes(parts.session.frame_bytes()).to_vec();
    let Some(buf) = simapi_sys::SimDataBuf::from_bytes(parts.session.frame_bytes()) else {
        return;
    };
    let mut sim = Telemetry::from_buf(buf);
    let result = tyres::check_tyres(
        &mut sim,
        &tyres::TyreCheckRequest {
            car: &car,
            path: &path,
            config_checked: parts.devices.tyres.config_checked,
        },
    );
    parts.devices.tyres.config_checked = result.config_checked;
    if result.updated {
        publish_tyres(parts.session, parts.snapshot, &sim);
    }
    if result.stop_timer {
        parts.devices.tyres.active = false;
    }
}

fn publish_tyres(session: &mut GameSession, snapshot: &mut games::FrameSnapshot, sim: &Telemetry) {
    let mut bytes = vec![0u8; sim.byte_len()];
    if !sim.copy_into(&mut bytes) {
        return;
    }
    if session.write_frame(&bytes) {
        snapshot.publish(session.frame_bytes());
    }
}

fn load_configured(parsed: &cli::Invocation) -> LoadedDevices {
    let path = games::config_path_for(
        parsed.config_file.as_deref().map(std::path::Path::new),
        parsed.config_dir.as_deref().map(std::path::Path::new),
    );
    let path_text = path.display().to_string();
    slog(
        parsed,
        Level::Info,
        &games::loading_profile_message(&path_text, parsed.config_index),
    );
    let Ok(config) = cargopit_config::config::load_file(&path) else {
        return LoadedDevices::empty();
    };
    let count = i32::try_from(config.profiles.len()).unwrap_or(i32::MAX);
    if let Some(fault) = games::profile_load_fault(count, parsed.config_index) {
        log_profile_fault(parsed, &path_text, parsed.config_index, fault);
        return LoadedDevices::empty();
    }
    LoadedDevices::from_config(&config, parsed.config_index, parsed.disable_audio)
}

fn log_profile_fault(
    parsed: &cli::Invocation,
    path: &str,
    config_index: i32,
    fault: games::ProfileLoadFault,
) {
    match fault {
        games::ProfileLoadFault::NoProfiles => {
            slog(parsed, Level::Error, &games::no_profiles_message(path));
        }
        games::ProfileLoadFault::IndexOutOfRange { configs } => {
            slog(
                parsed,
                Level::Error,
                &games::config_index_range_message(config_index, configs),
            );
        }
    }
    slog(
        parsed,
        Level::Error,
        &games::no_profile_message(config_index),
    );
}

fn recv_udp(
    session: &mut GameSession,
    snapshot: &mut games::FrameSnapshot,
    socket: &Option<UdpSocket>,
    clock: &impl Clock,
    map_api: i32,
) {
    let Some(socket) = socket else {
        return;
    };
    let Some(mut packet) = udp::recv_packet(socket) else {
        return;
    };
    if udp::ingest(session, &mut packet, map_api, clock) {
        snapshot.publish(session.frame_bytes());
    }
}

fn observe(session: &mut GameSession, force_udp: bool, direct: bool) -> Observed {
    let info = session.detect_with(force_udp, direct, Some(udp::setup_udp));
    Observed {
        seen: SeenSim {
            is_sim_on: info.isSimOn,
            sim_status: session_status(session),
            map_api: info.mapapi as i32,
            uses_udp: info.SimUsesUDP,
            sim_exe: info.simulatorexe as u64,
        },
        flags: TyreSimFlags {
            calculates_diameter: info.SimCalculatesTyreDiameter,
            supports_haptics: info.SimSupportsHapticEffects,
            calculates_slip: info.SimCalculatesSlipRatio,
        },
    }
}

fn session_status(session: &mut GameSession) -> i32 {
    session.data_mut().simstatus as i32
}

fn test_mode(parsed: &cli::Invocation) -> ExitCode {
    if !prepare_host(parsed) {
        return ExitCode::SUCCESS;
    }
    slog(parsed, Level::Info, games::MSG_TEST_MODE_BANNER);
    let code = run_test(parsed);
    log_action_exit(
        parsed,
        code,
        games::test_exit_message,
        games::test_fail_message,
    );
    let _ = io::stdout().flush();
    ExitCode::SUCCESS
}

fn run_test(parsed: &cli::Invocation) -> i32 {
    let path = games::config_path_for(
        parsed.config_file.as_deref().map(std::path::Path::new),
        parsed.config_dir.as_deref().map(std::path::Path::new),
    );
    let config = cargopit_config::config::load_file(&path).ok();
    match testmode::plan(
        config.as_ref(),
        parsed.config_index,
        parsed.device_index,
        parsed.disable_audio,
    ) {
        testmode::Plan::MissingIndex => {
            slog(parsed, Level::Error, testmode::MSG_NO_DEVICES);
            games::ERROR_INVALID_DEV
        }
        testmode::Plan::Empty => {
            slog(parsed, Level::Error, games::MSG_TEST_INDEX);
            games::ERROR_INVALID_DEV
        }
        testmode::Plan::Ready { subjects, .. } if subjects.is_empty() => {
            slog(parsed, Level::Error, testmode::MSG_NO_DEVICES);
            games::ERROR_INVALID_DEV
        }
        testmode::Plan::Ready {
            subjects,
            device_index,
        } => {
            print_test_script(parsed, &subjects, device_index);
            games::ERROR_NONE
        }
    }
}

fn prepare_host(parsed: &cli::Invocation) -> bool {
    eprintln!("{}", games::MSG_APPLYING_SETTINGS);
    eprintln!("{}", games::MSG_SETTINGS_APPLIED);
    slog(parsed, Level::Info, games::MSG_CHECKING_DIAMETERS);
    let path = games::config_path_for(
        parsed.config_file.as_deref().map(std::path::Path::new),
        parsed.config_dir.as_deref().map(std::path::Path::new),
    );
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let path_text = path.display().to_string();
    slog(
        parsed,
        Level::Info,
        &games::testing_config_message(&path_text),
    );
    let diameters = games::home_config_file(cargopit_config::keys::DIAMETERS_FILE_NAME);
    slog(
        parsed,
        Level::Debug,
        &games::diameters_debug_message(
            &diameters.display().to_string(),
            games::CONFIG_CHECK_START,
        ),
    );
    match games::inspect_config(&path) {
        Ok(()) => {
            slog(parsed, Level::Info, games::MSG_OPENED_CONFIG);
            true
        }
        Err(issue) => {
            let message = games::config_issue_message(&issue.file, issue.line, &issue.text);
            slog(parsed, Level::Error, &message);
            eprintln!("{message}");
            false
        }
    }
}

fn log_action_exit(
    parsed: &cli::Invocation,
    code: i32,
    success: fn(i32) -> String,
    failure: fn(i32) -> String,
) {
    if code == games::ERROR_NONE {
        slog(parsed, Level::Info, &success(code));
        return;
    }
    slog(parsed, Level::Error, &failure(code));
}

fn print_test_script(
    parsed: &cli::Invocation,
    subjects: &[testmode::TestSubject],
    device_index: Option<u32>,
) {
    let mut session = GameSession::new();
    let open_error = session.try_open_publish_map();
    let publish = session.map_open();
    if !publish {
        slog(parsed, Level::Warn, &testmode::map_open_warning(open_error));
    }
    let options = testmode::RunOptions {
        subjects,
        device_index,
        trace: false,
        publish,
    };
    let script = testmode::run_frames(&options, &mut |_| false, &mut |bytes| {
        let _ = session.publish_bytes(bytes);
    });
    for line in script.lines {
        slog(parsed, Level::Info, &line);
    }
}

fn slog(parsed: &cli::Invocation, level: Level, message: &str) {
    let (dir, stem) = log::destination(parsed.log_file.as_deref());
    let stamp = SystemClock::new().wall_stamp();
    let Some(line) = log::emit(parsed.verbosity, level, message, &stamp) else {
        return;
    };
    println!("{line}");
    let _ = log::append(&dir, &stem, &line, &stamp);
}

fn config_tach(parsed: &cli::Invocation) -> ExitCode {
    if !prepare_host(parsed) {
        return ExitCode::SUCCESS;
    }
    let Some(targets) = tach::rpm_targets(parsed.max_revs, parsed.granularity) else {
        eprintln!("{}", tach::min_revs_message());
        return ExitCode::SUCCESS;
    };
    let Some(path) = &parsed.save_file else {
        print!("{}", cli::usage_text());
        return ExitCode::SUCCESS;
    };
    let interactive = io::IsTerminal::is_terminal(&io::stdin());
    let stdin = io::stdin();
    let reader = io::BufReader::new(stdin.lock());
    let mut keys = reader.bytes().map_while(Result::ok);
    let Ok(pulses) = tach::capture(&targets, &mut keys, |event| {
        host_tach_event(event, interactive);
    }) else {
        return ExitCode::SUCCESS;
    };
    let nodes: Vec<tach::TachNode> = targets
        .iter()
        .zip(pulses)
        .map(|(rpm, pulses)| tach::TachNode { rpm: *rpm, pulses })
        .collect();
    if std::fs::write(path, tach::write_xml(parsed.max_revs, &nodes)).is_err() {
        eprintln!("{}", tach::MSG_WRITE_FAILED);
        return ExitCode::SUCCESS;
    }
    settle_tach(interactive);
    let _ = io::stdout().flush();
    ExitCode::SUCCESS
}

fn host_tach_event(event: tach::WizardEvent, interactive: bool) {
    match event {
        tach::WizardEvent::Line(text) => println!("{text}"),
        tach::WizardEvent::Settle => settle_tach(interactive),
        tach::WizardEvent::Show(_) => {}
    }
}

fn settle_tach(interactive: bool) {
    if !interactive {
        return;
    }
    thread::sleep(Duration::from_secs(tach::SETTLE_SECS));
}
