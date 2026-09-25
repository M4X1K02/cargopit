//! Play-session control socket. One line in, one JSON line out.

pub const SOCKET_NAME: &str = "cargopit.sock";
pub const RUNTIME_DIR_ENV: &str = "XDG_RUNTIME_DIR";
pub const CMD_STATUS: &str = "status";
pub const CMD_RELOAD: &str = "reload";
pub const CMD_STOP: &str = "stop";
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

pub fn socket_path(runtime_dir: Option<&str>, uid: u32) -> String {
    match runtime_dir {
        Some(dir) if !dir.is_empty() => format!("{dir}/{SOCKET_NAME}"),
        _ => format!("/tmp/cargopit-{uid}.sock"),
    }
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
}
