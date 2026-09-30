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

/// Check if a port is available for binding on loopback.
///
/// Binds 127.0.0.1 (and ::1) and — because bind semantics can lie (e.g.
/// BSD `SO_REUSEADDR`: a second specific-address bind succeeds even though
/// a wildcard-bound server like `python3 -m http.server` owns the port) —
/// also probe-connects: anything accepting on loopback means taken.
pub fn is_port_available(port: u16) -> bool {
    let v4 = TcpListener::bind(("127.0.0.1", port));
    let v6 = TcpListener::bind(("::1", port));
    // Release both before the connect probe below, or it finds our own
    // listener and every port reads as taken.
    match loopback_binds(v4, v6) {
        Ok(listeners) => drop(listeners),
        Err(_) => return false,
    }
    if std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        std::time::Duration::from_millis(200),
    )
    .is_ok()
    {
        return false;
    }
    true
}

/// Combine the two loopback binds for one port into "both", "IPv4 only", or
/// an error.
///
/// Antra binds `::1` beside `127.0.0.1` so `localhost`, which modern macOS
/// resolves to `::1` first, can never reach some other IPv6 listener on the
/// same port. That reason disappears on a host with no IPv6 loopback: nothing
/// can listen on `::1` there, so there is nothing to shadow. Before this, any
/// `::1` failure counted as "port in use", and on such a host (booted with
/// `ipv6.disable=1`, some containers and WSL setups) every port read as taken
/// and the daemon refused to start with `Both 443 and 8443 are in use` on
/// free ports — ROADMAP C15.
///
/// So the `::1` bind is dropped only for [`ipv6_loopback_unavailable`]; any
/// other failure, above all the port being taken, is still an error, which
/// keeps callers from half-listening. The IPv4 bind is always required.
///
/// Generic over the listener so the sync and async binds share one decision,
/// and so it can be tested without a host that lacks IPv6.
pub fn loopback_binds<L>(
    v4: std::io::Result<L>,
    v6: std::io::Result<L>,
) -> std::io::Result<Vec<L>> {
    let mut listeners = vec![v4?];
    match v6 {
        Ok(listener) => listeners.push(listener),
        Err(e) if ipv6_loopback_unavailable(&e) => {}
        Err(e) => return Err(e),
    }
    Ok(listeners)
}

/// Whether a failed `::1` bind means the host has no IPv6 loopback, rather
/// than that the port is taken or forbidden.
///
/// `EAFNOSUPPORT`: the kernel has no IPv6 at all, so the socket cannot even
/// be created. `EADDRNOTAVAIL`: IPv6 exists but `::1` is not configured, as
/// with `net.ipv6.conf.lo.disable_ipv6=1`. `EADDRINUSE` and `EACCES` are
/// deliberately not here.
pub fn ipv6_loopback_unavailable(e: &std::io::Error) -> bool {
    if e.kind() == std::io::ErrorKind::AddrNotAvailable {
        return true;
    }
    #[cfg(unix)]
    let no_ipv6 = libc::EAFNOSUPPORT;
    #[cfg(windows)]
    let no_ipv6 = 10047; // WSAEAFNOSUPPORT
    #[cfg(any(unix, windows))]
    if e.raw_os_error() == Some(no_ipv6) {
        return true;
    }
    false
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
        if let Some(pos) = command.iter().position(|a| a == "http.server") {
            for arg in command.iter().skip(pos + 1) {
                if let Ok(port) = arg.parse::<u16>() {
                    return Some(port);
                }
            }
        }
    }

    // 3. Ruby: rackup, rails server, etc.
    if joined.contains("rackup") || joined.contains("rails server") {
        for i in 0..command.len() {
            if command[i] == "-p" && i + 1 < command.len() {
                if let Ok(port) = command[i + 1].parse::<u16>() {
                    return Some(port);
                }
            }
            if let Some(rest) = command[i].strip_prefix("-p=") {
                if let Ok(port) = rest.parse::<u16>() {
                    return Some(port);
                }
            }
        }
    }

    // 4. Django: python manage.py runserver [PORT]
    if joined.contains("manage.py") && joined.contains("runserver") {
        if let Some(pos) = command.iter().position(|a| a == "runserver") {
            for arg in command.iter().skip(pos + 1) {
                if let Ok(port) = arg.parse::<u16>() {
                    return Some(port);
                }
                if arg.starts_with('-') {
                    break;
                }
            }
        }
    }

    // 5. npm/pnpm/yarn/bun scripts forwarding: npm run dev -- --port PORT
    //    Also handles: pnpm dev --port PORT, bun run dev --port PORT
    if let Some(dash_pos) = command.iter().position(|a| a == "--") {
        for i in (dash_pos + 1)..command.len() {
            if (command[i] == "--port" || command[i] == "-p") && i + 1 < command.len() {
                if let Ok(port) = command[i + 1].parse::<u16>() {
                    return Some(port);
                }
            }
            if let Some(rest) = command[i].strip_prefix("--port=") {
                if let Ok(port) = rest.parse::<u16>() {
                    return Some(port);
                }
            }
        }
    }

    // 6. Go: air, gin, etc. — typically use --port already handled above

    None
}

/// Try to detect a port pinned inside a package.json script body.
///
/// For a Node project the argv Antra execs is `npm run dev` — the real
/// command, and any `--port 3001` in it, stays inside the script string in
/// `package.json`. `detect_port_from_command` only ever sees the argv, so
/// it cannot find that port. Split the script the way a shell would and
/// hand the tokens to the same parser, so there is exactly one port-flag
/// parser in the tree.
///
/// Returns `None` when the script pins no port, or when the pin is
/// something only the shell can resolve (`--port $PORT`). Guessing there
/// would be worse than falling back to the framework default.
pub fn detect_port_from_script(script: &str) -> Option<u16> {
    detect_port_from_command(&crate::config::project::split_command_string(script))
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
        "npx" if rest.first().is_some_and(|s| s == "expo") => true,
        // Create React App
        "react-scripts" => true,
        // Vue CLI
        "vue" => true,
        "npx" if rest.first().is_some_and(|s| s == "vue") => true,
        // Svelte
        "npx" if rest.first().is_some_and(|s| s.starts_with("svelte")) => true,
        // Solid
        "npx" if rest.first().is_some_and(|s| s.starts_with("solid")) => true,
        _ => false,
    };

    if !needs_port_injection {
        return command.to_vec();
    }

    // Check if --port is already present
    let has_port_flag = command.windows(2).any(|w| {
        (w[0] == "--port" || w[0] == "-p") || w[0].starts_with("--port=") || w[0].starts_with("-p=")
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
#[allow(dead_code)]
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

/// Describe what typically occupies well-known ports and how to free them.
pub fn describe_port_conflict(port: u16) -> Option<String> {
    match port {
        80 => {
            #[cfg(target_os = "macos")]
            {
                Some(
                    "Port 80 is likely used by AirPlay Receiver (macOS Monterey+).\n\
                     \n  To free it:\n\
                     \x20  1. Open System Settings → General → AirDrop & Handoff\n\
                     \x20  2. Turn off 'AirPlay Receiver'\n\
                     \n  Or run: sudo antra proxy start"
                        .to_string(),
                )
            }
            #[cfg(not(target_os = "macos"))]
            {
                Some(
                    "Port 80 is used by another service (Apache, nginx, etc.).\n\
                     \n  To free it:\n\
                     \x20  • Stop the service using: sudo lsof -i :80\n\
                     \x20  • Or run: sudo antra proxy start"
                        .to_string(),
                )
            }
        }
        443 => {
            #[cfg(target_os = "macos")]
            {
                Some(
                    "Port 443 is likely used by AirPlay Receiver (macOS Monterey+).\n\
                     \n  To free it:\n\
                     \x20  1. Open System Settings → General → AirDrop & Handoff\n\
                     \x20  2. Turn off 'AirPlay Receiver'\n\
                     \n  Or run: sudo antra proxy start"
                        .to_string(),
                )
            }
            #[cfg(not(target_os = "macos"))]
            {
                Some(
                    "Port 443 is used by another service.\n\
                     \n  To free it:\n\
                     \x20  • Stop the service using: sudo lsof -i :443\n\
                     \x20  • Or run: sudo antra proxy start"
                        .to_string(),
                )
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_port_available_roundtrip() {
        // Ephemeral ports are contended under parallel test load: another
        // test/process may grab our released port in the check window
        // (TOCTOU). Retry the whole hold/release cycle a few times before
        // calling it a failure.
        for attempt in 1..=10 {
            // Grab a free port, hold it, release it.
            let held = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = held.local_addr().unwrap().port();
            assert!(
                !is_port_available(port),
                "held port {port} must report unavailable"
            );
            drop(held);
            if is_port_available(port) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            if attempt == 10 {
                panic!("released port {port} still unavailable after 10 attempts (port snatched by parallel load?)");
            }
        }
    }

    /// The error a `::1` socket gets on a kernel with no IPv6 at all.
    fn no_ipv6() -> std::io::Error {
        #[cfg(unix)]
        let code = libc::EAFNOSUPPORT;
        #[cfg(windows)]
        let code = 10047; // WSAEAFNOSUPPORT
        std::io::Error::from_raw_os_error(code)
    }

    fn kind(k: std::io::ErrorKind) -> std::io::Error {
        std::io::Error::from(k)
    }

    #[test]
    fn missing_ipv6_is_told_apart_from_a_busy_port() {
        assert!(ipv6_loopback_unavailable(&no_ipv6()));
        assert!(ipv6_loopback_unavailable(&kind(
            std::io::ErrorKind::AddrNotAvailable
        )));
        for k in [
            std::io::ErrorKind::AddrInUse,
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::ConnectionRefused,
        ] {
            assert!(
                !ipv6_loopback_unavailable(&kind(k)),
                "{k:?} must not read as missing IPv6"
            );
        }
    }

    #[test]
    fn loopback_binds_keeps_both_stacks_when_both_bind() {
        assert_eq!(loopback_binds(Ok(4), Ok(6)).unwrap(), vec![4, 6]);
    }

    /// ROADMAP C15: on a host without IPv6 loopback the daemon must still
    /// bind 127.0.0.1, instead of reading every port as taken.
    #[test]
    fn loopback_binds_drops_ipv6_only_when_the_host_has_none() {
        assert_eq!(loopback_binds(Ok(4), Err(no_ipv6())).unwrap(), vec![4]);
        assert_eq!(
            loopback_binds(Ok(4), Err(kind(std::io::ErrorKind::AddrNotAvailable))).unwrap(),
            vec![4]
        );
    }

    /// The reason `::1` is bound at all: another server there would catch
    /// `localhost`. A taken or forbidden `::1` must stay an error.
    #[test]
    fn loopback_binds_fails_when_ipv6_is_taken_or_forbidden() {
        for k in [
            std::io::ErrorKind::AddrInUse,
            std::io::ErrorKind::PermissionDenied,
        ] {
            let err = loopback_binds(Ok(4), Err(kind(k))).unwrap_err();
            assert_eq!(err.kind(), k);
        }
    }

    #[test]
    fn loopback_binds_always_needs_ipv4() {
        let err =
            loopback_binds::<i32>(Err(kind(std::io::ErrorKind::AddrInUse)), Ok(6)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
        assert!(loopback_binds::<i32>(Err(no_ipv6()), Ok(6)).is_err());
    }

    #[test]
    fn test_find_free_port_is_available() {
        // Same TOCTOU note as above: retry before failing.
        for attempt in 1..=10 {
            let port = find_free_port().unwrap();
            if is_port_available(port) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            if attempt == 10 {
                panic!("find_free_port returned {port}, still unavailable after 10 attempts");
            }
        }
    }

    #[test]
    fn script_port_space_separated_flag() {
        assert_eq!(detect_port_from_script("vite --port 3001"), Some(3001));
    }

    #[test]
    fn script_port_equals_syntax() {
        assert_eq!(detect_port_from_script("vite --port=3001"), Some(3001));
    }

    #[test]
    fn script_port_short_flag() {
        assert_eq!(detect_port_from_script("next dev -p 3001"), Some(3001));
    }

    #[test]
    fn script_port_quoted_value() {
        // The whole script is one JSON string; the shell quoting has to
        // survive the trip or a quoted port reads as part of a word.
        assert_eq!(
            detect_port_from_script("vite --host '0.0.0.0' --port 3001"),
            Some(3001)
        );
    }

    #[test]
    fn script_without_port_yields_none() {
        assert_eq!(detect_port_from_script("vite"), None);
        assert_eq!(detect_port_from_script("next dev"), None);
        assert_eq!(detect_port_from_script(""), None);
    }

    #[test]
    fn unparseable_script_port_yields_none() {
        // Only the shell can resolve these. Guessing a port here would
        // route the domain somewhere the server never listens, which is
        // the exact failure this function exists to prevent.
        assert_eq!(detect_port_from_script("vite --port $PORT"), None);
        assert_eq!(detect_port_from_script("vite --port=${PORT:-3000}"), None);
        assert_eq!(detect_port_from_script("vite --port"), None);
        assert_eq!(detect_port_from_script("vite --port=notaport"), None);
        // A script Antra cannot make sense of must not error, and must not
        // invent a port either.
        assert_eq!(detect_port_from_script("&& || ;;"), None);
        assert_eq!(detect_port_from_script("node -e \"console.log(1)"), None);
    }

    #[test]
    fn script_port_beats_bare_number_but_not_absent_flag() {
        // A bare trailing number is not a port pin (`webpack-dev-server
        // . --port` vs `echo 3001`), so only an explicit flag counts.
        assert_eq!(detect_port_from_script("vite 3001"), None);
    }
}
