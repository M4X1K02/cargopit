//! Rust host. `play` discovers a live sim after simd is running.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use cargopit::acr;
use cargopit::cli::{self, ProgramAction};
use cargopit::control::{self, ControlEffect, SessionStatus};
use cargopit::devices::LoadedDevices;
use cargopit::games::{self, PlayAction, PlayPhase, SeenSim};
use cargopit::scheduler::{self, TimerKind};
use cargopit::simd::{self, EnsureStatus};
use cargopit::tach;
use cargopit::testmode;
use cargopit::udp;
use cargopit_devices::clock::{Clock, SystemClock};
use simapi_sys::GameSession;
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
    let _ = parsed.disable_audio;
    match simd::ensure() {
        EnsureStatus::Ok => run_discovery(parsed),
        EnsureStatus::NotInstalled | EnsureStatus::StartFailed => ExitCode::SUCCESS,
    }
}

struct PlayLoop {
    phase: PlayPhase,
    use_udp: bool,
    map_api: i32,
    wait_ms: u64,
}

struct DeviceLoop {
    loaded: LoadedDevices,
    scheduler: scheduler::Scheduler,
    pending: bool,
}

fn run_discovery(parsed: &cli::Invocation) -> ExitCode {
    let mut session = GameSession::new();
    let mut snapshot = games::FrameSnapshot::new();
    let mut socket: Option<UdpSocket> = None;
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
    };
    let mut play = PlayLoop {
        phase: PlayPhase::Searching,
        use_udp: parsed.force_udp,
        map_api: games::MAP_API_SIMD,
        wait_ms: games::CHECK_INTERVAL_MS,
    };
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
    let mut seen = observe(parts.session, force_udp, false);
    if play.phase == PlayPhase::Searching && games::simd_map_is_stale(seen.map_api, false) {
        let advancing = parts
            .session
            .daemon_advancing(Duration::from_micros(games::DAEMON_PROBE_US));
        if games::simd_map_is_stale(seen.map_api, advancing) {
            seen = observe(parts.session, force_udp, true);
        }
    }
    let seen = bridge_if_needed(seen);
    let play = apply_control(parts.control, parts.devices, play, parsed);
    if play.phase == PlayPhase::Exiting {
        return play;
    }
    match play.phase {
        PlayPhase::Searching => match games::search_tick(seen, force_udp) {
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
            if next.phase == PlayPhase::Mapping {
                tick_devices(parts.devices, parts.clock, parsed);
            }
            next
        }
        PlayPhase::Exiting => play,
    }
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
        Some(ControlEffect::Stop) => PlayLoop {
            phase: PlayPhase::Exiting,
            ..play
        },
        Some(ControlEffect::Reload) => {
            devices.pending = true;
            searching(play)
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
        releasing: false,
        paused: false,
        config_index: parsed.config_index,
        sim: control::SIM_NONE.to_string(),
        devices: devices.loaded.len() as u64,
        updates,
        overruns: 0,
    }
}

fn searching(play: PlayLoop) -> PlayLoop {
    PlayLoop {
        phase: PlayPhase::Searching,
        wait_ms: games::CHECK_INTERVAL_MS,
        ..play
    }
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

fn tick_devices(devices: &mut DeviceLoop, clock: &impl Clock, parsed: &cli::Invocation) {
    if devices.pending {
        devices.pending = false;
        devices.loaded = load_configured(parsed);
        for index in 0..devices.loaded.len() {
            let fps = devices
                .loaded
                .device(index)
                .map(cargopit_devices::SimDevice::fps);
            devices.scheduler.add_device(index, fps.unwrap_or(0) as i32);
        }
    }
    let due = devices.scheduler.poll(clock.monotonic_ms());
    for event in due {
        if event.kind == TimerKind::Device {
            devices.loaded.tick(event.device_index);
        }
    }
}

fn load_configured(parsed: &cli::Invocation) -> LoadedDevices {
    let path = parsed
        .config_file
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(cargopit_config::paths::default_config_path);
    let Ok(config) = cargopit_config::config::load_file(&path) else {
        return LoadedDevices::empty();
    };
    LoadedDevices::from_config(&config, parsed.config_index, parsed.disable_audio)
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

fn observe(session: &mut GameSession, force_udp: bool, direct: bool) -> SeenSim {
    let info = session.detect_with(force_udp, direct, Some(udp::setup_udp));
    SeenSim {
        is_sim_on: info.isSimOn,
        sim_status: session_status(session),
        map_api: info.mapapi as i32,
        uses_udp: info.SimUsesUDP,
        sim_exe: info.simulatorexe as u64,
    }
}

fn session_status(session: &mut GameSession) -> i32 {
    session.data_mut().simstatus as i32
}

fn test_mode(parsed: &cli::Invocation) -> ExitCode {
    let count = device_count(parsed);
    println!("{}", testmode::preparing(count));
    let _ = io::stdout().flush();
    ExitCode::SUCCESS
}

fn device_count(parsed: &cli::Invocation) -> usize {
    let Some(path) = &parsed.config_file else {
        return 0;
    };
    let Ok(config) = cargopit_config::config::load_file(std::path::Path::new(path)) else {
        return 0;
    };
    let index = if parsed.config_index < 0 {
        0usize
    } else {
        parsed.config_index as usize
    };
    config
        .profiles
        .get(index)
        .map(|profile| profile.devices.len())
        .unwrap_or(0)
}

fn config_tach(parsed: &cli::Invocation) -> ExitCode {
    if tach::rpm_targets(parsed.max_revs, parsed.granularity).is_none() {
        eprintln!("{}", tach::min_revs_message());
        return ExitCode::SUCCESS;
    }
    let Some(path) = &parsed.save_file else {
        print!("{}", cli::usage_text());
        return ExitCode::SUCCESS;
    };
    let targets = tach::rpm_targets(parsed.max_revs, parsed.granularity).unwrap_or_default();
    let nodes: Vec<tach::TachNode> = targets
        .iter()
        .map(|rpm| tach::TachNode {
            rpm: *rpm,
            pulses: 0,
        })
        .collect();
    let xml = tach::write_xml(parsed.max_revs, &nodes);
    if std::fs::write(path, xml).is_err() {
        eprintln!("could not write tachometer file");
    }
    ExitCode::SUCCESS
}
