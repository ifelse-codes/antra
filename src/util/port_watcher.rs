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
pub fn watch_port_changes(
    stdout: ChildStdout,
    domain: String,
    initial_port: u16,
    child_pid: Option<u32>,
) {
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut current_port = initial_port;

        while let Ok(Some(line)) = lines.next_line().await {
            // Print the line to user's terminal (passthrough)
            println!("{line}");

            // Try to extract a port from this line (host:port URLs only)
            if let Some(new_port) = extract_port_from_line(&line) {
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

                    // Unregister old route
                    let _ = send_command(IpcPayload::UnregisterRoute(UnregisterRouteRequest {
                        domain: domain.clone(),
                    }))
                    .await;

                    // Register new route with the same owner PID (managed)
                    if let Err(e) = send_command(IpcPayload::RegisterRoute(RegisterRouteRequest {
                        domain: domain.clone(),
                        port: new_port,
                        pid: child_pid,
                        managed: true,
                    }))
                    .await
                    {
                        output::print_error(&format!(
                            "Failed to update route for port change: {e}"
                        ));
                    } else {
                        output::print_success(&format!(
                            "Route updated: {} → port {}",
                            domain, new_port
                        ));
                    }

                    current_port = new_port;
                }
            }
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
}
