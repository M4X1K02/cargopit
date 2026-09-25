//! slog tag names. Verbosity matches the C host: debug at `-v`, trace at `-vv`.

pub const TAG_INFO: &str = "info";
pub const TAG_WARN: &str = "warn";
pub const TAG_DEBUG: &str = "debug";
pub const TAG_ERROR: &str = "error";
pub const TAG_TRACE: &str = "trace";
pub const TAG_FATAL: &str = "fatal";
const VERBOSITY_DEBUG: u8 = 1;
const VERBOSITY_TRACE: u8 = 2;

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
}

pub fn format_line(level: Level, message: &str) -> String {
    format!("<{}> {message}", level.tag())
}

pub fn emit(verbosity: u8, level: Level, message: &str) -> Option<String> {
    if !level.enabled(verbosity) {
        return None;
    }
    Some(format_line(level, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbosity_gates_debug_and_trace() {
        assert!(emit(0, Level::Debug, "hidden").is_none());
        assert!(emit(0, Level::Info, "shown").unwrap().contains("<info>"));
        assert!(emit(1, Level::Debug, "dbg").unwrap().contains("<debug>"));
        assert!(emit(1, Level::Trace, "hidden").is_none());
        assert!(emit(2, Level::Trace, "tr").unwrap().contains("<trace>"));
    }
}
