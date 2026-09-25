//! Play-session control socket. One line in, one JSON line out.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

pub const SOCKET_NAME: &str = "cargopit.sock";
const SOCKET_MODE: u32 = 0o600;
pub const RUNTIME_DIR_ENV: &str = "XDG_RUNTIME_DIR";
pub const CMD_STATUS: &str = "status";
pub const CMD_RELOAD: &str = "reload";
pub const CMD_STOP: &str = "stop";
pub const SIM_NONE: &str = "none";
pub const REPLY_OK: &str = "{\"ok\":true}";
pub const REPLY_UNKNOWN: &str = "{\"ok\":false,\"error\":\"unknown command\"}";
pub const REPLY_TOO_LONG: &str = "{\"ok\":false,\"error\":\"command too long\"}";
const LINE_MAX: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionStatus {
    pub state: String,
    pub releasing: bool,
    pub paused: bool,
    pub config_index: i32,
    pub sim: String,
    pub devices: u64,
    pub updates: u64,
    pub overruns: u64,
}

pub fn current_uid() -> u32 {
    unsafe { getuid() }
}

unsafe extern "C" {
    fn getuid() -> u32;
}

pub fn socket_path(runtime_dir: Option<&str>, uid: u32) -> String {
    match runtime_dir {
        Some(dir) if !dir.is_empty() => format!("{dir}/{SOCKET_NAME}"),
        _ => format!("/tmp/cargopit-{uid}.sock"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlEffect {
    None,
    Reload,
    Stop,
}

pub fn effect(command: &str) -> ControlEffect {
    match command.trim_end_matches('\n') {
        CMD_RELOAD => ControlEffect::Reload,
        CMD_STOP => ControlEffect::Stop,
        _ => ControlEffect::None,
    }
}

pub fn bind_listener(path: &str) -> std::io::Result<UnixListener> {
    if Path::new(path).exists() {
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    listener.set_nonblocking(true)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE))?;
    Ok(listener)
}

pub fn serve_stream(
    stream: &mut UnixStream,
    status: &SessionStatus,
) -> std::io::Result<ControlEffect> {
    let mut buf = [0u8; LINE_MAX];
    let mut used = 0usize;
    let command = loop {
        if used >= LINE_MAX - 1 {
            break String::from_utf8_lossy(&buf[..used]).into_owned();
        }
        let mut byte = [0u8; 1];
        let read = stream.read(&mut byte)?;
        if read == 0 {
            break String::from_utf8_lossy(&buf[..used]).into_owned();
        }
        buf[used] = byte[0];
        used += 1;
        if byte[0] == b'\n' {
            break String::from_utf8_lossy(&buf[..used]).into_owned();
        }
    };
    let response = reply(&command, status);
    let effect = effect(command.trim_end_matches('\n'));
    stream.write_all(response.as_bytes())?;
    stream.write_all(b"\n")?;
    let _ = stream.shutdown(std::net::Shutdown::Both);
    Ok(effect)
}

pub fn try_accept(listener: &UnixListener, status: &SessionStatus) -> Option<ControlEffect> {
    let Ok((mut stream, _)) = listener.accept() else {
        return None;
    };
    let _ = stream.set_nonblocking(false);
    serve_stream(&mut stream, status).ok()
}

pub fn reply(command: &str, status: &SessionStatus) -> String {
    let command = command.trim_end_matches('\n');
    if command.len() >= LINE_MAX {
        return REPLY_TOO_LONG.to_string();
    }
    match command {
        CMD_STATUS => status_json(status),
        CMD_RELOAD | CMD_STOP => REPLY_OK.to_string(),
        _ => REPLY_UNKNOWN.to_string(),
    }
}

fn status_json(status: &SessionStatus) -> String {
    let sim = json_safe(&status.sim);
    let releasing = json_bool(status.releasing);
    let paused = json_bool(status.paused);
    format!(
        "{{\"ok\":true,\"state\":\"{}\",\"releasing\":{releasing},\"paused\":{paused},\"config_index\":{},\"sim\":\"{sim}\",\"devices\":{},\"updates\":{},\"overruns\":{}}}",
        status.state, status.config_index, status.devices, status.updates, status.overruns
    )
}

fn json_bool(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn json_safe(text: &str) -> String {
    text.chars()
        .filter(|ch| *ch != '"' && *ch != '\\' && !ch.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SessionStatus {
        SessionStatus {
            state: "searching".to_string(),
            releasing: false,
            paused: false,
            config_index: -1,
            sim: "none".to_string(),
            devices: 0,
            updates: 0,
            overruns: 0,
        }
    }

    #[test]
    fn commands_match_the_tui_protocol() {
        let status = sample();
        assert!(reply(CMD_STATUS, &status).contains("\"state\":\"searching\""));
        assert_eq!(reply(CMD_RELOAD, &status), REPLY_OK);
        assert_eq!(reply(CMD_STOP, &status), REPLY_OK);
        assert_eq!(reply("nope", &status), REPLY_UNKNOWN);
        assert_eq!(
            socket_path(Some("/run/user/1"), 1),
            "/run/user/1/cargopit.sock"
        );
        assert_eq!(socket_path(None, 7), "/tmp/cargopit-7.sock");
    }

    #[test]
    fn listener_answers_status_and_stop() {
        let dir = std::env::temp_dir().join(format!("cargopit-sock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SOCKET_NAME);
        let listener = bind_listener(path.to_str().unwrap()).unwrap();
        let mut client = UnixStream::connect(&path).unwrap();
        client.write_all(b"status\n").unwrap();
        let effect = try_accept(&listener, &sample()).unwrap();
        assert_eq!(effect, ControlEffect::None);
        let mut reply_buf = [0u8; 256];
        let n = client.read(&mut reply_buf).unwrap();
        let text = String::from_utf8_lossy(&reply_buf[..n]);
        assert!(text.contains("\"ok\":true"));
        assert!(text.ends_with('\n'));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
