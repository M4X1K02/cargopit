use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use crate::consts;
use crate::paths;
use crate::process::SessionKind;

#[derive(Debug, Clone)]
pub struct LogLine {
    pub source: String,
    pub text: String,
}

pub struct LogState {
    pub lines: Vec<LogLine>,
    pub scroll: usize,
    pub filter: LogFilter,
    files: Vec<TailedFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFilter {
    All,
    Play,
    Test,
    File,
}

impl LogFilter {
    pub fn cycle(self) -> Self {
        match self {
            LogFilter::All => LogFilter::Play,
            LogFilter::Play => LogFilter::Test,
            LogFilter::Test => LogFilter::File,
            LogFilter::File => LogFilter::All,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LogFilter::All => "all",
            LogFilter::Play => "play",
            LogFilter::Test => "test",
            LogFilter::File => "file",
        }
    }
}

struct TailedFile {
    path: PathBuf,
    offset: u64,
}

impl LogState {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            scroll: 0,
            filter: LogFilter::All,
            files: Vec::new(),
        }
    }

    pub fn refresh_files(&mut self) {
        let dir = paths::log_dir();
        let Ok(entries) = fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("log") {
                continue;
            }
            if self.files.iter().any(|f| f.path == path) {
                continue;
            }
            self.files.push(TailedFile { path, offset: 0 });
        }
        self.read_new();
    }

    fn read_new(&mut self) {
        let mut incoming = Vec::new();
        for file in &mut self.files {
            let Ok(mut handle) = File::open(&file.path) else {
                continue;
            };
            let _ = handle.seek(SeekFrom::Start(file.offset));
            let mut buf = String::new();
            let _ = handle.read_to_string(&mut buf);
            if let Ok(pos) = handle.stream_position() {
                file.offset = pos;
            }
            let source = file
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string();
            for line in buf.lines() {
                incoming.push((source.clone(), line.to_string()));
            }
        }
        for (source, text) in incoming {
            self.push(source, text);
        }
    }

    pub fn push(&mut self, source: String, text: String) {
        let text = strip_ansi(&text);
        self.lines.push(LogLine { source, text });
        if self.lines.len() > consts::LOG_TAIL_MAX_LINES {
            let extra = self.lines.len() - consts::LOG_TAIL_MAX_LINES;
            self.lines.drain(0..extra);
        }
    }

    pub fn push_child(&mut self, kind: SessionKind, text: String) {
        for line in text.lines() {
            self.push(kind.as_str().to_string(), line.to_string());
        }
    }

    pub fn recent_from(&self, source: &str, limit: usize) -> Vec<&LogLine> {
        let matching: Vec<&LogLine> = self
            .lines
            .iter()
            .filter(|line| line.source == source)
            .collect();
        let start = matching.len().saturating_sub(limit);
        matching[start..].to_vec()
    }

    pub fn visible(&self) -> Vec<&LogLine> {
        self.lines
            .iter()
            .filter(|line| match self.filter {
                LogFilter::All => true,
                LogFilter::Play => line.source == "play",
                LogFilter::Test => line.source == "test",
                LogFilter::File => line.source.ends_with(consts::LOG_GLOB_SUFFIX)
                    || (!line.source.eq("play") && !line.source.eq("test")),
            })
            .collect()
    }
}

impl Default for LogState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != consts::ANSI_ESC {
            out.push(ch);
            continue;
        }
        skip_ansi_sequence(&mut chars);
    }
    out
}

fn skip_ansi_sequence(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    if chars.peek() != Some(&consts::ANSI_CSI) {
        return;
    }
    chars.next();
    for next in chars.by_ref() {
        if next.is_ascii_alphabetic() {
            break;
        }
    }
}

pub fn is_error_line(text: &str) -> bool {
    text.contains(consts::SLOG_TAG_ERROR) || text.contains(consts::SLOG_TAG_FATAL)
}

pub fn is_warn_line(text: &str) -> bool {
    text.contains(consts::SLOG_TAG_WARN)
}

pub fn is_info_line(text: &str) -> bool {
    text.contains(consts::SLOG_TAG_INFO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_keeps_slog_tags() {
        let raw = "\u{1b}[31m<error>\u{1b}[0m Error opening serial port";
        assert_eq!(strip_ansi(raw), "<error> Error opening serial port");
    }

    #[test]
    fn push_strips_ansi() {
        let mut logs = LogState::new();
        logs.push("file".into(), "\u{1b}[33m<warn>\u{1b}[0m skip".into());
        assert_eq!(logs.lines[0].text, "<warn> skip");
        assert!(is_warn_line(&logs.lines[0].text));
        assert!(!is_error_line(&logs.lines[0].text));
        logs.push("test".into(), "phase lock".into());
        assert_eq!(logs.recent_from("test", 1)[0].text, "phase lock");
    }
}
