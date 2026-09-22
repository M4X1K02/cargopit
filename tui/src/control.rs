//! Client for the control socket of a running `cargopit play`
//! (protocol in src/cargopit/gameloop/control.h): one command line per
//! connection, one JSON line back.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::consts;

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct PlayStatus {
    pub ok: bool,
    pub state: String,
    pub releasing: bool,
    pub paused: bool,
    pub config_index: i64,
    pub sim: String,
    pub devices: u32,
    pub updates: u64,
    pub overruns: u64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Reply {
    ok: bool,
}

pub fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var(consts::ENV_XDG_RUNTIME_DIR).ok();
    let uid = std::fs::metadata(consts::PROC_SELF)
        .map(|meta| meta.uid())
        .unwrap_or_default();
    socket_path_for(runtime_dir.as_deref(), uid)
}

fn socket_path_for(runtime_dir: Option<&str>, uid: u32) -> PathBuf {
    match runtime_dir.filter(|dir| !dir.is_empty()) {
        Some(dir) => Path::new(dir).join(consts::CONTROL_SOCKET_NAME),
        None => PathBuf::from(format!(
            "{}{uid}{}",
            consts::CONTROL_SOCKET_FALLBACK_PREFIX,
            consts::CONTROL_SOCKET_FALLBACK_SUFFIX
        )),
    }
}

fn request_at(path: &Path, command: &str) -> std::io::Result<String> {
    let timeout = Some(Duration::from_millis(consts::CONTROL_TIMEOUT_MS));
    let mut stream = UnixStream::connect(path)?;
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    stream.write_all(format!("{command}\n").as_bytes())?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    Ok(line)
}

fn command_ok_at(path: &Path, command: &str) -> bool {
    request_at(path, command)
        .ok()
        .and_then(|line| serde_json::from_str::<Reply>(&line).ok())
        .is_some_and(|reply| reply.ok)
}

fn status_at(path: &Path) -> Option<PlayStatus> {
    let line = request_at(path, consts::CONTROL_CMD_STATUS).ok()?;
    serde_json::from_str::<PlayStatus>(&line)
        .ok()
        .filter(|status| status.ok)
}

/// Live session state, or `None` when no play session is listening.
pub fn status() -> Option<PlayStatus> {
    status_at(&socket_path())
}

/// Ask the running session to reload its device profile from disk.
pub fn reload() -> bool {
    command_ok_at(&socket_path(), consts::CONTROL_CMD_RELOAD)
}

/// Ask the running session to release its devices and exit.
pub fn stop() -> bool {
    command_ok_at(&socket_path(), consts::CONTROL_CMD_STOP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;

    const CONTROL_H: &str = include_str!("../../src/cargopit/gameloop/control.h");

    fn c_define(name: &str) -> String {
        CONTROL_H
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("#define ")?
                    .strip_prefix(name)?
                    .trim()
                    .strip_prefix('"')?
                    .strip_suffix('"')
                    .map(str::to_string)
            })
            .unwrap_or_else(|| panic!("{name} missing from control.h"))
    }

    #[test]
    fn protocol_matches_c_daemon() {
        assert_eq!(c_define("CONTROL_SOCKET_NAME"), consts::CONTROL_SOCKET_NAME);
        assert_eq!(c_define("CONTROL_CMD_STATUS"), consts::CONTROL_CMD_STATUS);
        assert_eq!(c_define("CONTROL_CMD_RELOAD"), consts::CONTROL_CMD_RELOAD);
        assert_eq!(c_define("CONTROL_CMD_STOP"), consts::CONTROL_CMD_STOP);
        assert_eq!(
            c_define("CONTROL_SOCKET_FALLBACK"),
            format!(
                "{}%u{}",
                consts::CONTROL_SOCKET_FALLBACK_PREFIX,
                consts::CONTROL_SOCKET_FALLBACK_SUFFIX
            )
        );
    }

    #[test]
    fn socket_path_prefers_runtime_dir() {
        assert_eq!(
            socket_path_for(Some("/run/user/1000"), 1000),
            PathBuf::from("/run/user/1000/cargopit.sock")
        );
        assert_eq!(
            socket_path_for(None, 1000),
            PathBuf::from("/tmp/cargopit-1000.sock")
        );
        assert_eq!(
            socket_path_for(Some(""), 7),
            PathBuf::from("/tmp/cargopit-7.sock")
        );
    }

    fn serve_once(reply: &'static str) -> (tempfile::TempDir, PathBuf, thread::JoinHandle<String>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(consts::CONTROL_SOCKET_NAME);
        let listener = UnixListener::bind(&path).unwrap();
        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            (&stream).write_all(reply.as_bytes()).unwrap();
            line
        });
        (dir, path, handle)
    }

    #[test]
    fn status_parses_daemon_reply() {
        let (_dir, path, server) = serve_once(
            "{\"ok\":true,\"state\":\"mapping\",\"releasing\":false,\"paused\":false,\"config_index\":1,\"sim\":\"Assetto Corsa\",\"devices\":2,\"updates\":120,\"overruns\":3}\n",
        );
        let status = status_at(&path).expect("status");
        assert_eq!(server.join().unwrap(), "status\n");
        assert_eq!(status.state, "mapping");
        assert_eq!(status.config_index, 1);
        assert_eq!(status.devices, 2);
        assert_eq!(status.overruns, 3);
    }

    #[test]
    fn stop_reports_daemon_ack() {
        let (_dir, path, server) = serve_once("{\"ok\":true}\n");
        assert!(command_ok_at(&path, consts::CONTROL_CMD_STOP));
        assert_eq!(server.join().unwrap(), "stop\n");
    }

    #[test]
    fn missing_socket_means_no_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(consts::CONTROL_SOCKET_NAME);
        assert!(status_at(&path).is_none());
        assert!(!command_ok_at(&path, consts::CONTROL_CMD_RELOAD));
    }
}
