//! simd discovery and startup. Matches `ensure_simd` candidate order.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
pub const BINARY_NAME: &str = "simd";
pub const REQUIRED_MESSAGE: &str = "simd is required but is not installed";
pub const ENV_SIMD: &str = "SIMD";
pub const ENV_HOME: &str = "HOME";
pub const ENV_XDG_DATA: &str = "XDG_DATA_HOME";
pub const ENV_PATH: &str = "PATH";
pub const PID_FILE: &str = "/tmp/simd.pid";
pub const START_TIMEOUT_MS: u64 = 3000;
pub const POLL_MS: u64 = 50;
const EXEC_BITS: u32 = 0o111;
const SYSTEMD_UNIT: &str = "simd.service";
const INSTALL_HINT: &str =
    "Install simd (package simd / simd-git, or run ./install.sh) and try again.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnsureStatus {
    Ok,
    NotInstalled,
    StartFailed,
}

pub fn find_from_candidates(candidates: &[Option<PathBuf>]) -> Option<PathBuf> {
    candidates
        .iter()
        .flatten()
        .find(|path| is_executable(path))
        .cloned()
}

pub fn find_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(ENV_SIMD) {
        let path = PathBuf::from(path);
        if is_executable(&path) {
            return Some(path);
        }
    }
    if let Some(path) = find_on_path(BINARY_NAME) {
        return Some(path);
    }
    let home = std::env::var_os(ENV_HOME).map(PathBuf::from);
    let xdg = std::env::var_os(ENV_XDG_DATA).map(PathBuf::from);
    let xdg_simd = xdg
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join("cargopit/simapi/simd/build").join(BINARY_NAME));
    let home_simd = home.as_ref().map(|dir| {
        dir.join(".local/share/cargopit/simapi/simd/build")
            .join(BINARY_NAME)
    });
    let local_bin = home
        .as_ref()
        .map(|dir| dir.join(".local/bin").join(BINARY_NAME));
    find_from_candidates(&[
        xdg_simd,
        home_simd,
        local_bin,
        Some(PathBuf::from("/usr/local/bin").join(BINARY_NAME)),
        Some(PathBuf::from("/usr/bin").join(BINARY_NAME)),
    ])
}

pub fn process_running() -> bool {
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .chars()
            .all(|ch| ch.is_ascii_digit())
        {
            continue;
        }
        let comm = fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
        if comm.trim() == BINARY_NAME {
            return true;
        }
    }
    false
}

pub fn ensure() -> EnsureStatus {
    if process_running() {
        return EnsureStatus::Ok;
    }
    remove_stale_pid();
    if systemd_start() && wait_until_running() {
        return EnsureStatus::Ok;
    }
    let Some(path) = find_binary() else {
        report_not_installed();
        return EnsureStatus::NotInstalled;
    };
    if spawn_and_wait(&path).is_err() || !wait_until_running() {
        return EnsureStatus::StartFailed;
    }
    EnsureStatus::Ok
}

fn report_not_installed() {
    eprintln!("{REQUIRED_MESSAGE}.\n{INSTALL_HINT}");
}

fn remove_stale_pid() {
    if process_running() {
        return;
    }
    let _ = fs::remove_file(PID_FILE);
}

fn systemd_start() -> bool {
    let Some(systemctl) = find_on_path("systemctl") else {
        return false;
    };
    Command::new(systemctl)
        .args(["--user", "start", SYSTEMD_UNIT])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn spawn_and_wait(path: &Path) -> io::Result<()> {
    let status = Command::new(path).status().map_err(io::Error::other)?;
    if status.success() {
        return Ok(());
    }
    Err(io::Error::other("simd exited before daemonizing"))
}

fn wait_until_running() -> bool {
    let polls = START_TIMEOUT_MS / POLL_MS;
    let mut waited = 0u64;
    while waited < polls {
        if process_running() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(POLL_MS));
        waited += 1;
    }
    process_running()
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os(ENV_PATH)?;
    for dir in std::env::split_paths(&path) {
        let candidate = if dir.as_os_str().is_empty() {
            PathBuf::from(".").join(name)
        } else {
            dir.join(name)
        };
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & EXEC_BITS != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_missing_and_non_executable_candidates() {
        let dir = std::env::temp_dir().join("cargopit-simd-candidates");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        let plain = dir.join("not-exec");
        fs::write(&plain, "#!/bin/sh\nexit 0\n").expect("write");
        let exec = dir.join(BINARY_NAME);
        fs::write(&exec, "#!/bin/sh\nexit 0\n").expect("write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).expect("mode");
            fs::set_permissions(&exec, fs::Permissions::from_mode(0o755)).expect("mode");
        }
        assert!(find_from_candidates(&[]).is_none());
        assert!(find_from_candidates(&[Some(PathBuf::from("/no/such/simd-binary"))]).is_none());
        assert!(find_from_candidates(&[Some(plain.clone())]).is_none());
        assert_eq!(
            find_from_candidates(&[Some(plain), Some(exec.clone())]).as_deref(),
            Some(exec.as_path())
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
