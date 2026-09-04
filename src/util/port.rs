use std::net::TcpListener;

/// Find a free port by binding to port 0 and letting the OS assign one.
pub fn find_free_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
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
