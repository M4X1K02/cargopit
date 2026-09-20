use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::consts;
use crate::paths;
use crate::tui_state::PlayFlags;

#[derive(Debug, Clone, Default)]
pub struct ProcessStatus {
    pub simd_running: bool,
    pub cargopit_running: bool,
    pub cleaned_pid_files: Vec<String>,
}

pub struct ChildSession {
    pub child: Child,
    pub kind: SessionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Play,
    Test,
}

impl SessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionKind::Play => "play",
            SessionKind::Test => "test",
        }
    }
}

pub fn find_binary(name: &str) -> Option<PathBuf> {
    which(name)
        .into_iter()
        .chain([
            paths::source_root().join(consts::BUILD_DIRNAME).join(name),
            paths::local_bin_dir().join(name),
            paths::data_home()
                .join(consts::CONFIG_DIR_NAME)
                .join(consts::BUILD_DIRNAME)
                .join(name),
        ])
        .find(|path| is_executable(path))
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os(consts::ENV_PATH)?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    path.is_file()
        && fs::metadata(path)
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

pub fn check_processes() -> ProcessStatus {
    let mut status = ProcessStatus::default();
    let self_pid = std::process::id();
    status.simd_running = process_running(consts::BINARY_SIMD, self_pid);
    status.cargopit_running = process_running(consts::BINARY_CARGOPIT, self_pid);
    status.cleaned_pid_files = cleanup_stale_pid_files(&status);
    status
}

fn process_running(name: &str, self_pid: u32) -> bool {
    if pgrep_exact(name) {
        return true;
    }
    scan_ps(name, self_pid)
}

fn pgrep_exact(name: &str) -> bool {
    Command::new(consts::PGREP_BIN)
        .args([consts::PGREP_EXACT, name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn scan_ps(name: &str, self_pid: u32) -> bool {
    let output = Command::new(consts::PS_BIN)
        .args(consts::PS_LIST_ARGS)
        .output();
    let Ok(output) = output else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines().skip(1) {
        let mut parts = line.split_whitespace();
        let Some(pid) = parts.next() else {
            continue;
        };
        if pid.parse::<u32>().ok() == Some(self_pid) {
            continue;
        }
        let Some(comm) = parts.next() else {
            continue;
        };
        let args = parts.collect::<Vec<_>>().join(" ");
        if is_service_process(name, comm, &args) {
            return true;
        }
    }
    false
}

fn is_service_process(name: &str, comm: &str, args: &str) -> bool {
    if comm.contains(consts::BINARY_TUI) || args.contains(consts::BINARY_TUI) {
        return false;
    }
    if comm.contains("manager") || args.contains("manager") {
        return false;
    }
    comm == name || comm.ends_with(name) || args.contains(&format!("/{name}"))
}

fn cleanup_stale_pid_files(status: &ProcessStatus) -> Vec<String> {
    let files = [
        (consts::PID_FILE_SIMD, status.simd_running),
        (consts::PID_FILE_CARGOPIT, status.cargopit_running),
    ];
    let mut cleaned = Vec::new();
    for (path, running) in files {
        if try_remove_stale(path, running) {
            if let Some(name) = Path::new(path).file_name() {
                cleaned.push(name.to_string_lossy().into_owned());
            }
        }
    }
    cleaned
}

fn try_remove_stale(path: &str, running: bool) -> bool {
    if running || !Path::new(path).exists() {
        return false;
    }
    let Ok(contents) = fs::read_to_string(path) else {
        return fs::remove_file(path).is_ok();
    };
    let pid = contents.trim();
    if pid.parse::<u32>().is_err() {
        return fs::remove_file(path).is_ok();
    }
    if pid_matches_service(pid, path) {
        return false;
    }
    fs::remove_file(path).is_ok()
}

fn pid_matches_service(pid: &str, path: &str) -> bool {
    let output = Command::new(consts::PS_BIN)
        .args([consts::PS_PID_FLAG, pid])
        .args(consts::PS_COMM_ARGS)
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let comm = String::from_utf8_lossy(&output.stdout);
    let service = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    comm.contains(service)
}

pub fn spawn_args(kind: SessionKind, flags: &PlayFlags, config_path: &Path) -> Vec<String> {
    let mut args = vec![match kind {
        SessionKind::Play => consts::WHICH_CARGOPIT_PLAY.to_string(),
        SessionKind::Test => consts::WHICH_CARGOPIT_TEST.to_string(),
    }];
    match flags.verbosity {
        1 => args.push(consts::CLI_FLAG_VERBOSE.to_string()),
        n if n >= 2 => args.push(consts::CLI_FLAG_VERY_VERBOSE.to_string()),
        _ => {}
    }
    if flags.disable_audio {
        args.push(consts::CLI_FLAG_DISABLE_AUDIO.to_string());
    }
    if kind == SessionKind::Play {
        push_play_only_flags(&mut args, flags);
    }
    args.push(consts::CLI_FLAG_CONFIG_FILE.to_string());
    args.push(config_path.to_string_lossy().into_owned());
    args
}

fn push_play_only_flags(args: &mut Vec<String>, flags: &PlayFlags) {
    if flags.udp {
        args.push(consts::CLI_FLAG_UDP.to_string());
    }
    if let Some(fps) = flags.fps {
        args.push(consts::CLI_FLAG_FPS.to_string());
        args.push(fps.to_string());
    }
    if let Some(log) = &flags.log_file {
        args.push(consts::CLI_FLAG_LOG.to_string());
        args.push(log.clone());
    }
}

pub fn spawn_session(
    kind: SessionKind,
    flags: &PlayFlags,
    config_path: &Path,
) -> Result<ChildSession> {
    let bin = find_binary(consts::BINARY_CARGOPIT)
        .ok_or_else(|| anyhow!("{} binary not found", consts::BINARY_CARGOPIT))?;
    let args = spawn_args(kind, flags, config_path);
    let mut cmd = Command::new(bin);
    detach_from_tui(&mut cmd);
    let child = cmd
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn cargopit")?;
    Ok(ChildSession { child, kind })
}

fn detach_from_tui(cmd: &mut Command) {
    cmd.stdin(Stdio::null());
    cmd.process_group(0);
}

pub fn kill_session(session: &mut ChildSession) {
    let _ = session.child.kill();
}

pub fn stop_all() -> Vec<String> {
    stop_named(&[consts::BINARY_SIMD, consts::BINARY_CARGOPIT])
}

pub fn stop_play() -> Vec<String> {
    stop_named(&[consts::BINARY_CARGOPIT])
}

fn stop_named(names: &[&str]) -> Vec<String> {
    let mut stopped = Vec::new();
    let self_pid = std::process::id();
    for name in names {
        if pkill_exact(name, false) {
            stopped.push(format!("{name} (exact)"));
        }
        stopped.extend(kill_from_ps(name, self_pid));
    }
    thread::sleep(Duration::from_millis(consts::PROCESS_STOP_WAIT_MS));
    for name in names {
        pkill_exact(name, true);
    }
    stopped
}

fn pkill_exact(name: &str, force: bool) -> bool {
    let mut cmd = Command::new(consts::PKILL_BIN);
    if force {
        cmd.arg(consts::PKILL_SIGNAL_KILL);
    }
    cmd.args([consts::PGREP_EXACT, name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn kill_from_ps(name: &str, self_pid: u32) -> Vec<String> {
    let output = Command::new(consts::PS_BIN)
        .args(consts::PS_LIST_ARGS)
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut stopped = Vec::new();
    for line in text.lines().skip(1) {
        let mut parts = line.split_whitespace();
        let Some(pid) = parts.next() else {
            continue;
        };
        if pid.parse::<u32>().ok() == Some(self_pid) {
            continue;
        }
        let Some(comm) = parts.next() else {
            continue;
        };
        let args = parts.collect::<Vec<_>>().join(" ");
        if !is_service_process(name, comm, &args) {
            continue;
        }
        let _ = Command::new(consts::KILL_BIN)
            .args([consts::KILL_TERM, pid])
            .status();
        stopped.push(format!("{name} (PID {pid})"));
        thread::sleep(Duration::from_millis(consts::PROCESS_KILL_PAUSE_MS));
    }
    stopped
}

pub fn tachometer_args(max_revs: i64, granularity: i64, save_file: &str) -> Vec<String> {
    vec![
        consts::CLI_CONFIG_TACHOMETER.to_string(),
        consts::CLI_TACHOMETER.to_string(),
        consts::CLI_FLAG_MAX_REVS.to_string(),
        max_revs.to_string(),
        consts::CLI_FLAG_GRANULARITY.to_string(),
        granularity.to_string(),
        consts::CLI_FLAG_SAVEFILE.to_string(),
        save_file.to_string(),
    ]
}

pub fn spawn_tachometer(max_revs: i64, granularity: i64, save_file: &str) -> Result<String> {
    let bin = find_binary(consts::BINARY_CARGOPIT)
        .ok_or_else(|| anyhow!("{} binary not found", consts::BINARY_CARGOPIT))?;
    let args = tachometer_args(max_revs, granularity, save_file);
    let mut cmd = Command::new(bin);
    detach_from_tui(&mut cmd);
    let output = cmd
        .args(&args)
        .output()
        .context("run tachometer calibration")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        Ok(format!("{stdout}{stderr}"))
    } else {
        Err(anyhow!("tachometer calibration failed: {stdout}{stderr}"))
    }
}

pub fn simapi_present() -> bool {
    Path::new(consts::SIMAPI_DAT_PATH).exists()
}

pub fn simapi_nonzero() -> bool {
    let Ok(bytes) = fs::read(consts::SIMAPI_DAT_PATH) else {
        return false;
    };
    bytes.iter().any(|b| *b != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui_state::PlayFlags;

    #[test]
    fn spawn_play_includes_selected_flags() {
        let flags = PlayFlags {
            verbosity: 2,
            disable_audio: true,
            udp: true,
            fps: Some(30),
            log_file: Some("/tmp/cargopit.log".into()),
        };
        let args = spawn_args(SessionKind::Play, &flags, Path::new("/tmp/c.config"));
        assert_eq!(args[0], consts::WHICH_CARGOPIT_PLAY);
        assert!(args.contains(&consts::CLI_FLAG_VERY_VERBOSE.to_string()));
        assert!(args.contains(&consts::CLI_FLAG_DISABLE_AUDIO.to_string()));
        assert!(args.contains(&consts::CLI_FLAG_UDP.to_string()));
        assert!(args.contains(&consts::CLI_FLAG_FPS.to_string()));
        assert!(args.contains(&"30".to_string()));
        assert!(args.contains(&consts::CLI_FLAG_CONFIG_FILE.to_string()));
    }

    #[test]
    fn spawn_test_includes_config_file() {
        let flags = PlayFlags {
            udp: true,
            fps: Some(30),
            ..PlayFlags::default()
        };
        let args = spawn_args(SessionKind::Test, &flags, Path::new("/tmp/c.config"));
        assert_eq!(args[0], consts::WHICH_CARGOPIT_TEST);
        assert!(args.contains(&consts::CLI_FLAG_CONFIG_FILE.to_string()));
        assert!(args.contains(&"/tmp/c.config".to_string()));
        assert!(!args.contains(&consts::CLI_FLAG_UDP.to_string()));
        assert!(!args.contains(&consts::CLI_FLAG_FPS.to_string()));
    }
}
