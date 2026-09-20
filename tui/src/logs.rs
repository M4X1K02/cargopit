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
