//! `antra logs` — read what the daemon actually said.
//!
//! Before this, a daemon failure was a `tracing::error!` that went to
//! `/dev/null`, so "HTTPS server failed" existed only in the code that
//! printed it. Several user-test sessions asked for exactly this command
//! (`tests/user-test-2026-09-06.md`, `…-1355.md`).

use std::io::{Read, Seek, SeekFrom};
use std::time::Duration;

use anyhow::Result;
use colored::Colorize;

use crate::util::logs;

const FOLLOW_POLL: Duration = Duration::from_millis(250);

pub fn execute(follow: bool, lines: usize) -> Result<()> {
    let path = logs::daemon_log_path();

    if !path.exists() {
        // Not an error: nobody has started a daemon yet, which is a normal
        // state. Say where the log will appear so the next step is obvious.
        println!("{}", "ANTRA LOGS".bold());
        println!();
        println!("  No daemon log yet.");
        println!("  It is created when the daemon starts, at:");
        println!("    {}", path.display().to_string().dimmed());
        println!();
        println!(
            "  Start one with {} and re-run this command.",
            "antra proxy start".bold()
        );
        return Ok(());
    }

    println!("{}", "ANTRA LOGS".bold());
    println!();
    println!("  {}", path.display().to_string().dimmed());
    println!();

    let mut offset = print_tail(lines)?;
    if !follow {
        return Ok(());
    }

    println!("{}", "  Following — Ctrl-C to stop".dimmed());
    println!();
    follow_from(&mut offset)
}

/// Print the last `count` lines; return the byte offset just past them, which
/// is where a follow should resume.
fn print_tail(count: usize) -> Result<u64> {
    let path = logs::daemon_log_path();
    for line in logs::tail(count)? {
        println!("{line}");
    }
    Ok(std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0))
}

/// Poll from `offset` for appended bytes and print them as they arrive.
///
/// A poll loop rather than a pipe: the log is also written by `antra proxy
/// start` and by the launchd service, and following the *file* means
/// `antra logs -f` shows output no matter which of those is the writer.
fn follow_from(offset: &mut u64) -> Result<()> {
    let path = logs::daemon_log_path();
    loop {
        std::thread::sleep(FOLLOW_POLL);
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        if size < *offset {
            // Truncated (see `logs::open_for_append`) — restart from the top
            // rather than printing from an offset past the new end.
            println!(
                "{}",
                "  — log was truncated, restarting from the top —".dimmed()
            );
            *offset = 0;
        }
        if size == *offset {
            continue;
        }
        let mut reader = std::io::BufReader::new(file);
        if reader.seek(SeekFrom::Start(*offset)).is_err() {
            continue;
        }
        let mut buf = String::new();
        if reader.read_to_string(&mut buf).is_err() {
            continue;
        }
        *offset += buf.len() as u64;
        print!("{buf}");
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
}
