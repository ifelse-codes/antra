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
        // Flask's CLI ignores $PORT (it reads FLASK_RUN_PORT or --port), so
        // without this `antra dev --port 5001` routed to 5001 while Flask
        // still bound 5000.
        "flask" if rest.first().is_some_and(|s| s == "run") => true,
        "python" | "python3" if rest.starts_with(&["-m".into(), "flask".into(), "run".into()]) => {
            true
        }
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

/// A process listening on a port, as `lsof` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortHolder {
    pub pid: u32,
    pub name: String,
}

/// Who is listening on `port`, if `lsof` can tell.
///
/// Best-effort: `None` when `lsof` is missing, finds nothing, or is not
/// allowed to see another user's process. `+c 0` asks for the full command
/// name — macOS truncates it to nine characters otherwise, which is how
/// Control Center shows up as `ControlCe`.
#[cfg(unix)]
pub fn port_holder(port: u16) -> Option<PortHolder> {
    let out = std::process::Command::new("lsof")
        .args(["+c", "0", "-nP", "-sTCP:LISTEN", "-Fpc"])
        .arg(format!("-iTCP:{port}"))
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    parse_lsof_holder(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(unix))]
pub fn port_holder(_port: u16) -> Option<PortHolder> {
    None
}

/// Parse `lsof -F pc` output: a `p<pid>` line, then `c<command>` (and
/// other field lines) for each process. Takes the first complete pair.
/// Unused on Windows, which has no `lsof`; tested everywhere.
#[cfg_attr(not(unix), allow(dead_code))]
pub fn parse_lsof_holder(out: &str) -> Option<PortHolder> {
    let mut pid = None;
    for line in out.lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p.trim().parse().ok();
        } else if let (Some(name), Some(pid)) = (line.strip_prefix('c'), pid) {
            let name = name.trim();
            if !name.is_empty() {
                return Some(PortHolder {
                    pid,
                    name: name.to_string(),
                });
            }
        }
    }
    None
}

/// TCP ports that processes in process group `pgid` are listening on.
///
/// `antra run` starts its child as the leader of a new process group, so
/// this covers the whole tree the child starts (`npm` → `node`). Used when
/// the route's port and the server's port disagree: a server with a
/// hardcoded `listen(3000)` ignores the `PORT` Antra injected, and the only
/// way to know where it went is to ask the OS. Best-effort and sorted: an
/// empty list means "could not tell", never "nothing listens".
#[cfg(target_os = "linux")]
pub fn group_listening_ports(pgid: u32) -> Vec<u16> {
    let mut listeners = std::collections::HashMap::new();
    for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(text) = std::fs::read_to_string(table) {
            listeners.extend(parse_proc_net_tcp_listeners(&text));
        }
    }
    let mut ports = Vec::new();
    if listeners.is_empty() {
        return ports;
    }
    let Ok(procs) = std::fs::read_dir("/proc") else {
        return ports;
    };
    for entry in procs.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let in_group = std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| parse_proc_stat_pgrp(&stat))
            == Some(pgid);
        if !in_group {
            continue;
        }
        let Ok(fds) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let Ok(target) = std::fs::read_link(fd.path()) else {
                continue;
            };
            let inode = target
                .to_str()
                .and_then(|t| t.strip_prefix("socket:["))
                .and_then(|t| t.strip_suffix(']'))
                .and_then(|t| t.parse::<u64>().ok());
            if let Some(port) = inode.and_then(|i| listeners.get(&i)) {
                ports.push(*port);
            }
        }
    }
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// macOS and the BSDs: `lsof`, selected by process group (`-g`) and ANDed
/// (`-a`) with listening TCP sockets. Without `-a`, lsof ORs its selectors
/// and would list every TCP listener on the machine.
#[cfg(all(unix, not(target_os = "linux")))]
pub fn group_listening_ports(pgid: u32) -> Vec<u16> {
    let Ok(out) = std::process::Command::new("lsof")
        .args(["-nP", "-a", "-g"])
        .arg(pgid.to_string())
        .args(["-iTCP", "-sTCP:LISTEN", "-Fn"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    parse_lsof_listen_ports(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(unix))]
pub fn group_listening_ports(_pgid: u32) -> Vec<u16> {
    Vec::new()
}

/// Listening sockets in a `/proc/net/tcp` or `tcp6` table, as inode → port.
///
/// A row is `sl local_address rem_address st … uid timeout inode`, with the
/// address as `HEX_IP:HEX_PORT` and state `0A` meaning `LISTEN`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_proc_net_tcp_listeners(text: &str) -> Vec<(u64, u16)> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 || fields[3] != "0A" {
                return None;
            }
            let port = u16::from_str_radix(fields[1].rsplit(':').next()?, 16).ok()?;
            let inode = fields[9].parse::<u64>().ok()?;
            (inode != 0).then_some((inode, port))
        })
        .collect()
}

/// The process group in a `/proc/<pid>/stat` line.
///
/// The command name sits in parentheses and may itself contain spaces or
/// `)`, so fields are counted from the *last* `)`: state, ppid, pgrp.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_proc_stat_pgrp(stat: &str) -> Option<u32> {
    let (_, rest) = stat.rsplit_once(')')?;
    rest.split_whitespace().nth(2)?.parse().ok()
}

/// Ports in `lsof -F n` output: name lines like `n*:3000`,
/// `n127.0.0.1:3000` or `n[::1]:3000`. Sorted, without duplicates — a
/// server bound on both stacks shows up twice.
#[cfg_attr(any(target_os = "linux", not(unix)), allow(dead_code))]
pub fn parse_lsof_listen_ports(out: &str) -> Vec<u16> {
    let mut ports: Vec<u16> = out
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .filter_map(|name| name.rsplit(':').next()?.parse().ok())
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// Why a port a user asked for is taken, and what to do instead.
///
/// Pure, so each case is tested: the advice this replaced was wrong in the
/// most common case of all. A first `antra dev` on a Flask app (default
/// port 5000) on a Mac hits macOS Control Center, which holds 5000 for
/// AirPlay Receiver — and was told to "stop the process on that port",
/// which no user can, and asked whether they meant `antra alias`, which
/// would have routed their domain to AirPlay.
///
/// * `serving`: something accepts connections on the port, so it may be
///   the user's own app, already running — the one case `antra alias` fits.
/// * `pinned`: the user's own command names the port, so `--port` alone
///   cannot move it; the command has to change.
/// * `free`: a nearby port that is free now, to suggest concretely.
pub fn port_conflict_advice(
    port: u16,
    holder: Option<&PortHolder>,
    serving: bool,
    pinned: bool,
    domain: &str,
    free: Option<u16>,
    macos: bool,
) -> Vec<String> {
    let elsewhere = match (pinned, free) {
        (true, Some(f)) => format!("change the port in your command, e.g. to {f}"),
        (true, None) => "change the port in your command".to_string(),
        (false, Some(f)) => format!("run on another port: --port {f}"),
        (false, None) => "pass a free --port".to_string(),
    };

    // macOS Monterey and later: Control Center listens on 5000 and 7000 for
    // AirPlay Receiver. Not a process anyone should kill, and never the
    // user's app.
    let airplay = macos
        && (port == 5000 || port == 7000)
        && holder.is_some_and(|h| h.name.starts_with("ControlCe"));
    if airplay {
        return vec![
            format!("Port {port} is held by macOS AirPlay Receiver (Control Center)."),
            "Turn it off in System Settings → General → AirDrop & Handoff → AirPlay Receiver,"
                .to_string(),
            format!("or {elsewhere}"),
        ];
    }

    let mut advice = Vec::new();
    match holder {
        Some(h) => advice.push(format!(
            "Port {port} is in use by {} (PID {}).",
            h.name, h.pid
        )),
        None if serving => advice.push(format!("Something is already serving on port {port}.")),
        None => {}
    }
    if serving && !pinned {
        advice.push(format!(
            "If that is your app, already running, front it instead: antra alias {domain} {port}"
        ));
    }
    // No closing full stop: the line can end in a flag to copy.
    advice.push(format!("Otherwise stop it, or {elsewhere}"));
    advice
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

    fn holder(pid: u32, name: &str) -> PortHolder {
        PortHolder {
            pid,
            name: name.to_string(),
        }
    }

    #[test]
    fn lsof_field_output_names_the_first_listener() {
        // `lsof +c 0 -Fpc` on macOS with AirPlay Receiver on.
        let out = "p412\ncControlCenter\nf9\nf10\n";
        assert_eq!(parse_lsof_holder(out), Some(holder(412, "ControlCenter")));
        assert_eq!(parse_lsof_holder(""), None);
        assert_eq!(
            parse_lsof_holder("p12\n"),
            None,
            "a pid with no command is no holder"
        );
    }

    /// The case the old advice got wrong: Flask's default port on a Mac.
    #[test]
    fn airplay_on_5000_is_named_and_never_offered_to_alias() {
        let airplay = holder(412, "ControlCenter");
        let advice = port_conflict_advice(
            5000,
            Some(&airplay),
            true,
            false,
            "demo.localhost",
            Some(5001),
            true,
        );
        let text = advice.join("\n");
        assert!(text.contains("macOS AirPlay Receiver"), "{text}");
        assert!(text.contains("AirDrop & Handoff"), "{text}");
        assert!(text.ends_with("--port 5001"), "{text}");
        assert!(!text.contains("antra alias"), "{text}");
        assert!(!text.contains("stop it"), "{text}");
    }

    #[test]
    fn airplay_is_recognised_by_its_truncated_lsof_name_too() {
        let text = port_conflict_advice(
            7000,
            Some(&holder(412, "ControlCe")),
            true,
            false,
            "d.localhost",
            None,
            true,
        )
        .join("\n");
        assert!(text.contains("AirPlay Receiver"), "{text}");
        assert!(text.ends_with("pass a free --port"), "{text}");
    }

    /// Control Center is only AirPlay on 5000/7000, and only on macOS.
    #[test]
    fn control_center_elsewhere_gets_the_generic_advice() {
        for (port, macos) in [(8080, true), (5000, false)] {
            let text = port_conflict_advice(
                port,
                Some(&holder(412, "ControlCenter")),
                true,
                false,
                "d.localhost",
                None,
                macos,
            )
            .join("\n");
            assert!(
                !text.contains("AirPlay"),
                "port {port}, macos {macos}: {text}"
            );
            assert!(text.contains("in use by ControlCenter (PID 412)"), "{text}");
        }
    }

    #[test]
    fn a_serving_app_is_named_and_offered_alias_with_its_domain() {
        let advice = port_conflict_advice(
            5000,
            Some(&holder(2312, "python3")),
            true,
            false,
            "demo-flask.localhost",
            Some(5001),
            false,
        );
        assert_eq!(
            advice,
            vec![
                "Port 5000 is in use by python3 (PID 2312).".to_string(),
                "If that is your app, already running, front it instead: antra alias demo-flask.localhost 5000".to_string(),
                "Otherwise stop it, or run on another port: --port 5001".to_string(),
            ]
        );
    }

    /// `--port` cannot move a command that names its own port.
    #[test]
    fn a_pinned_command_is_told_to_change_its_command() {
        let text = port_conflict_advice(
            18090,
            Some(&holder(9, "python3")),
            true,
            true,
            "d.localhost",
            Some(18091),
            false,
        )
        .join("\n");
        assert!(!text.contains("antra alias"), "{text}");
        assert!(
            text.contains("change the port in your command, e.g. to 18091"),
            "{text}"
        );
    }

    #[test]
    fn nothing_known_and_nothing_serving_still_says_what_to_do() {
        assert_eq!(
            port_conflict_advice(5000, None, false, false, "d.localhost", None, false),
            vec!["Otherwise stop it, or pass a free --port".to_string()]
        );
    }

    /// Asks the real `lsof` about a port this test holds.
    #[cfg(unix)]
    #[test]
    fn port_holder_finds_this_process() {
        if std::process::Command::new("lsof")
            .arg("-v")
            .output()
            .is_err()
        {
            eprintln!("skipping: lsof not installed");
            return;
        }
        let held = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = held.local_addr().unwrap().port();
        let found = port_holder(port).expect("lsof should see our own listener");
        assert_eq!(found.pid, std::process::id());
        assert!(!found.name.is_empty());
    }

    #[test]
    fn flask_run_gets_the_port_it_is_routed_to() {
        let cmd = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            inject_port_flag(&cmd(&["flask", "run"]), 5001),
            cmd(&["flask", "run", "--port", "5001"])
        );
        assert_eq!(
            inject_port_flag(&cmd(&["python3", "-m", "flask", "run"]), 5001),
            cmd(&["python3", "-m", "flask", "run", "--port", "5001"])
        );
        // An explicit port is the user's; other flask commands are not servers.
        assert_eq!(
            inject_port_flag(&cmd(&["flask", "run", "-p", "6000"]), 5001),
            cmd(&["flask", "run", "-p", "6000"])
        );
        assert_eq!(
            inject_port_flag(&cmd(&["flask", "db", "upgrade"]), 5001),
            cmd(&["flask", "db", "upgrade"])
        );
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

    #[test]
    fn proc_net_tcp_keeps_only_listening_rows() {
        // Real rows, trimmed: a LISTEN on 3000 (0x0BB8), an ESTABLISHED
        // connection (01) that must not count, and a LISTEN with inode 0
        // (a socket in TIME_WAIT-like limbo with no owner to match).
        let table = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
           0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 41234 1 0000000000000000 100 0 0 10 0\n\
           1: 0100007F:0FA0 0100007F:C350 01 00000000:00000000 00:00000000 00000000  1000        0 41235 1 0000000000000000 20 4 30 10 -1\n\
           2: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 0 1 0000000000000000 100 0 0 10 0\n";
        assert_eq!(parse_proc_net_tcp_listeners(table), vec![(41234, 3000)]);
    }

    #[test]
    fn proc_net_tcp6_rows_parse_the_same_way() {
        let table = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
           0: 00000000000000000000000001000000:0BB8 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 52000 1 0000000000000000 100 0 0 10 0\n";
        assert_eq!(parse_proc_net_tcp_listeners(table), vec![(52000, 3000)]);
    }

    #[test]
    fn proc_stat_pgrp_survives_a_command_name_with_spaces_and_parens() {
        assert_eq!(
            parse_proc_stat_pgrp("3853 (cat) R 3849 3853 3849 0 -1 4194304"),
            Some(3853)
        );
        // `comm` is free text: counting fields from the first `)` would
        // read the ppid of a process named `a) b` as its pgrp.
        assert_eq!(
            parse_proc_stat_pgrp("77 (node a) b) S 70 75 70 0 -1"),
            Some(75)
        );
        assert_eq!(parse_proc_stat_pgrp("garbage"), None);
    }

    #[test]
    fn lsof_listen_ports_cover_both_stacks_once() {
        let out = "p4242\nf22\nn*:3000\nf23\nn[::1]:3000\nf24\nn127.0.0.1:9229\n";
        assert_eq!(parse_lsof_listen_ports(out), vec![3000, 9229]);
        assert_eq!(parse_lsof_listen_ports(""), Vec::<u16>::new());
    }

    /// Asks the real OS which ports this test's own process group listens
    /// on, while it holds one. Linux reads `/proc`; macOS asks `lsof`.
    #[cfg(unix)]
    #[test]
    fn group_listening_ports_finds_a_port_this_group_holds() {
        #[cfg(not(target_os = "linux"))]
        if std::process::Command::new("lsof")
            .arg("-v")
            .output()
            .is_err()
        {
            eprintln!("skipping: lsof not installed");
            return;
        }
        let held = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = held.local_addr().unwrap().port();
        let pgid = nix::unistd::getpgrp().as_raw() as u32;
        let ports = group_listening_ports(pgid);
        assert!(
            ports.contains(&port),
            "group {pgid} holds {port}, but the OS reported {ports:?}"
        );
        drop(held);
        // Another test may spawn a child (`lsof`, `true`) while `held` is
        // open. Until that child execs, it holds a copy of the socket inside
        // this process group, so the group really does still hold the port
        // for those milliseconds. A single check failed ~8% of runs under
        // load (and on a GitHub runner); alone it never did. Give the child
        // time to exec — a port that stays reported still fails.
        let released = (0..50).any(|_| {
            if !group_listening_ports(pgid).contains(&port) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            false
        });
        assert!(released, "a released port must not be reported");
    }
}
