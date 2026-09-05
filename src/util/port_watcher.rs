use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::ChildStdout;

use crate::ipc::client::send_command_sync;
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
    "Starting server on",
    "Listening on",
];

/// Watch a child process stdout for port changes and update the route accordingly.
///
/// Spawns a tokio task that monitors the child's stdout for lines containing
/// port information. When a new port is detected, it updates the route in the daemon.
pub fn watch_port_changes(stdout: ChildStdout, domain: String, initial_port: u16) {
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut current_port = initial_port;

        while let Ok(Some(line)) = lines.next_line().await {
            // Print the line to user's terminal (passthrough)
            println!("{line}");

            // Try to extract a port from this line
            if let Some(new_port) = extract_port_from_line(&line) {
                if new_port != current_port {
                    output::print_warning(&format!(
                        "Port changed: {} → {} (detected from output)",
                        current_port, new_port
                    ));

                    // Unregister old route
                    let _ =
                        send_command_sync(IpcPayload::UnregisterRoute(UnregisterRouteRequest {
                            domain: domain.clone(),
                        }));

                    // Register new route
                    if let Err(e) =
                        send_command_sync(IpcPayload::RegisterRoute(RegisterRouteRequest {
                            domain: domain.clone(),
                            port: new_port,
                            pid: None,
                        }))
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

/// Extract a port number from a log line.
fn extract_port_from_line(line: &str) -> Option<u16> {
    let lower = line.to_lowercase();

    // Must contain at least one of our patterns
    let has_pattern = PORT_PATTERNS
        .iter()
        .any(|p| lower.contains(&p.to_lowercase()));
    if !has_pattern {
        return None;
    }

    // Extract port from common URL patterns
    if let Some(port) = extract_port_from_url(line) {
        return Some(port);
    }

    // Extract port from "port XXXX" pattern
    if let Some(port) = extract_port_from_text(line) {
        return Some(port);
    }

    None
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

/// Extract port from text patterns like "port 3000" or "on port 8080"
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
        let line = "Example app listening on port 3000!";
        assert_eq!(extract_port_from_line(line), Some(3000));
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
