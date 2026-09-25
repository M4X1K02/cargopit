//! Rust host. `play` discovers a live sim after simd is running.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use cargopit::acr;
use cargopit::cli::{self, ProgramAction};
use cargopit::games::{self, PlayAction, PlayPhase, SeenSim};
use cargopit::simd::{self, EnsureStatus};
use cargopit::tach;
use cargopit::testmode;
use simapi_sys::GameSession;

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
        EnsureStatus::Ok => run_discovery(parsed.force_udp),
        EnsureStatus::NotInstalled | EnsureStatus::StartFailed => ExitCode::SUCCESS,
    }
}

fn run_discovery(force_udp: bool) -> ExitCode {
    let mut session = GameSession::new();
    let mut phase = PlayPhase::Searching;
    loop {
        phase = poll_once(&mut session, phase, force_udp);
        if phase == PlayPhase::Exiting {
            return ExitCode::SUCCESS;
        }
        thread::sleep(Duration::from_millis(games::CHECK_INTERVAL_MS));
    }
}

fn poll_once(session: &mut GameSession, phase: PlayPhase, force_udp: bool) -> PlayPhase {
    let mut seen = observe(session, force_udp, false);
    if phase == PlayPhase::Searching && games::simd_map_is_stale(seen.map_api, false) {
        let advancing = session.daemon_advancing(Duration::from_micros(games::DAEMON_PROBE_US));
        if games::simd_map_is_stale(seen.map_api, advancing) {
            seen = observe(session, force_udp, true);
        }
    }
    let seen = bridge_if_needed(seen);
    match phase {
        PlayPhase::Searching => match games::search_tick(seen, force_udp) {
            PlayAction::StartMapping { .. } => PlayPhase::Mapping,
            PlayAction::Wait | PlayAction::Release => PlayPhase::Searching,
        },
        PlayPhase::Mapping => match games::mapping_tick(seen) {
            PlayAction::Release => {
                let _ = session.clear(false);
                PlayPhase::Searching
            }
            PlayAction::Wait | PlayAction::StartMapping { .. } => PlayPhase::Mapping,
        },
        PlayPhase::Exiting => PlayPhase::Exiting,
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

fn observe(session: &mut GameSession, force_udp: bool, direct: bool) -> SeenSim {
    let info = session.detect(force_udp, direct);
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
