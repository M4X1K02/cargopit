use crate::consts::*;
use crate::paths;
use std::fs;
use std::io::{Read, Seek, SeekFrom};

pub fn read_recent_logs() -> Vec<String> {
    let dir = paths::log_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return vec![format!("No log directory at {}", dir.display())];
    };
    let mut files: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case(LOG_FILE_EXTENSION))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return vec![format!(
            "No {LOG_FILE_EXTENSION} files in {}",
            dir.display()
        )];
    }
    let mut lines = Vec::new();
    for path in files {
        lines.push(format!("--- {} ---", path.display()));
        lines.extend(tail_file(&path, MAX_LOG_LINES / 4));
    }
    if lines.len() > MAX_LOG_LINES {
        lines = lines[lines.len() - MAX_LOG_LINES..].to_vec();
    }
    lines
}

fn tail_file(path: &std::path::Path, max_lines: usize) -> Vec<String> {
    let Ok(mut file) = fs::File::open(path) else {
        return vec![format!("failed to read {}", path.display())];
    };
    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        let _ = file.seek(SeekFrom::Start(0));
        return vec![format!("failed to read {}", path.display())];
    }
    buf.lines()
        .rev()
        .take(max_lines)
        .map(str::to_string)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}
