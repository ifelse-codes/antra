use std::net::TcpListener;

/// Find a free port by binding to port 0 and letting the OS assign one.
pub fn find_free_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

/// Find a free port in the 4000-4999 range.
/// Falls back to any free port if the range is exhausted.
pub fn find_free_port_in_range() -> anyhow::Result<u16> {
    // Try ports in the 4000-4999 range
    for port in 4000..5000 {
        if is_port_available(port) {
            return Ok(port);
        }
    }
    // Fallback to any free port
    find_free_port()
}

/// Find a free port starting from a preferred port, scanning forward in range.
/// If the preferred port is taken, automatically try the next one.
/// Returns the actual port assigned (may differ from preferred).
pub fn find_free_port_with_fallback(preferred: u16) -> anyhow::Result<u16> {
    // If preferred is in range and available, use it
    if preferred >= 4000 && preferred < 5000 && is_port_available(preferred) {
        return Ok(preferred);
    }

    // Scan forward from preferred (wrap around at 5000)
    let start = if preferred >= 4000 && preferred < 5000 { preferred } else { 4000 };
    for port in start..5000 {
        if is_port_available(port) {
            if port != preferred {
                tracing::debug!(
                    preferred, assigned = port,
                    "Preferred port unavailable, auto-assigned new port"
                );
            }
            return Ok(port);
        }
    }

    // Wrap around and try 4000..start
    for port in 4000..start {
        if is_port_available(port) {
            tracing::debug!(
                preferred, assigned = port,
                "Preferred port unavailable, auto-assigned new port"
            );
            return Ok(port);
        }
    }

    // Fallback to any free port
    find_free_port()
}

/// Check if a port is available
fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Try to detect the port from a command's arguments.
///
/// Checks for:
/// - `--port PORT` or `-p PORT` flags
/// - `python3 -m http.server PORT` patterns
/// - `vite --port PORT` / `next dev -p PORT` patterns
/// - Bare port number as last argument for known servers
pub fn detect_port_from_command(command: &[String]) -> Option<u16> {
    if command.is_empty() {
        return None;
    }

    // Join all args for pattern matching
    let joined = command.join(" ");

    // 1. Check for --port PORT or -p PORT flags (most common)
    for i in 0..command.len() {
        if (command[i] == "--port" || command[i] == "-p") && i + 1 < command.len() {
            if let Ok(port) = command[i + 1].parse::<u16>() {
                return Some(port);
            }
        }
        // Handle --port=PORT syntax
        if let Some(rest) = command[i].strip_prefix("--port=") {
            if let Ok(port) = rest.parse::<u16>() {
                return Some(port);
            }
        }
        if let Some(rest) = command[i].strip_prefix("-p=") {
            if let Ok(port) = rest.parse::<u16>() {
                return Some(port);
            }
        }
    }

    // 2. python3 -m http.server PORT (port is the first numeric arg after http.server)
    if joined.contains("http.server") || joined.contains("SimpleHTTPServer") {
        // Find the position of "http.server" and look for the first numeric arg after it
        if let Some(pos) = command.iter().position(|a| a == "http.server") {
            for arg in command.iter().skip(pos + 1) {
                if let Ok(port) = arg.parse::<u16>() {
                    return Some(port);
                }
                // Stop if we hit a flag (skip non-numeric args like --directory)
            }
        }
    }

    // 3. node/tsx/ts-node with --port already handled above, but also check
    //    for `node server.js` or `tsx server.ts` where PORT is in env or config
    //    — we can't detect these without running the command, so skip.

    // 4. Ruby: rackup, rails server, etc.
    if joined.contains("rackup") || joined.contains("rails server") {
        if let Some(rest) = joined.strip_prefix("-p ") {
            if let Ok(port) = rest.trim().parse::<u16>() {
                return Some(port);
            }
        }
    }

    // 5. Go: air, gin, etc. — typically use --port already handled above

    None
}

/// Frameworks that ignore the PORT env var and need explicit --port flag injection.
/// Returns the modified command with --port flag injected if applicable.
pub fn inject_port_flag(command: &[String], port: u16) -> Vec<String> {
    if command.is_empty() {
        return command.to_vec();
    }

    let first = command[0].as_str();
    let rest = &command[1..];

    // Check if this is a framework that needs --port injection
    let needs_port_injection = match first {
        // Vite and derivatives
        "vite" | "vite-dev" => true,
        // Astro
        "astro" => true,
        // Angular CLI
        "ng" => true,
        // Expo / React Native
        "expo" => true,
        "npx" if rest.first().map_or(false, |s| s == "expo") => true,
        // Create React App
        "react-scripts" => true,
        // Vue CLI
        "vue" => true,
        "npx" if rest.first().map_or(false, |s| s == "vue") => true,
        // Svelte
        "npx" if rest.first().map_or(false, |s| s.starts_with("svelte")) => true,
        // Solid
        "npx" if rest.first().map_or(false, |s| s.starts_with("solid")) => true,
        _ => false,
    };

    if !needs_port_injection {
        return command.to_vec();
    }

    // Check if --port is already present
    let has_port_flag = command.windows(2).any(|w| {
        (w[0] == "--port" || w[0] == "-p")
            || w[0].starts_with("--port=")
            || w[0].starts_with("-p=")
    });

    if has_port_flag {
        return command.to_vec();
    }

    // Inject --port flag
    let mut new_command = command.to_vec();
    new_command.push("--port".to_string());
    new_command.push(port.to_string());
    new_command
}

/// Prompt the user for the port their server listens on.
/// Returns None if the user doesn't provide a valid port.
pub fn prompt_for_port() -> Option<u16> {
    use colored::Colorize;

    println!();
    print!(
        "  {} ",
        "What port does your server listen on? (e.g., 3000, 8080)".yellow()
    );
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let input = input.trim();
        if let Ok(port) = input.parse::<u16>() {
            return Some(port);
        }
    }
    None
}
