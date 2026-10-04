//! Asking once to serve on port 443 (ROADMAP C27).
//!
//! Without root the daemon cannot bind 443 on macOS or Linux and falls back
//! to 8443, so a first run printed `https://app.localhost:8443` while every
//! page about Antra shows the URL with no port. portless runs `sudo` on its
//! own; Antra asks first, remembers the answer, and the daemon it starts
//! gives up root as soon as the ports are open (`platform::sudo`).

use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::process::{Command, Stdio};

use colored::Colorize;

use crate::config::global;
use crate::util::output;

/// Whether this user can open 127.0.0.1:443 right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bind443 {
    /// It can (root, Windows, a lowered `ip_unprivileged_port_start`).
    Allowed,
    /// Only an admin can (`EACCES`) — the one case `sudo` fixes.
    Forbidden,
    /// Something else holds it. `sudo` would not help.
    Unavailable,
}

#[derive(Debug, PartialEq, Eq)]
enum Offer {
    Skip,
    Ask,
    Use { interactive: bool },
}

/// The whole decision, pure so every branch is tested.
fn decide(
    ports_chosen: bool,
    bind: Bind443,
    has_sudo: bool,
    saved: Option<bool>,
    interactive: bool,
) -> Offer {
    if ports_chosen || bind != Bind443::Forbidden || !has_sudo {
        return Offer::Skip;
    }
    match saved {
        Some(false) => Offer::Skip,
        Some(true) => Offer::Use { interactive },
        None if interactive => Offer::Ask,
        None => Offer::Skip,
    }
}

fn probe_443() -> Bind443 {
    match std::net::TcpListener::bind(("127.0.0.1", 443)) {
        Ok(_) => Bind443::Allowed,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Bind443::Forbidden,
        Err(_) => Bind443::Unavailable,
    }
}

/// `ANTRA_PORT` / `ANTRA_HTTP_PORT` mean the user picked the ports.
fn ports_chosen() -> bool {
    ["ANTRA_PORT", "ANTRA_HTTP_PORT"]
        .iter()
        .any(|v| std::env::var_os(v).is_some_and(|s| !s.is_empty()))
}

fn has_sudo() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("sudo").is_file()))
}

/// Offer port 443 before the daemon is started for the first time. Returns
/// true when a daemon is now running — on 443 — and the caller is done.
pub(crate) fn maybe_start_on_443() -> bool {
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let offer = decide(
        ports_chosen(),
        probe_443(),
        has_sudo(),
        global::port_443_answer(),
        interactive,
    );
    let interactive = match offer {
        Offer::Skip => return false,
        Offer::Use { interactive } => {
            println!(
                "  {} Starting the proxy on port 443 (asks for your password)...",
                "▸".cyan()
            );
            interactive
        }
        Offer::Ask => match ask() {
            Some(true) => {
                let _ = global::remember_port_443(true);
                true
            }
            Some(false) => {
                // The 8443 note printed with the URL says how to switch later.
                let _ = global::remember_port_443(false);
                println!(
                    "  {} Using port 8443. Antra will not ask again.",
                    "ℹ".cyan()
                );
                return false;
            }
            None => return false,
        },
    };

    match start_with_sudo(interactive) {
        Ok(()) => {
            output::print_success("Proxy started on port 443 (admin rights dropped)");
            true
        }
        Err(reason) => {
            output::print_warning(&format!("Could not start on port 443: {reason}"));
            output::print_warning("Using port 8443 instead.");
            false
        }
    }
}

fn ask() -> Option<bool> {
    println!();
    println!(
        "  Antra can give you {} with no port number.",
        "https://<app>.localhost".bold()
    );
    println!("  Port 443 needs admin rights, so this asks for your password (sudo).");
    println!("  The proxy drops admin rights as soon as the port is open, and stays");
    println!("  up until you restart, so you are asked once.");
    println!();
    let stdin = std::io::stdin();
    let mut input = String::new();
    loop {
        print!("  {} ", "Use port 443? [Y/n]".yellow());
        std::io::stdout().flush().ok()?;
        input.clear();
        match stdin.read_line(&mut input) {
            Ok(0) | Err(_) => {
                println!();
                return None;
            }
            Ok(_) => match super::run::parse_trust_answer(&input) {
                Some(super::run::TrustAnswer::Yes) => return Some(true),
                Some(super::run::TrustAnswer::No) => return Some(false),
                None => println!("  Please answer y, yes, n, or no."),
            },
        }
    }
}

/// The path variables this CLI resolves its socket, CA and log from. `sudo`
/// on Linux resets or drops them, so they are passed through `env` after it,
/// and the daemon finds exactly what this CLI will look for. An unset XDG
/// one is passed empty: `dirs` ignores an empty value, while leaving it out
/// would let `platform::sudo` fill in a default this CLI is not using.
const PATH_VARS: [&str; 4] = [
    "HOME",
    "XDG_RUNTIME_DIR",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
];

fn start_with_sudo(interactive: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find antra itself: {e}"))?;
    let mut cmd = Command::new("sudo");
    if !interactive {
        // Never wait on a password nobody can type.
        cmd.arg("-n");
    }
    cmd.arg("--").arg("/usr/bin/env");
    for var in PATH_VARS {
        let value = std::env::var_os(var);
        if var == "HOME" && value.is_none() {
            continue;
        }
        let mut pair = OsString::from(var);
        pair.push("=");
        pair.push(value.unwrap_or_default());
        cmd.arg(pair);
    }
    cmd.arg(exe)
        .args(["proxy", "start", "--port", "443", "--http-port", "80"])
        // sudo asks on the terminal itself; only `proxy start`'s own report
        // is captured, and shown if it fails.
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let out = cmd
        .output()
        .map_err(|e| format!("could not run sudo: {e}"))?;
    if !out.status.success() {
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let last = text.lines().rev().map(str::trim).find(|l| !l.is_empty());
        return Err(match (last, interactive) {
            (Some(line), _) => line.to_string(),
            (None, false) => "sudo needs a password and there is no terminal to ask in".into(),
            (None, true) => "sudo did not run it".into(),
        });
    }
    match crate::ipc::client::get_startup_status() {
        Ok(status) if status.https_port == 443 => Ok(()),
        Ok(status) => Err(format!(
            "the proxy came up on {} instead — see `antra logs`",
            status.https_port
        )),
        Err(e) => Err(format!("the proxy did not answer: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORBIDDEN: Bind443 = Bind443::Forbidden;

    #[test]
    fn a_first_run_that_needs_sudo_asks() {
        assert_eq!(decide(false, FORBIDDEN, true, None, true), Offer::Ask);
    }

    #[test]
    fn the_saved_answer_is_used_without_asking_again() {
        assert_eq!(
            decide(false, FORBIDDEN, true, Some(true), true),
            Offer::Use { interactive: true }
        );
        assert_eq!(
            decide(false, FORBIDDEN, true, Some(false), true),
            Offer::Skip
        );
    }

    #[test]
    fn no_terminal_never_asks_and_never_waits_on_a_password() {
        assert_eq!(decide(false, FORBIDDEN, true, None, false), Offer::Skip);
        // A saved yes still tries, but as `sudo -n`.
        assert_eq!(
            decide(false, FORBIDDEN, true, Some(true), false),
            Offer::Use { interactive: false }
        );
    }

    #[test]
    fn nothing_is_offered_when_sudo_would_not_help() {
        // Already allowed: root, Windows, a lowered unprivileged port start.
        assert_eq!(
            decide(false, Bind443::Allowed, true, None, true),
            Offer::Skip
        );
        // Taken by something else: root would get the same answer.
        assert_eq!(
            decide(false, Bind443::Unavailable, true, None, true),
            Offer::Skip
        );
        assert_eq!(
            decide(false, Bind443::Unavailable, true, Some(true), true),
            Offer::Skip
        );
    }

    #[test]
    fn chosen_ports_or_no_sudo_mean_no_offer() {
        assert_eq!(decide(true, FORBIDDEN, true, None, true), Offer::Skip);
        assert_eq!(decide(true, FORBIDDEN, true, Some(true), true), Offer::Skip);
        assert_eq!(decide(false, FORBIDDEN, false, None, true), Offer::Skip);
    }
}
