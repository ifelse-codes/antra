use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::ChildStdout;

use crate::ipc::client::send_command;
use crate::ipc::protocol::{IpcPayload, RegisterRouteRequest, UnregisterRouteRequest};
use crate::util::output;

/// Patterns that indicate a server is listening on a port.
const PORT_PATTERNS: &[&str] = &[
    "Local:",
    "listening on",
    "listening at",
    "started on port",
    "Server listening on",
    "port",
    "Running on",
    "Running at",
    "Starting server on",
    "Listening on",
];

/// Watch a child process stdout for port changes and update the route accordingly.
///
/// Spawns a tokio task that monitors the child's stdout for lines containing
/// port information. When a new port is detected, it updates the route in the daemon.
/// Must be called from within a Tokio runtime — uses async IPC (never
/// `send_command_sync`, which would panic with "Cannot start a runtime
/// from within a runtime").
///
/// Only switches on an explicit host:port URL (127.0.0.1/localhost/0.0.0.0/::1).
/// Bare prose like "port 19999 reserved for metrics" never switches.
/// The new port is TCP-verified before switching, and re-registered with the
/// same owner PID as a managed route.
///
/// `current` is the route's port, shared with [`confirm_auto_port`] so the
/// two never switch the same route out from under each other.
pub fn watch_port_changes(
    stdout: ChildStdout,
    domain: String,
    current: Arc<AtomicU16>,
    child_pid: Option<u32>,
) {
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            // Print the line to user's terminal (passthrough)
            println!("{line}");

            // Try to extract a port from this line (host:port URLs only)
            if let Some(new_port) = extract_port_from_line(&line) {
                let current_port = current.load(Ordering::SeqCst);
                if new_port != current_port {
                    // Verify the new port actually accepts connections before
                    // abandoning a working route (Vite prints early).
                    if !port_accepts_connections(new_port).await {
                        output::print_warning(&format!(
                            "Detected port {new_port} in output, but nothing listens there yet — keeping port {current_port}",
                        ));
                        continue;
                    }
                    output::print_warning(&format!(
                        "Port changed: {} → {} (detected from output)",
                        current_port, new_port
                    ));
                    current.store(new_port, Ordering::SeqCst);
                    switch_route(&domain, new_port, child_pid).await;
                }
            }
        }
    });
}

/// Re-register `domain` on `new_port` with the same owner PID (managed).
async fn switch_route(domain: &str, new_port: u16, child_pid: Option<u32>) {
    let _ = send_command(IpcPayload::UnregisterRoute(UnregisterRouteRequest {
        domain: domain.to_string(),
    }))
    .await;

    if let Err(e) = send_command(IpcPayload::RegisterRoute(RegisterRouteRequest {
        domain: domain.to_string(),
        port: new_port,
        pid: child_pid,
        managed: true,
    }))
    .await
    {
        output::print_error(&format!("Failed to update route for port change: {e}"));
    } else {
        output::print_success(&format!("Route updated: {} → port {}", domain, new_port));
    }
}

/// How long [`confirm_auto_port`] waits before it believes a listener it
/// found elsewhere. A server can open another port first (`node --inspect`
/// opens 9229 before the app listens; a dev command may start an API before
/// the app), so an early sighting is not enough. A server with a hardcoded
/// port pays this once, as a few seconds of the 503 that names its port.
const AUTO_PORT_GRACE: Duration = Duration::from_secs(5);
/// When to tell the user that nothing answers on the assigned port.
const AUTO_PORT_WARN_AFTER: Duration = Duration::from_secs(10);
/// When to stop looking. A server that has not listened anywhere by now is
/// not starting, and the warning has already said what to do.
const AUTO_PORT_GIVE_UP: Duration = Duration::from_secs(60);
const AUTO_PORT_POLL: Duration = Duration::from_millis(500);

/// What [`confirm_auto_port`] should do after one look at the ports.
#[derive(Debug, PartialEq, Eq)]
enum AutoPortStep {
    /// Something answers on the assigned port, or the route already moved.
    Done,
    /// The server listens on exactly this other port: route there.
    Switch(u16),
    /// The server listens on several other ports: say so, guess none.
    Ambiguous(Vec<u16>),
    /// Nothing to go on yet.
    Wait,
}

/// One decision of [`confirm_auto_port`], pure so each case is tested.
///
/// * `assigned_answers`: the assigned port accepts connections.
/// * `elsewhere`: the ports the child's process group listens on, minus the
///   assigned one; `previous` is the same list from the last look. Nothing
///   is decided before `AUTO_PORT_GRACE`, and only on the same list twice in
///   a row, so a port the server opens on the way up is not mistaken for
///   the one it serves on.
fn auto_port_step(
    assigned_answers: bool,
    route_moved: bool,
    elapsed: Duration,
    elsewhere: &[u16],
    previous: &[u16],
) -> AutoPortStep {
    if assigned_answers || route_moved {
        return AutoPortStep::Done;
    }
    if elapsed < AUTO_PORT_GRACE || elsewhere.is_empty() || elsewhere != previous {
        return AutoPortStep::Wait;
    }
    match elsewhere {
        [only] => AutoPortStep::Switch(*only),
        many => AutoPortStep::Ambiguous(many.to_vec()),
    }
}

/// After `antra run` auto-assigned a port, make sure the server is actually
/// on it — and if it is not, find where it is.
///
/// A server that ignores `PORT` (an ordinary `listen(3000)`) left the route
/// pointing at a port nothing listened on, and every request got a 503 that
/// asked "is your server running?" while it was. The stdout watcher only
/// catches servers that print a `host:port` URL; this asks the OS which ports
/// the child's process group holds, so it works for a silent server too.
///
/// Stops as soon as the assigned port answers, the stdout watcher moves the
/// route, or `AUTO_PORT_GIVE_UP` passes. After moving the route itself it
/// keeps watching, and moves it back if the assigned port starts answering.
pub fn confirm_auto_port(
    domain: String,
    assigned: u16,
    current: Arc<AtomicU16>,
    child_pid: Option<u32>,
    command: String,
) {
    let Some(pgid) = child_pid else {
        return;
    };
    tokio::spawn(async move {
        let start = Instant::now();
        let mut previous: Vec<u16> = Vec::new();
        let mut warned = false;
        // Set once this task has moved the route. The move is a guess: a dev
        // command that starts two servers can open another port before the
        // one on $PORT, so keep watching the assigned port and move back if
        // it starts answering.
        let mut moved_to: Option<u16> = None;
        while start.elapsed() < AUTO_PORT_GIVE_UP {
            tokio::time::sleep(AUTO_PORT_POLL).await;
            if let Some(port) = moved_to {
                if current.load(Ordering::SeqCst) != port {
                    // The stdout watcher has moved it since: its call.
                    return;
                }
                if port_accepts_connections(assigned).await
                    && current
                        .compare_exchange(port, assigned, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                {
                    println!();
                    output::print_warning(&format!(
                        "Port {assigned} answers after all — moving the route back from {port}."
                    ));
                    switch_route(&domain, assigned, child_pid).await;
                    println!();
                    return;
                }
                continue;
            }
            let route_moved = current.load(Ordering::SeqCst) != assigned;
            let assigned_answers = !route_moved && port_accepts_connections(assigned).await;
            let elsewhere: Vec<u16> = if assigned_answers || route_moved {
                Vec::new()
            } else {
                tokio::task::spawn_blocking(move || crate::util::port::group_listening_ports(pgid))
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|&p| p != assigned)
                    .collect()
            };
            match auto_port_step(
                assigned_answers,
                route_moved,
                start.elapsed(),
                &elsewhere,
                &previous,
            ) {
                AutoPortStep::Done => return,
                AutoPortStep::Switch(port) => {
                    // Lose the race to the stdout watcher gracefully: only
                    // switch a route that is still on the assigned port.
                    if current
                        .compare_exchange(assigned, port, Ordering::SeqCst, Ordering::SeqCst)
                        .is_err()
                    {
                        return;
                    }
                    println!();
                    output::print_warning(&format!(
                        "Your server is listening on port {port}, not on the assigned port {assigned} (it ignores $PORT)."
                    ));
                    switch_route(&domain, port, child_pid).await;
                    output::print_warning(&format!(
                        "Next time, skip the guess: antra run --domain {domain} --port {port} -- {command}"
                    ));
                    println!();
                    moved_to = Some(port);
                    continue;
                }
                AutoPortStep::Ambiguous(ports) => {
                    let list = ports
                        .iter()
                        .map(u16::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!();
                    output::print_warning(&format!(
                        "Nothing answers on the assigned port {assigned}, but your server is listening on ports {list}."
                    ));
                    output::print_warning(&format!(
                        "Stop it (Ctrl+C) and re-run with the right one: antra run --domain {domain} --port <port> -- {command}"
                    ));
                    println!();
                    return;
                }
                AutoPortStep::Wait => {}
            }
            if !warned && start.elapsed() >= AUTO_PORT_WARN_AFTER {
                warned = true;
                println!();
                output::print_warning(&format!(
                    "Nothing is listening on port {assigned} yet — Antra assigned it and passed it to your server as $PORT."
                ));
                output::print_warning(&format!(
                    "If your server uses a fixed port, stop it (Ctrl+C) and re-run: antra run --domain {domain} --port <its port> -- {command}"
                ));
                println!();
            }
            previous = elsewhere;
        }
    });
}

/// Best-effort TCP check: does something accept on 127.0.0.1:port yet?
async fn port_accepts_connections(port: u16) -> bool {
    tokio::time::timeout(
        std::time::Duration::from_millis(500),
        tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")),
    )
    .await
    .is_ok_and(|r| r.is_ok())
}

/// Extract a port number from a log line.
///
/// Strict: only explicit host:port URLs (127.0.0.1/localhost/0.0.0.0/::1).
/// Bare prose like "port 19999 reserved for metrics" returns None — it must
/// never flip a live route.
fn extract_port_from_line(line: &str) -> Option<u16> {
    let lower = line.to_lowercase();

    // Must contain at least one of our patterns
    let has_pattern = PORT_PATTERNS
        .iter()
        .any(|p| lower.contains(&p.to_lowercase()));
    if !has_pattern {
        return None;
    }

    // Host:port URLs only — no bare "port N" fallback.
    extract_port_from_url(line)
}

/// Extract port from URL patterns like http://127.0.0.1:5173/ or http://localhost:3000
fn extract_port_from_url(line: &str) -> Option<u16> {
    let parts: Vec<&str> = line.split(':').collect();
    for part in &parts {
        let cleaned: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(port) = cleaned.parse::<u16>() {
            if (1024..=65535).contains(&port) {
                if let Some(before) = line.split(&format!(":{cleaned}")).next() {
                    let before_trimmed = before.trim_end();
                    if before_trimmed.ends_with("127.0.0.1")
                        || before_trimmed.ends_with("localhost")
                        || before_trimmed.ends_with("0.0.0.0")
                        || before_trimmed.ends_with("::1")
                    {
                        return Some(port);
                    }
                }
            }
        }
    }
    None
}

/// Extract port from text patterns like "port 3000" or "on port 8080".
/// Kept for unit-test coverage only — NOT used for route switching (too loose:
/// prose like "port 19999 reserved for metrics" must never flip a route).
#[allow(dead_code)]
fn extract_port_from_text(line: &str) -> Option<u16> {
    let lower = line.to_lowercase();
    let keywords = ["port ", "on port ", "port="];

    for keyword in &keywords {
        if let Some(pos) = lower.find(keyword) {
            let after = &line[pos + keyword.len()..];
            let num: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(port) = num.parse::<u16>() {
                if (1024..=65535).contains(&port) {
                    return Some(port);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_port_from_vite_output() {
        let line = "  Vite is listening on: http://127.0.0.1:5174/";
        assert_eq!(extract_port_from_line(line), Some(5174));
    }

    #[test]
    fn test_extract_port_from_express_output() {
        // Strict mode: bare "port 3000" prose without a host must NOT switch.
        // The loose text extractor still parses it (kept for coverage), but
        // the line-level extractor requires host:port.
        let line = "Example app listening on port 3000!";
        assert_eq!(extract_port_from_text(line), Some(3000));
        assert_eq!(extract_port_from_line(line), None);
    }

    #[test]
    fn test_prose_port_does_not_switch() {
        // Deep-dive repro: "Config port 19999 reserved for metrics" flipped route.
        let line = "Compiler ready. Config port 19999 reserved for metrics.";
        assert_eq!(extract_port_from_line(line), None);
    }

    #[test]
    fn test_host_port_url_switches() {
        let line = "Vite listening on http://127.0.0.1:5174/";
        assert_eq!(extract_port_from_line(line), Some(5174));
        let line = "Server running at http://localhost:3001 ready";
        assert_eq!(extract_port_from_line(line), Some(3001));
    }

    #[test]
    fn test_extract_port_from_url() {
        let line = "Local:   http://localhost:8080/";
        assert_eq!(extract_port_from_url(line), Some(8080));
    }

    #[test]
    fn test_no_port_in_normal_line() {
        let line = "Compiling...";
        assert_eq!(extract_port_from_line(line), None);
    }

    const LATE: Duration = Duration::from_secs(10);

    #[test]
    fn assigned_port_answering_ends_the_check() {
        assert_eq!(
            auto_port_step(true, false, LATE, &[], &[]),
            AutoPortStep::Done
        );
        // Even with other listeners: a server on $PORT that also opens a
        // debugger port is fine, and must not be moved.
        assert_eq!(
            auto_port_step(true, false, LATE, &[9229], &[9229]),
            AutoPortStep::Done
        );
    }

    #[test]
    fn a_route_the_stdout_watcher_moved_is_left_alone() {
        assert_eq!(
            auto_port_step(false, true, LATE, &[3000], &[3000]),
            AutoPortStep::Done
        );
    }

    #[test]
    fn a_hardcoded_port_seen_twice_is_switched_to() {
        // The A2 case: `listen(3000)` ignoring PORT=4000.
        assert_eq!(
            auto_port_step(false, false, LATE, &[3000], &[3000]),
            AutoPortStep::Switch(3000)
        );
    }

    #[test]
    fn one_sighting_or_an_early_one_is_not_enough() {
        // First sighting: `previous` is still empty.
        assert_eq!(
            auto_port_step(false, false, LATE, &[3000], &[]),
            AutoPortStep::Wait
        );
        // Changed since last look — still settling.
        assert_eq!(
            auto_port_step(false, false, LATE, &[3000], &[9229]),
            AutoPortStep::Wait
        );
        // Inside the grace window, `node --inspect` may only have opened
        // its debugger so far.
        assert_eq!(
            auto_port_step(false, false, Duration::from_secs(3), &[9229], &[9229]),
            AutoPortStep::Wait
        );
        assert_eq!(
            auto_port_step(false, false, LATE, &[], &[]),
            AutoPortStep::Wait
        );
    }

    #[test]
    fn several_listeners_are_reported_not_guessed() {
        assert_eq!(
            auto_port_step(false, false, LATE, &[3000, 9229], &[3000, 9229]),
            AutoPortStep::Ambiguous(vec![3000, 9229])
        );
    }
}
