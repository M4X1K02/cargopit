//! Rust host used by contract tests. The installed C binary stays `cargopit` until cutover.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use cargopit::cli::{self, ProgramAction};
use cargopit::simd::{self, EnsureStatus};
use cargopit::tach;
use cargopit::testmode;

const PARK_MS: u64 = 200;

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
        EnsureStatus::Ok => loop {
            thread::sleep(Duration::from_millis(PARK_MS));
        },
        EnsureStatus::NotInstalled | EnsureStatus::StartFailed => ExitCode::SUCCESS,
    }
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
