use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::consts;
use crate::control;
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

#[derive(Debug, Clone, Copy, Default)]
pub struct TestScope {
    pub config_index: Option<usize>,
    pub device_index: Option<usize>,
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
    if name == consts::BINARY_CARGOPIT {
        if let Some(path) = env_binary(consts::ENV_CARGOPIT_BIN) {
            return Some(path);
        }
    }
    if let Some(path) = which(name) {
        if !is_managed_binary(&path, name) {
            return Some(path);
        }
    }
    managed_binaries(name)
        .into_iter()
        .chain(which(name))
        .find(|path| is_executable(path))
}

fn managed_binaries(name: &str) -> [PathBuf; 3] {
    [
        paths::source_root().join(consts::BUILD_DIRNAME).join(name),
        paths::local_bin_dir().join(name),
        paths::data_home()
            .join(consts::CONFIG_DIR_NAME)
            .join(consts::BUILD_DIRNAME)
            .join(name),
    ]
}

fn is_managed_binary(path: &Path, name: &str) -> bool {
    managed_binaries(name).iter().any(|managed| managed == path)
}

fn env_binary(key: &str) -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os(key)?);
    if is_executable(&path) {
        return Some(path);
    }
    None
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
    check_processes_from(&process_listing())
}

pub fn check_processes_from(listing: &str) -> ProcessStatus {
    let mut status = ProcessStatus::default();
    let self_pid = std::process::id();
    status.simd_running = service_running(listing, consts::BINARY_SIMD, self_pid);
    status.cargopit_running = service_running(listing, consts::BINARY_CARGOPIT, self_pid)
        || service_running(listing, consts::BINARY_CARGOPIT_LEGACY, self_pid);
    status.cleaned_pid_files = cleanup_stale_pid_files(&status);
    status
}

fn service_running(listing: &str, name: &str, self_pid: u32) -> bool {
    if listing_has_service(listing, name, self_pid) {
        return true;
    }
    if !listing.is_empty() {
        return false;
    }
    pgrep_exact(name)
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

fn listing_has_service(listing: &str, name: &str, self_pid: u32) -> bool {
    for line in listing.lines().skip(1) {
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

pub fn process_listing() -> String {
    let output = Command::new(consts::PS_BIN)
        .args(consts::PS_LIST_ARGS)
        .output();
    let Ok(output) = output else {
        return String::new();
    };
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn is_service_process(name: &str, comm: &str, args: &str) -> bool {
    if is_excluded_process(comm, args) {
        return false;
    }
    process_basename(comm) == name || args_launch_binary(args, name)
}

fn is_excluded_process(comm: &str, args: &str) -> bool {
    comm.contains(consts::BINARY_TUI)
        || args.contains(consts::BINARY_TUI)
        || comm.contains("manager")
        || args.contains("manager")
}

fn process_basename(token: &str) -> &str {
    Path::new(token)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(token)
}

fn args_launch_binary(args: &str, name: &str) -> bool {
    args.split_whitespace()
        .next()
        .is_some_and(|token| process_basename(token) == name)
}

pub fn wait_until_stopped(name: &str) {
    let self_pid = std::process::id();
    for _ in 0..consts::PROCESS_STOP_POLL_ATTEMPTS {
        if !service_running(&process_listing(), name, self_pid) {
            return;
        }
        thread::sleep(Duration::from_millis(consts::PROCESS_STOP_POLL_MS));
    }
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

pub fn session_exit_message(kind: SessionKind, status: std::process::ExitStatus) -> String {
    if status.success() {
        return match kind {
            SessionKind::Play => consts::MSG_PLAY_EXITED.to_string(),
            SessionKind::Test => consts::MSG_TEST_FINISHED.to_string(),
        };
    }
    let prefix = match kind {
        SessionKind::Play => consts::MSG_PLAY_FAILED,
        SessionKind::Test => consts::MSG_TEST_FAILED,
    };
    format!("{prefix} ({status})")
}

pub fn spawn_args(kind: SessionKind, flags: &PlayFlags, config_path: &Path) -> Vec<String> {
    spawn_args_with_scope(kind, flags, config_path, TestScope::default())
}

pub fn spawn_args_with_scope(
    kind: SessionKind,
    flags: &PlayFlags,
    config_path: &Path,
    scope: TestScope,
) -> Vec<String> {
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
    push_session_scope(&mut args, kind, scope);
    args
}

fn push_session_scope(args: &mut Vec<String>, kind: SessionKind, scope: TestScope) {
    if let Some(index) = scope.config_index {
        args.push(consts::CLI_FLAG_CONFIG_INDEX.to_string());
        args.push(index.to_string());
    }
    if kind != SessionKind::Test {
        return;
    }
    if let Some(index) = scope.device_index {
        args.push(consts::CLI_FLAG_DEVICE_INDEX.to_string());
        args.push(index.to_string());
    }
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
    spawn_session_with_scope(kind, flags, config_path, TestScope::default())
}

pub fn spawn_session_with_scope(
    kind: SessionKind,
    flags: &PlayFlags,
    config_path: &Path,
    scope: TestScope,
) -> Result<ChildSession> {
    let bin = find_binary(consts::BINARY_CARGOPIT)
        .ok_or_else(|| anyhow!("{} binary not found", consts::BINARY_CARGOPIT))?;
    let args = spawn_args_with_scope(kind, flags, config_path, scope);
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
    let _ = terminate_then_kill(&mut session.child);
}

fn terminate_then_kill(child: &mut Child) -> std::io::Result<std::process::ExitStatus> {
    if let Ok(Some(status)) = child.try_wait() {
        return Ok(status);
    }
    send_term(child.id());
    for _ in 0..consts::PROCESS_STOP_POLL_ATTEMPTS {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(consts::PROCESS_STOP_POLL_MS)),
            Err(err) => return Err(err),
        }
    }
    child.kill()?;
    child.wait()
}

fn send_term(pid: u32) {
    let pid_text = pid.to_string();
    let group = format!("{}{pid}", consts::PROCESS_GROUP_SIGNAL_PREFIX);
    send_kill_signal(&pid_text);
    send_kill_signal(&group);
}

fn send_kill_signal(target: &str) {
    let _ = Command::new(consts::KILL_BIN)
        .args([consts::KILL_TERM, consts::KILL_END_OF_OPTIONS, target])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn stop_all() -> Vec<String> {
    let mut stopped = stop_play_via_control();
    stopped.extend(stop_named(&[consts::BINARY_SIMD, consts::BINARY_CARGOPIT]));
    stopped
}

pub fn stop_play() -> Vec<String> {
    let stopped = stop_play_via_control();
    if !stopped.is_empty() {
        return stopped;
    }
    stop_named(&[consts::BINARY_CARGOPIT])
}

/// Clean shutdown through the play session's control socket; devices are released before it exits.
fn stop_play_via_control() -> Vec<String> {
    if !control::stop() {
        return Vec::new();
    }
    wait_until_stopped(consts::BINARY_CARGOPIT);
    vec![consts::MSG_STOPPED_PLAY_CONTROL.to_string()]
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
    fn spawn_play_includes_config_index() {
        let scope = TestScope {
            config_index: Some(2),
            device_index: Some(9),
        };
        let args = spawn_args_with_scope(
            SessionKind::Play,
            &PlayFlags::default(),
            Path::new("/tmp/c.config"),
            scope,
        );
        assert!(args.contains(&consts::CLI_FLAG_CONFIG_INDEX.to_string()));
        assert!(args.contains(&"2".to_string()));
        assert!(!args.contains(&consts::CLI_FLAG_DEVICE_INDEX.to_string()));
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
        assert!(!args.contains(&consts::CLI_FLAG_DEVICE_INDEX.to_string()));
    }

    #[test]
    fn spawn_device_test_includes_indexes() {
        let scope = TestScope {
            config_index: Some(1),
            device_index: Some(3),
        };
        let args = spawn_args_with_scope(
            SessionKind::Test,
            &PlayFlags::default(),
            Path::new("/tmp/c.config"),
            scope,
        );
        assert!(args.contains(&consts::CLI_FLAG_CONFIG_INDEX.to_string()));
        assert!(args.contains(&"1".to_string()));
        assert!(args.contains(&consts::CLI_FLAG_DEVICE_INDEX.to_string()));
        assert!(args.contains(&"3".to_string()));
    }

    #[test]
    fn test_success_exit_is_not_a_fault() {
        use std::os::unix::process::ExitStatusExt;
        let status = std::process::ExitStatus::from_raw(0);
        assert_eq!(
            session_exit_message(SessionKind::Test, status),
            consts::MSG_TEST_FINISHED
        );
    }

    #[test]
    fn repo_path_in_args_is_not_cargopit_play() {
        assert!(!is_service_process(
            consts::BINARY_CARGOPIT,
            "cursor",
            "/opt/cursor --workspace /home/dev/cargopit",
        ));
        assert!(!is_service_process(
            consts::BINARY_CARGOPIT,
            "cmake",
            "cmake --build /home/dev/cargopit/build",
        ));
    }

    #[test]
    fn cargopit_play_command_is_detected() {
        assert!(is_service_process(
            consts::BINARY_CARGOPIT,
            consts::BINARY_CARGOPIT,
            "/opt/build/cargopit play",
        ));
    }

    #[test]
    fn check_processes_from_listing_detects_play() {
        let listing =
            "PID COMMAND ARGS\n1 systemd /sbin/init\n42 cargopit /opt/build/cargopit play\n";
        let status = check_processes_from(listing);
        assert!(status.cargopit_running);
        assert!(!status.simd_running);
    }

    #[test]
    fn check_processes_from_listing_ignores_tui() {
        let listing = "PID COMMAND ARGS\n9 cargopit-tui /opt/build/cargopit-tui\n";
        let status = check_processes_from(listing);
        assert!(!status.cargopit_running);
    }

    #[test]
    fn terminate_then_kill_stops_sleep() {
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep");
        let status = terminate_then_kill(&mut child).expect("stop sleep");
        assert!(!status.success());
    }

    #[test]
    fn tui_binary_is_not_play() {
        assert!(!is_service_process(
            consts::BINARY_CARGOPIT,
            consts::BINARY_TUI,
            "/opt/build/cargopit-tui",
        ));
    }

    #[test]
    fn cargopit_bin_overrides_path_lookup() {
        let dir = std::env::temp_dir().join("cargopit-bin-override");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        let bin = dir.join(consts::BINARY_CARGOPIT);
        fs::write(&bin, "#!/bin/sh\nexit 0\n").expect("write");
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).expect("mode");
        let previous = std::env::var_os(consts::ENV_CARGOPIT_BIN);
        std::env::set_var(consts::ENV_CARGOPIT_BIN, &bin);
        assert_eq!(
            find_binary(consts::BINARY_CARGOPIT).as_deref(),
            Some(bin.as_path())
        );
        match previous {
            Some(value) => std::env::set_var(consts::ENV_CARGOPIT_BIN, value),
            None => std::env::remove_var(consts::ENV_CARGOPIT_BIN),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_play_counts_as_cargopit_running() {
        let listing = "PID COMMAND ARGS\n7 cargopit-legacy /opt/bin/cargopit-legacy play\n";
        assert!(check_processes_from(listing).cargopit_running);
    }

    #[test]
    fn path_stub_is_not_a_managed_install() {
        let stub = PathBuf::from("/tmp/walk/bin").join(consts::BINARY_CARGOPIT);
        assert!(!is_managed_binary(&stub, consts::BINARY_CARGOPIT));
        let source_build = paths::source_root()
            .join(consts::BUILD_DIRNAME)
            .join(consts::BINARY_CARGOPIT);
        assert!(is_managed_binary(&source_build, consts::BINARY_CARGOPIT));
    }
}
