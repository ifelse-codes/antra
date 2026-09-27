//! The daemon log: one path, shared by every writer and reader.
//!
//! The daemon's `tracing` output goes to its stdout/stderr, which the CLI
//! redirects into a file. That only works if everyone agrees on the path, and
//! they used not to: `antra proxy start` logged to
//! `data_local_dir()/antra/daemon.log` while the launchd service wrote to
//! `~/.config/antra/daemon.log` — two different files on macOS, so a
//! service-managed daemon's output was unreachable from the CLI. The
//! auto-started daemon (the one most people get, via `antra run`) discarded
//! its output entirely, which is why "HTTPS server failed" was a message
//! nobody could act on.
//!
//! One path, one writer helper, one reader. `antra logs` reads it;
//! `antra doctor` tails the errors out of it.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Beyond this, the log is truncated on the next daemon start. A debug log
/// that grows without bound is a bug waiting to fill someone's disk; nobody
/// reads the last 4 MiB of it anyway.
pub const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Where the daemon's stdout and stderr land.
///
/// `data_local_dir()` rather than `config_dir()`: on macOS both resolve to
/// `~/Library/Application Support`, and on Linux this puts a log where XDG
/// says logs go. The launchd plist points at this same function, which is
/// what keeps the service and the CLI on one file.
pub fn daemon_log_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("antra")
        .join("daemon.log")
}

/// Open the log for appending, creating it and its directory if needed.
///
/// Truncates an oversized log rather than rotating it: one file, no index,
/// and a fresh file per oversized session is easier to reason about than a
/// scheme whose numbering someone has to remember.
pub fn open_for_append() -> Result<File> {
    let path = daemon_log_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    if oversized(&path) {
        tracing::info!(
            path = %path.display(),
            "Daemon log exceeded {MAX_LOG_BYTES} bytes — starting a new one"
        );
        File::create(&path).with_context(|| format!("Failed to truncate {}", path.display()))
    } else {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("Failed to open {}", path.display()))
    }
}

fn oversized(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() > MAX_LOG_BYTES)
        .unwrap_or(false)
}

/// The last `count` lines of the log, oldest first.
///
/// Reads from the end of the file rather than slurping it: the log can be
/// megabytes, and `antra logs` should be instant whether the daemon has been
/// up for an hour or a week.
pub fn tail(count: usize) -> Result<Vec<String>> {
    let path = daemon_log_path();
    let file = File::open(&path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);

    // Walk backwards in blocks until `count` newlines have been seen, or the
    // file starts. A malformed (non-UTF-8) byte costs us a lossy decode at
    // the end rather than a failed command.
    const BLOCK: usize = 8 * 1024;
    let mut buf = vec![0u8; BLOCK];
    let mut collected: Vec<u8> = Vec::new();
    let mut newlines = 0usize;
    let mut pos = reader
        .seek(SeekFrom::End(0))
        .with_context(|| format!("Failed to seek in {}", path.display()))?;

    while newlines <= count {
        let read_at = pos.saturating_sub(BLOCK as u64);
        let len = (pos - read_at) as usize;
        if len == 0 {
            let mut rest = Vec::new();
            reader.seek(SeekFrom::Start(0))?;
            reader.read_to_end(&mut rest)?;
            collected = rest;
            break;
        }
        reader.seek(SeekFrom::Start(read_at))?;
        reader.read_exact(&mut buf[..len])?;
        pos = read_at;
        newlines += buf[..len].iter().filter(|b| **b == b'\n').count();
        let mut block = buf[..len].to_vec();
        block.extend_from_slice(&collected);
        collected = block;
    }

    let text = String::from_utf8_lossy(&collected);
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if lines.len() > count {
        lines = lines.split_off(lines.len() - count);
    }
    Ok(lines)
}

/// The most recent lines that look like failures, for `antra doctor`.
///
/// `tracing`'s default formatter prefixes with the level, so this is a
/// substring match on the rendered line rather than a parse — the daemon
/// writes text, and doctor needs to *show* it, not analyse it.
pub fn recent_errors(count: usize) -> Vec<String> {
    let Ok(lines) = tail(500) else {
        return Vec::new();
    };
    lines
        .into_iter()
        .rev()
        .filter(|l| l.contains("ERROR") || l.contains("WARN"))
        .take(count)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}
