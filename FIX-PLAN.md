# Antra — Fix Plan (Deliver the Promise)

> The landing page promises: **"One command. Real HTTPS. No ports. No /etc/hosts. No certificate warnings."**
>
> This plan makes the app **actually deliver that**. No disclaimers. No "well actually you need to...".
> Every fix makes the app match the promise, not the other way around.

---

## Table of Contents

1. [P0 — `antra run` auto-trusts the CA on first run](#1-p0--antra-run-auto-trusts-the-ca-on-first-run)
2. [P0 — Auto-fallback when privileged ports are unavailable](#2-p0--auto-fallback-when-privileged-ports-are-unavailable)
3. [P0 — Daemon reports real bind status, not fake success](#3-p0--daemon-reports-real-bind-status-not-fake-success)
4. [P0 — 503 errors tell you exactly what's wrong](#4-p0--503-errors-tell-you-exactly-whats-wrong)
5. [P1 — `--yes` flag on trust, clean, and run](#5-p1---yes-flag-on-trust-clean-and-run)
6. [P1 — `antra doctor` detects its own daemon](#6-p1--antra-doctor-detects-its-own-daemon)
7. [P2 — Fix `proxy stop` test race condition](#7-p2--fix-proxy-stop-test-race-condition)
8. [P2 — Install script TTY portability](#8-p2--install-script-tty-portability)

---

## 1. P0 — `antra run` auto-trusts the CA on first run

**The promise:** "One command and you're there."
**The reality:** `antra run` shows an interactive `[y/N]` prompt for CA trust, or silently skips if non-TTY.

**Fix:** When `antra run` starts and the CA isn't trusted yet, **automatically install it** (with sudo if needed). No prompt. The user typed `antra run` — they want it to work.

### File: `src/cli/run.rs`

**1a.** In `maybe_prompt_trust()` (lines 58-131), change the flow:

```rust
// BEFORE: prompts user, skips if non-TTY
async fn maybe_prompt_trust(no_trust_prompt: bool) {
    if no_trust_prompt { return; }
    if global::was_trust_prompted() { return; }
    // ... interactive prompt ...
}

// AFTER: auto-install, no prompt
async fn maybe_prompt_trust(no_trust_prompt: bool) {
    if no_trust_prompt { return; }
    if global::was_trust_prompted() { return; }

    // Check if CA is already trusted
    if trust::check_trust_status().unwrap_or(false) {
        global::mark_trust_prompted();
        return;
    }

    // CA not trusted — auto-install it
    println!();
    println!("  {} Setting up HTTPS (one-time)...", "▸".cyan());
    match trust::install_ca_noninteractive() {
        Ok(()) => {
            println!("  {} CA installed — HTTPS ready", "✓".green().bold());
            global::mark_trust_prompted();
        }
        Err(e) => {
            // If auto-install fails (e.g. no sudo), fall back gracefully
            println!("  {} Auto-trust failed: {e}", "⚠".yellow());
            println!("  Run {} to install manually, or use {}", "antra trust".bold(), "--no-trust-prompt".bold());
            // Don't block — let the user continue, they'll get cert warnings but it works
        }
    }
}
```

### File: `src/trust.rs`

**1b.** Add a non-interactive trust installation function:

```rust
pub fn install_ca_noninteractive() -> Result<()> {
    // 1. Generate CA if not exists
    let ca = certs::ca::get_or_create_ca()?;

    // 2. Install into system trust store without prompt
    #[cfg(target_os = "macos")]
    {
        // Write CA to a temp file, then use `security add-trusted-cert` with sudo
        let temp_cert = tempfile::NamedTempFile::new()?;
        std::fs::write(temp_cert.path(), ca.pem.as_bytes())?;

        let status = std::process::Command::new("sudo")
            .args([
                "security", "add-trusted-cert",
                "-d", "-r", "trustRoot",
                "-k", "/Library/Keychains/System.keychain",
                temp_cert.path().to_str().unwrap(),
            ])
            .status()?;

        if !status.success() {
            anyhow::bail!("sudo security add-trusted-cert failed");
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Copy CA to /usr/local/share/ca-certificates/ and run update-ca-certificates
        let cert_path = "/usr/local/share/ca-certificates/antra-ca.crt";
        std::fs::write(cert_path, ca.pem.as_bytes())?;

        let status = std::process::Command::new("sudo")
            .args(["update-ca-certificates"])
            .status()?;

        if !status.success() {
            anyhow::bail!("sudo update-ca-certificates failed");
        }
    }

    Ok(())
}
```

**1c.** Add `check_trust_status()` that returns a bool (not just prints):

```rust
pub fn check_trust_status() -> Result<bool> {
    // Reuse existing trust check logic from trust::check_status()
    // but return a bool instead of printing
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("security")
            .args(["find-certificate", "-c", "Antra Local CA", "-a"])
            .output()?;
        Ok(output.status.success())
    }
    #[cfg(target_os = "linux")]
    {
        // Check if our CA file exists in the trust store
        Ok(std::path::Path::new("/usr/local/share/ca-certificates/antra-ca.crt").exists())
    }
}
```

---

## 2. P0 — Auto-fallback when privileged ports are unavailable

**The promise:** "No ports." (meaning no `:5173`, no `:3000` — clean URLs)
**The reality:** `antra proxy start` silently fails on port 443/80 (needs root), user gets no feedback.

**Fix:** When ports 443/80 fail, **automatically try 8443/8080** and tell the user. The clean URLs still work — just on high ports that don't need root.

### File: `src/daemon/server.rs`

**2a.** Change the proxy startup to try ports with automatic fallback:

```rust
// BEFORE: just try the requested port, fail silently
let https_port = config.https_port;  // 443
tokio::spawn(async move {
    if let Err(e) = crate::proxy::https::start_server(https_port, ...).await {
        tracing::error!(error = %e, "HTTPS proxy server error");
    }
});

// AFTER: try preferred port, fallback to high port
async fn try_start_https(
    preferred_port: u16,
    registry: Arc<RouteRegistry>,
    cert_cache: Arc<CertCache>,
) -> (u16, Result<(), String>) {
    // Try preferred port first
    match crate::proxy::https::start_server(preferred_port, Arc::clone(&registry), Arc::clone(&cert_cache)).await {
        Ok(()) => (preferred_port, Ok(())),
        Err(e) => {
            tracing::warn!(error = %e, port = preferred_port, "Preferred port failed, trying fallback");
            let fallback_port = match preferred_port {
                443 => 8443,
                p => p + 1000,
            };
            match crate::proxy::https::start_server(fallback_port, registry, cert_cache).await {
                Ok(()) => (fallback_port, Ok(())),
                Err(e2) => (fallback_port, Err(format!("Both port {preferred_port} and {fallback_port} failed: {e2}"))),
            }
        }
    }
}
```

Same pattern for `start_http_redirect` with 80 → 8080 fallback.

### File: `src/ipc/protocol.rs`

**2b.** Add a `StartupStatus` message that tells the parent which ports actually bound:

```rust
pub enum IpcMessage {
    // ... existing ...
    StartupStatus {
        https_port: u16,
        https_ok: bool,
        https_error: Option<String>,
        http_port: u16,
        http_ok: bool,
        http_error: Option<String>,
    },
}
```

### File: `src/cli/proxy.rs`

**2c.** After daemon starts, query the actual ports and report accurately:

```rust
// After daemon detected running:
match ipc::client::get_startup_status() {
    Ok(status) => {
        println!("  {} Daemon started (PID: {child_pid})", "✓".green().bold());
        if status.https_ok {
            println!("  {} HTTPS proxy on 127.0.0.1:{}", "✓".green().bold(), status.https_port);
        } else {
            println!("  {} HTTPS proxy on port {}: {}", "⚠".yellow().bold(), status.https_port,
                status.https_error.unwrap_or_default());
        }
        if status.http_ok {
            println!("  {} HTTP→HTTPS redirect on 127.0.0.1:{}", "✓".green().bold(), status.http_port);
        } else {
            println!("  {} HTTP redirect on port {}: {}", "⚠".yellow().bold(), status.http_port,
                status.http_error.unwrap_or_default());
        }
    }
    Err(_) => {
        // Fallback: can't query, print what we know
        println!("  {} Daemon started (PID: {child_pid})", "✓".green().bold());
    }
}
```

### File: `src/cli/run.rs`

**2d.** When `antra run` auto-starts the daemon, also auto-detect and report the actual ports:

```rust
// In the run command, after daemon starts:
// Print the actual URL the user should visit
if status.https_port != 443 {
    println!();
    println!("  {} Note: HTTPS on port {} (port 443 unavailable)", "ℹ".cyan(), status.https_port);
    println!("  Visit: https://{}.localhost:{}", domain, status.https_port);
} else {
    println!("  → https://{}.localhost", domain);
}
```

---

## 3. P0 — Daemon reports real bind status, not fake success

**The promise:** "One command and you're there."
**The reality:** `antra proxy start` says "✓ HTTPS proxy on 127.0.0.1:443" when nothing is listening.

**Fix:** The daemon must **verify ports are actually bound** before declaring success. The parent must **query bind status** before printing success.

### File: `src/daemon/server.rs`

**3a.** Replace fire-and-forget spawns with oneshot-channel handshake (same as Fix 2a but for the bind-confirmation pattern):

```rust
use tokio::sync::oneshot;

let (http_result_tx, http_result_rx) = oneshot::channel();
let http_port = config.http_port;
tokio::spawn(async move {
    let result = crate::proxy::https::start_http_redirect(http_port).await;
    let _ = http_result_tx.send(result.map_err(|e| e.to_string()));
});

let (https_result_tx, https_result_rx) = oneshot::channel();
let https_port = config.https_port;
let https_registry = Arc::clone(&registry);
let https_cert_cache = Arc::clone(&cert_cache);
tokio::spawn(async move {
    let result = crate::proxy::https::start_server(https_port, https_registry, https_cert_cache).await;
    let _ = https_result_tx.send(result.map_err(|e| e.to_string()));
});

// Wait for bind results (with timeout)
let http_bind_result = tokio::time::timeout(
    std::time::Duration::from_secs(3),
    http_result_rx
).await.unwrap_or(Ok(Err("timeout".into())));

let https_bind_result = tokio::time::timeout(
    std::time::Duration::from_secs(3),
    https_result_rx
).await.unwrap_or(Ok(Err("timeout".into())));

// Store results in shared state for IPC queries
let startup_status = Arc::new(Mutex::new(StartupStatus {
    https_port,
    https_ok: https_bind_result.as_ref().map(|r| r.is_ok()).unwrap_or(false),
    https_error: https_bind_result.as_ref().err().cloned().flatten(),
    http_port,
    http_ok: http_bind_result.as_ref().map(|r| r.is_ok()).unwrap_or(false),
    http_error: http_bind_result.as_ref().err().cloned().flatten(),
}));
```

**3b.** Handle `GetStartupStatus` IPC message by returning the cached status.

### File: `src/cli/proxy.rs`

**3c.** Remove `Stdio::null()` so daemon output is accessible for debugging:

```rust
// BEFORE:
.stdout(std::process::Stdio::null())
.stderr(std::process::Stdio::null())

// AFTER:
.stdout(std::process::Stdio::piped())
.stderr(std::process::Stdio::piped())
```

---

## 4. P0 — 503 errors tell you exactly what's wrong

**The promise:** Just works.
**The reality:** `503 Service Unavailable: Upstream connection failed: client error (Connect)` — useless.

**Fix:** Every 503 response must include: the domain, the upstream target, the specific error, and what to do about it.

### File: `src/proxy/http.rs` (lines 76-89)

**4a.** Enrich the 503 response body:

```rust
// BEFORE:
Err(e) => {
    tracing::error!(%domain, error = %e, "Upstream request failed");
    let response = Response::builder()
        .status(503)
        .header("content-type", "text/plain")
        .body(Either::Left(Full::new(Bytes::from(format!(
            "503 Service Unavailable: {e}"
        )))))
        .unwrap();
    Ok(response)
}

// AFTER:
Err(e) => {
    tracing::error!(%domain, error = %e, "Upstream request failed");
    let body = format!(
        "503 Service Unavailable\n\n\
         Domain:   {domain}\n\
         Upstream: {}:{}\n\
         Error:    {e}\n\n\
         Fix this:\n\
         1. Is your server running on port {}?\n\
         2. Start it: antra run --domain {domain} --port {} -- <your-command>\n\
         3. Check routes: antra list\n",
        route.host, route.port, route.port, route.port
    );
    let response = Response::builder()
        .status(503)
        .header("content-type", "text/plain")
        .body(Either::Left(Full::new(Bytes::from(body))))
        .unwrap();
    Ok(response)
}
```

### File: `src/proxy/forward.rs` (line 61)

**4b.** Include upstream address in the error:

```rust
// BEFORE:
.map_err(|e| anyhow::anyhow!("Upstream connection failed: {e}"))?;

// AFTER:
.map_err(|e| anyhow::anyhow!(
    "Connection to {}:{} refused — is your server running?", route.host, route.port
))?;
```

### File: `src/proxy/https.rs`

**4c.** Apply the same 503 enrichment to the HTTPS handler's error path. Find the equivalent error response and add the same domain/upstream/hint format.

---

## 5. P1 — `--yes` flag on trust, clean, and run

**Why:** Even with auto-trust (Fix 1), some users want explicit control. `--yes` also helps CI and scripting.

### File: `src/cli/trust.rs`

**5a.** Add `--yes` flag:

```rust
#[derive(Args)]
pub struct TrustArgs {
    #[arg(long)]
    pub status: bool,
    #[arg(long)]
    pub remove: bool,
    #[arg(short, long)]
    pub yes: bool,
}
```

Pass to trust functions:

```rust
if self.yes {
    trust::install_ca_noninteractive()?;
} else if self.remove {
    trust::remove_ca(self.yes)?;
} else {
    trust::install_ca(false)?;  // interactive
}
```

### File: `src/cli/clean.rs`

**5b.** Add `--yes` flag:

```rust
#[derive(Args)]
pub struct CleanArgs {
    #[arg(short, long)]
    pub yes: bool,
}
```

In `execute()`:

```rust
pub fn execute(args: CleanArgs) -> Result<()> {
    if !args.yes {
        // existing prompt
    }
    // cleanup
}
```

### File: `src/cli/mod.rs`

**5c.** Update the `Clean` variant:

```rust
Clean(clean::CleanArgs),
```

### File: `src/cli/run.rs`

**5d.** Add `-y` / `--yes` as a general skip-prompts flag:

```rust
#[derive(Args)]
pub struct RunArgs {
    // ... existing ...
    #[arg(short, long)]
    pub yes: bool,
}
```

In `maybe_prompt_trust()`:

```rust
if no_trust_prompt || yes {
    return;
}
```

---

## 6. P1 — `antra doctor` detects its own daemon

**The promise:** Smart diagnostics.
**The reality:** Doctor says "Port 443 in use" when the Antra daemon itself holds it.

**Fix:** When doctor detects a port in use, check if it's the Antra daemon. If so, report success, not a problem.

### File: `src/cli/doctor.rs` (lines 119-148)

```rust
// BEFORE:
Err(_) => {
    println!("  {} {}", "⚠".yellow().bold(),
        format!("Port {port} ({name}) in use").yellow());
    issues.push((...));
}

// AFTER:
Err(_) => {
    if is_antra_daemon_port(port) {
        println!("  {} {}",
            "✓".green().bold(),
            format!("Port {port} ({name}) — Antra daemon active").green());
    } else {
        println!("  {} {}",
            "⚠".yellow().bold(),
            format!("Port {port} ({name}) in use by another process").yellow());
        issues.push((
            format!("Port {port} ({name}) in use"),
            format!("antra proxy start --port {} --http-port {}", 8443, 8080),
        ));
    }
}
```

### Add helper function

```rust
fn is_antra_daemon_port(port: u16) -> bool {
    // Check if the Antra daemon PID is running
    let pid_path = crate::config::global::daemon_pid_path();
    if let Ok(pid) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = pid.trim().parse::<u32>() {
            // Check if process is alive
            #[cfg(unix)]
            {
                use nix::sys::signal::kill;
                use nix::unistd::Pid;
                if kill(Pid::from_raw(pid as i32), None).is_ok() {
                    // Process is alive — check if it holds this port
                    // On macOS: lsof -p <pid> -i :<port>
                    // On Linux: ss -tlnp = <port> shows pid
                    return check_port_holder(pid, port);
                }
            }
        }
    }
    false
}
```

---

## 7. P2 — Fix `proxy stop` test race condition

**The problem:** `test_proxy_stop_when_not_running` fails because the error message goes to stderr but the test only checks stdout.

### File: `src/cli/proxy.rs` (lines 93-104)

**7a.** Print errors to stdout (consistent with other commands):

```rust
// BEFORE:
Err(e) => {
    eprintln!("  ✗ {e}");
}

// AFTER:
Err(e) => {
    println!("  ✗ {e}");
}
```

### File: `tests/e2e_binary.rs` (lines 122-129)

**7b.** Fix the test to check both streams:

```rust
#[test]
fn test_proxy_stop_when_not_running() {
    let (stdout, stderr, _) = run_antra(&["proxy", "stop"]);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("not running") || combined.contains("Daemon not running"),
        "Expected 'not running' in output.\nstdout: {stdout}\nstderr: {stderr}"
    );
}
```

---

## 8. P2 — Install script TTY portability

### File: `install.sh` (lines 187-223)

**8a.** Make the trust prompt work in all pipe contexts:

```bash
ask_trust() {
    local binary_path="$1"

    header "Trust Setup"
    echo "  Antra generates a local CA certificate to serve HTTPS for"
    echo "  domains like https://myapp.localhost and https://myapp.test."
    echo ""
    echo "  ${BOLD}Installing the CA into your system trust store${RESET} means"
    echo "  HTTPS works with zero browser warnings — forever."
    echo ""
    echo "  ${DIM}This requires admin privileges (sudo) on macOS/Linux.${RESET}"
    echo "  ${DIM}The CA is local-only. Nothing is sent anywhere.${RESET}"
    echo ""

    # Detect TTY availability
    if [ -t 0 ] 2>/dev/null; then
        # Stdin is a terminal
        printf "  Install CA into system trust store? [Y/n] "
        read -r response
    elif [ -e /dev/tty ] 2>/dev/null; then
        # Can read from /dev/tty even when piped
        printf "  Install CA into system trust store? [Y/n] "
        read -r response < /dev/tty
    else
        # Non-interactive: default to YES (auto-install)
        echo "  Non-interactive mode — auto-installing CA..."
        response="y"
    fi

    case "$response" in
        [nN][oO]|[nN])
            echo ""
            warn "Skipped. You can run 'antra trust' later."
            warn "HTTPS for custom domains may show cert warnings until then."
            ;;
        *)
            echo ""
            info "Installing CA into system trust store..."
            if "$binary_path" trust --yes; then
                ok "CA installed. HTTPS will work with no warnings."
            else
                echo ""
                warn "CA install failed or was cancelled."
                warn "You can run 'antra trust' later to try again."
                warn "You can run 'antra doctor' to diagnose issues."
            fi
            ;;
    esac
}
```

Key change: **default to YES** in non-interactive mode (piped install). The user ran the install script — they want it to work.

---

## Implementation Order

| Phase | Fixes | What changes |
|-------|-------|-------------|
| **Phase 1** | 1a-1c, 2a-2d, 3a-3c | `antra run` auto-trusts, auto-fallback ports, honest startup output |
| **Phase 2** | 4a-4c | 503 errors are useful and actionable |
| **Phase 3** | 5a-5d, 6, 7a-7b, 8a | `--yes` flags, doctor smarts, test fixes, installer portability |

---

## Testing Checklist

After all fixes, verify the **promise**:

- [ ] `antra run --domain my.localhost -- pnpm dev` → works on first run, no prompts, CA auto-installed
- [ ] Ports 443/80 unavailable → auto-falls back to 8443/8080, tells user which port
- [ ] `antra proxy start` → only prints "✓" for ports that actually bound
- [ ] `curl` to dead upstream → 503 with domain, port, error, and fix instructions
- [ ] `antra trust --yes` → no prompt, installs CA
- [ ] `antra clean --yes` → no prompt, cleans state
- [ ] `antra doctor` → says "Antra daemon active" (not "in use") for own ports
- [ ] `test_proxy_stop_when_not_running` → passes 10/10 times
- [ ] `curl ... | bash install.sh` → auto-installs CA in non-interactive mode
- [ ] `cargo test` — all tests pass
- [ ] `cargo clippy -- -D warnings` — zero warnings
- [ ] `cargo fmt --check` — clean
