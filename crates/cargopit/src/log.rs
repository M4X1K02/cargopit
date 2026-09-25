//! slog screen and file lines. Verbosity matches the C host: debug at `-v`, trace at `-vv`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use cargopit_devices::clock::WallStamp;

pub const TAG_INFO: &str = "info";
pub const TAG_WARN: &str = "warn";
pub const TAG_DEBUG: &str = "debug";
pub const TAG_ERROR: &str = "error";
pub const TAG_TRACE: &str = "trace";
pub const TAG_FATAL: &str = "fatal";
pub const DEFAULT_LOG_STEM: &str = "cargopit.log";
const VERBOSITY_DEBUG: u8 = 1;
const VERBOSITY_TRACE: u8 = 2;
const COLOR_RESET: &str = "\x1b[0m";
const COLOR_RED: &str = "\x1b[31m";
const COLOR_GREEN: &str = "\x1b[32m";
const COLOR_YELLOW: &str = "\x1b[33m";
const COLOR_BLUE: &str = "\x1b[34m";
const COLOR_MAGENTA: &str = "\x1b[35m";
const COLOR_CYAN: &str = "\x1b[36m";
const LOG_EXTENSION: &str = "log";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Debug,
    Error,
    Trace,
    Fatal,
}

impl Level {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Info => TAG_INFO,
            Self::Warn => TAG_WARN,
            Self::Debug => TAG_DEBUG,
            Self::Error => TAG_ERROR,
            Self::Trace => TAG_TRACE,
            Self::Fatal => TAG_FATAL,
        }
    }

    fn enabled(self, verbosity: u8) -> bool {
        match self {
            Self::Debug => verbosity >= VERBOSITY_DEBUG,
            Self::Trace => verbosity >= VERBOSITY_TRACE,
            _ => true,
        }
    }

    fn color(self) -> &'static str {
        match self {
            Self::Info => COLOR_GREEN,
            Self::Warn => COLOR_YELLOW,
            Self::Debug => COLOR_BLUE,
            Self::Error => COLOR_RED,
            Self::Trace => COLOR_CYAN,
            Self::Fatal => COLOR_MAGENTA,
        }
    }
}

pub fn format_line(level: Level, message: &str) -> String {
    format!("<{}> {message}", level.tag())
}

pub fn emit(verbosity: u8, level: Level, message: &str, stamp: &WallStamp) -> Option<String> {
    if !level.enabled(verbosity) {
        return None;
    }
    Some(format_record(level, message, stamp))
}

pub fn destination(requested: Option<&str>) -> (PathBuf, String) {
    if let Some(placed) = existing_request(requested) {
        return placed;
    }
    (
        cargopit_config::paths::log_dir(),
        DEFAULT_LOG_STEM.to_string(),
    )
}

pub fn dated_name(stem: &str, stamp: &WallStamp) -> String {
    format!(
        "{stem}-{:04}-{:02}-{:02}.{LOG_EXTENSION}",
        stamp.year, stamp.month, stamp.day
    )
}

pub fn append(dir: &Path, stem: &str, line: &str, stamp: &WallStamp) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let path = dir.join(dated_name(stem, stamp));
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return false;
    };
    writeln!(file, "{line}").is_ok()
}

fn format_record(level: Level, message: &str, stamp: &WallStamp) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03} {}<{}>{COLOR_RESET} {message}",
        stamp.hour,
        stamp.minute,
        stamp.second,
        stamp.millis,
        level.color(),
        level.tag()
    )
}

fn existing_request(requested: Option<&str>) -> Option<(PathBuf, String)> {
    let path = Path::new(requested?);
    if !path.is_file() {
        return None;
    }
    let stem = path.file_name()?.to_str()?.to_string();
    let parent = path.parent().filter(|dir| !dir.as_os_str().is_empty())?;
    Some((parent.to_path_buf(), stem))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp() -> WallStamp {
        WallStamp::from_unix_ms(0)
    }

    #[test]
    fn verbosity_gates_debug_and_trace() {
        let stamp = stamp();
        assert!(emit(0, Level::Debug, "hidden", &stamp).is_none());
        let info = emit(0, Level::Info, "shown", &stamp).unwrap();
        assert_eq!(
            info,
            format!("00:00:00.000 {COLOR_GREEN}<info>{COLOR_RESET} shown")
        );
        assert!(emit(1, Level::Debug, "dbg", &stamp)
            .unwrap()
            .contains("<debug>"));
        assert!(emit(1, Level::Trace, "hidden", &stamp).is_none());
        assert!(emit(2, Level::Trace, "tr", &stamp)
            .unwrap()
            .contains("<trace>"));
    }

    #[test]
    fn dated_file_appends_the_colored_line() {
        let stamp = stamp();
        let dir = std::env::temp_dir().join(format!("cargopit-slog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let line = emit(
            0,
            Level::Info,
            "test step: preparing test with 0 devices...",
            &stamp,
        )
        .unwrap();
        assert!(append(&dir, DEFAULT_LOG_STEM, &line, &stamp));
        let path = dir.join(dated_name(DEFAULT_LOG_STEM, &stamp));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("<info>"));
        assert!(text.contains("test step: preparing test with 0 devices..."));
        let requested = dir.join("session.log");
        std::fs::write(&requested, b"seed").unwrap();
        let (placed, stem) = destination(Some(requested.to_str().unwrap()));
        assert_eq!(placed, dir);
        assert_eq!(stem, "session.log");
        assert_eq!(
            destination(Some("/no/such/cargopit.log")).1,
            DEFAULT_LOG_STEM
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
