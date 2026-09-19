use crate::consts::*;
use crate::paths;
use anyhow::{Context, Result};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct ProcessStatus {
    pub simd_running: bool,
    pub monocoque_running: bool,
    pub simapi_present: bool,
    pub notes: Vec<String>,
}

pub struct ManagedChild {
    pub label: &'static str,
    child: Child,
}

pub fn refresh_status() -> ProcessStatus {
    let mut status = ProcessStatus {
        simd_running: process_running(SIMD_PROCESS_NAME),
        monocoque_running: process_running(MONOCOQUE_PROCESS_NAME),
        simapi_present: Path::new(SIMAPI_SHM_PATH).exists(),
        notes: Vec::new(),
    };
    if let Some(note) =
        cleanup_stale_pid_file(SIMD_PID_FILE, status.simd_running, SIMD_PROCESS_NAME)
    {
        status.notes.push(note);
    }
    if let Some(note) = cleanup_stale_pid_file(
        MONOCOQUE_PID_FILE,
        status.monocoque_running,
        MONOCOQUE_PROCESS_NAME,
    ) {
        status.notes.push(note);
    }
    status
}

fn process_running(name: &str) -> bool {
    if pgrep_exact(name) {
        return true;
    }
    scan_ps_for(name)
}

fn pgrep_exact(name: &str) -> bool {
    Command::new("pgrep")
        .args(["-x", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn scan_ps_for(name: &str) -> bool {
    let output = Command::new("ps").args(["axo", "pid,comm,args"]).output();
    let Ok(output) = output else {
        return false;
    };
    let self_pid = std::process::id().to_string();
    for line in String::from_utf8_lossy(&output.stdout).lines().skip(1) {
        let mut parts = line.split_whitespace();
        let Some(pid) = parts.next() else {
            continue;
        };
        if pid == self_pid {
            continue;
        }
        let Some(comm) = parts.next() else {
            continue;
        };
        let args = parts.collect::<Vec<_>>().join(" ");
        if comm.contains(BINARY_NAME) || args.contains(BINARY_NAME) {
            continue;
        }
        if comm == name || comm.ends_with(name) || args.contains(&format!("/{name}")) {
            return true;
        }
    }
    false
}

fn cleanup_stale_pid_file(path: &str, running: bool, service: &str) -> Option<String> {
    if running || !Path::new(path).exists() {
        return None;
    }
    let pid_text = fs::read_to_string(path).ok()?;
    let pid_text = pid_text.trim();
    if let Ok(pid) = pid_text.parse::<i32>() {
        if pid_is_service(pid, service) {
            return None;
        }
    }
    let _ = fs::remove_file(path);
    Some(format!(
        "Cleaned stale PID file {}",
        Path::new(path).file_name()?.to_string_lossy()
    ))
}

fn pid_is_service(pid: i32, service: &str) -> bool {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout).contains(service)
}

pub fn find_monocoque() -> Option<PathBuf> {
    if let Ok(path) = which(MONOCOQUE_PROCESS_NAME) {
        return Some(path);
    }
    let candidates = [
        paths::data_dir().join("monocoque/build/monocoque"),
        paths::local_bin_dir().join(MONOCOQUE_PROCESS_NAME),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

fn which(name: &str) -> Result<PathBuf> {
    let output = Command::new("which").arg(name).output()?;
    if !output.status.success() {
        anyhow::bail!("{name} not on PATH");
    }
    let path = String::from_utf8(output.stdout)?.trim().to_string();
    Ok(PathBuf::from(path))
}

pub fn spawn_play(log_tx: Sender<String>) -> Result<ManagedChild> {
    spawn_monocoque(
        &[MONOCOQUE_PLAY_ARG],
        START_MONOCOQUE_WRAPPER,
        "play",
        log_tx,
    )
}

pub fn spawn_test(log_tx: Sender<String>) -> Result<ManagedChild> {
    spawn_monocoque(
        &[MONOCOQUE_TEST_ARG],
        TEST_MONOCOQUE_WRAPPER,
        "test",
        log_tx,
    )
}

fn spawn_monocoque(
    args: &[&str],
    wrapper: &str,
    label: &'static str,
    log_tx: Sender<String>,
) -> Result<ManagedChild> {
    let mut command = if let Some(binary) = find_monocoque() {
        let mut command = Command::new(binary);
        command.args(args);
        command
    } else {
        Command::new(paths::local_bin_dir().join(wrapper))
    };
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start monocoque {label}"))?;
    attach_pipe(child.stdout.take(), log_tx.clone(), label);
    attach_pipe(child.stderr.take(), log_tx, label);
    Ok(ManagedChild { label, child })
}

fn attach_pipe<R>(pipe: Option<R>, tx: Sender<String>, label: &str)
where
    R: std::io::Read + Send + 'static,
{
    let Some(pipe) = pipe else {
        return;
    };
    let prefix = format!("[{label}] ");
    thread::spawn(move || {
        let reader = BufReader::new(pipe);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(format!("{prefix}{line}")).is_err() {
                break;
            }
        }
    });
}

impl ManagedChild {
    pub fn try_reap(&mut self) -> Option<String> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(format!("{} exited: {status}", self.label)),
            _ => None,
        }
    }
}

pub fn stop_all_services() -> Vec<String> {
    let mut stopped = Vec::new();
    for service in [SIMD_PROCESS_NAME, MONOCOQUE_PROCESS_NAME] {
        if Command::new("pkill")
            .args(["-x", service])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
        {
            stopped.push(format!("{service} (exact)"));
        }
    }
    kill_matching_from_ps(&mut stopped);
    thread::sleep(Duration::from_millis(PROCESS_STOP_WAIT_MS));
    for service in [SIMD_PROCESS_NAME, MONOCOQUE_PROCESS_NAME] {
        let _ = Command::new("pkill").args(["-9", "-x", service]).status();
    }
    stopped
}

fn kill_matching_from_ps(stopped: &mut Vec<String>) {
    let output = Command::new("ps").args(["axo", "pid,comm,args"]).output();
    let Ok(output) = output else {
        return;
    };
    let self_pid = std::process::id().to_string();
    for line in String::from_utf8_lossy(&output.stdout).lines().skip(1) {
        let mut parts = line.split_whitespace();
        let Some(pid) = parts.next() else {
            continue;
        };
        if pid == self_pid {
            continue;
        }
        let Some(comm) = parts.next() else {
            continue;
        };
        let args = parts.collect::<Vec<_>>().join(" ");
        if comm.contains(BINARY_NAME) || args.contains(BINARY_NAME) {
            continue;
        }
        let is_simd = comm == SIMD_PROCESS_NAME
            || comm.ends_with(SIMD_PROCESS_NAME)
            || args.contains(&format!("/{SIMD_PROCESS_NAME}"));
        let is_monocoque = (comm == MONOCOQUE_PROCESS_NAME
            || comm.ends_with(MONOCOQUE_PROCESS_NAME))
            && !args.contains(BINARY_NAME);
        if is_simd || is_monocoque {
            let _ = Command::new("kill").args(["-TERM", pid]).status();
            let name = if is_simd {
                SIMD_PROCESS_NAME
            } else {
                MONOCOQUE_PROCESS_NAME
            };
            stopped.push(format!("{name} (PID {pid})"));
        }
    }
}

pub fn log_channel() -> (Sender<String>, Receiver<String>) {
    mpsc::channel()
}
