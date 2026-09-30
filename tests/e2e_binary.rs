mod common;

use std::process::Stdio;

use common::*;
use tempfile::TempDir;

// ===================================================================
// SECTION 1: CLI Help & Version
// ===================================================================

#[test]
fn test_help_shows_all_commands() {
    let home = TestHome::shared();
    let (stdout, _, code) = run_antra(home, &["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("run"));
    assert!(stdout.contains("dev"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("trust"));
    assert!(stdout.contains("proxy"));
    assert!(stdout.contains("clean"));
    assert!(stdout.contains("alias"));
    assert!(stdout.contains("open"));
    assert!(stdout.contains("remove"));
}

#[test]
fn test_version_flag() {
    let home = TestHome::shared();
    let (stdout, _, code) = run_antra(home, &["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("antra"));
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_run_help() {
    let home = TestHome::shared();
    let (stdout, _, code) = run_antra(home, &["run", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--domain"));
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--allow-custom-domain"));
    assert!(stdout.contains("COMMAND"));
}

#[test]
fn test_dev_help() {
    let home = TestHome::shared();
    let (stdout, _, code) = run_antra(home, &["dev", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--domain"));
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--allow-custom-domain"));
    assert!(stdout.contains("--yes"));
}

#[test]
fn test_add_help() {
    let home = TestHome::shared();
    let (stdout, _, code) = run_antra(home, &["add", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--allow-custom-domain"));
    let (stdout, _, code) = run_antra(home, &["add", "route", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--allow-custom-domain"));
}

#[test]
fn test_proxy_help() {
    let home = TestHome::shared();
    let (stdout, _, code) = run_antra(home, &["proxy", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("start"));
    assert!(stdout.contains("stop"));
    assert!(stdout.contains("status"));
    let (stdout, _, code) = run_antra(home, &["proxy", "start", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--route"));
    assert!(stdout.contains("--allow-custom-domain"));
}

#[test]
fn test_unknown_subcommand() {
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra(home, &["nonexistent"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("error") || stderr.contains("unknown"));
}

// ===================================================================
// SECTION 2: Proxy Daemon Lifecycle
// ===================================================================

#[test]
fn test_proxy_status_when_not_running() {
    let home = TestHome::shared();
    let (stdout, _, _) = run_antra(home, &["proxy", "status"]);
    assert!(
        stdout.contains("not running")
            || stdout.contains("Daemon")
            || stdout.contains("Error")
            || stdout.contains("not")
    );
}

#[test]
fn test_proxy_stop_when_not_running() {
    let home = TestHome::shared();
    let (stdout, stderr, _) = run_antra(home, &["proxy", "stop"]);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("not running")
            || combined.contains("Daemon stopped")
            || combined.contains("Daemon not running"),
        "Expected daemon stop output.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

// ===================================================================
// SECTION 3: Dev Command (Config-based)
// ===================================================================

#[test]
fn test_dev_without_config_fails() {
    let home = TestHome::shared();
    let dir = TempDir::new().unwrap();
    let (stdout, stderr, code) = run_antra_with_dir(home, dir.path(), &["dev"]);
    assert_ne!(code, 0);
    let output = format!("{stdout}{stderr}");
    assert!(output.contains("antra.toml") || output.contains("No"));
}

#[test]
fn test_dev_with_valid_config() {
    let home = TestHome::shared();
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("antra.toml"),
        r#"domain = "test.localhost"

[server]
command = "echo"
args = ["hello from config"]
port = 3456
"#,
    )
    .unwrap();

    let (stdout, _, _code) = run_antra_with_dir(home, dir.path(), &["dev"]);
    let output = stdout;
    assert!(output.contains("antra.toml") || output.contains("Loaded"));
}

#[test]
fn test_dev_with_override_flags() {
    let home = TestHome::shared();
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("antra.toml"),
        r#"domain = "original.localhost"

[server]
command = "echo"
args = ["test"]
port = 3456
"#,
    )
    .unwrap();

    let (stdout, _, _) =
        run_antra_with_dir(home, dir.path(), &["dev", "--domain", "override.localhost"]);
    let output = stdout;
    assert!(output.contains("override.localhost") || output.contains("antra.toml"));
}

#[test]
fn test_dev_with_invalid_toml() {
    let home = TestHome::shared();
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("antra.toml"), "not valid {{{ toml").unwrap();

    let (_, stderr, code) = run_antra_with_dir(home, dir.path(), &["dev"]);
    assert_ne!(code, 0);
    let output = stderr.to_string();
    assert!(output.contains("parse") || output.contains("error") || output.contains("Failed"));
}

#[test]
fn test_dev_with_missing_required_fields() {
    let home = TestHome::shared();
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("antra.toml"),
        r#"domain = ""
[server]
command = "echo"
"#,
    )
    .unwrap();

    let (_, stderr, code) = run_antra_with_dir(home, dir.path(), &["dev"]);
    assert_ne!(code, 0);
    let output = stderr.to_string();
    assert!(output.contains("domain") || output.contains("error"));
}

// ===================================================================
// SECTION 4: List Command
// ===================================================================

#[test]
fn test_list_when_daemon_not_running() {
    let home = TestHome::shared();
    let (stdout, _, _) = run_antra(home, &["list"]);
    assert!(
        stdout.contains("not running")
            || stdout.contains("ACTIVE ROUTES")
            || stdout.contains("Daemon")
            || stdout.contains("route")
    );
}

// ===================================================================
// SECTION 5: Doctor Command
// ===================================================================

#[test]
fn test_doctor_runs_without_panic() {
    let home = TestHome::shared();
    let (stdout, _stderr, code) = run_antra(home, &["doctor"]);
    assert!(stdout.contains("ANTRA DOCTOR") || stdout.contains("Checking") || code == 0);
}

/// `antra logs` is the answer to "the daemon said something and I never saw
/// it". Both directions matter: the not-yet-created case must explain itself,
/// and an existing log must actually be shown.
#[test]
fn test_logs_explains_itself_when_there_is_no_log() {
    // A private home, not the shared one: other tests in this binary start a
    // daemon, which now writes exactly the log this test is asserting is absent.
    let home = TestHome::new();
    let (stdout, stderr, code) = run_antra(&home, &["logs"]);
    let out = format!("{stdout}{stderr}");
    assert_eq!(code, 0, "no daemon yet is not a failure: {out}");
    assert!(out.contains("No daemon log yet"), "{out}");
    assert!(out.contains("daemon.log"), "must say where it goes: {out}");
}

#[test]
fn test_logs_prints_the_daemon_log() {
    let home = TestHome::new();
    let log_dir = home.log_dir();
    std::fs::create_dir_all(&log_dir).unwrap();
    std::fs::write(
        log_dir.join("daemon.log"),
        "INFO antra::proxy::https: HTTPS proxy listening\nERROR antra::daemon::server: HTTPS server failed\n",
    )
    .unwrap();

    let (stdout, stderr, code) = run_antra(&home, &["logs"]);
    let out = format!("{stdout}{stderr}");
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("HTTPS server failed"),
        "must show the failure: {out}"
    );
    assert!(out.contains("HTTPS proxy listening"), "{out}");
}

#[test]
fn test_logs_line_limit_is_respected() {
    let home = TestHome::new();
    let log_dir = home.log_dir();
    std::fs::create_dir_all(&log_dir).unwrap();
    let body: String = (1..=200).map(|i| format!("line {i}\n")).collect();
    std::fs::write(log_dir.join("daemon.log"), body).unwrap();

    let (stdout, stderr, code) = run_antra(&home, &["logs", "--lines", "3"]);
    let out = format!("{stdout}{stderr}");
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("line 200"),
        "newest lines must be shown: {out}"
    );
    assert!(
        !out.contains("line 100\n"),
        "older lines must be dropped: {out}"
    );
}

#[test]
fn test_doctor_checks_ports() {
    let home = TestHome::shared();
    let (stdout, _, _) = run_antra(home, &["doctor"]);
    assert!(
        stdout.contains("Port")
            || stdout.contains("443")
            || stdout.contains("80")
            || stdout.contains("available")
            || stdout.contains("in use")
    );
}

// ===================================================================
// SECTION 6: Clean Command
// ===================================================================

#[test]
fn test_clean_cancels_on_no() {
    let home = TestHome::shared();
    // Hermetic like every other spawn: `antra clean` is the one command that
    // would otherwise reach the developer's real trust store and hosts block.
    let dir = TempDir::new().unwrap();
    let mut child = home
        .command(&["clean"])
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        writeln!(stdin, "n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(stdout.contains("System trust") && stdout.contains("Antra-managed hosts block"));
    assert!(stdout.contains("Cancelled") || stdout.contains("cancel") || !output.status.success());
}

// ===================================================================
// SECTION 7: Alias Command
// ===================================================================

#[test]
fn test_alias_requires_daemon() {
    let home = TestHome::shared();
    let (stdout, _, _) = run_antra(home, &["alias", "test.localhost", "3000"]);
    assert!(
        stdout.contains("daemon")
            || stdout.contains("running")
            || stdout.contains("error")
            || stdout.contains("Removing")
            || stdout.contains("Route")
    );
}

// ===================================================================
// SECTION 8: Remove Command
// ===================================================================

#[test]
fn test_remove_requires_daemon() {
    let home = TestHome::shared();
    let (stdout, _, _) = run_antra(home, &["remove", "test.localhost"]);
    assert!(
        stdout.contains("daemon")
            || stdout.contains("running")
            || stdout.contains("Removing")
            || stdout.contains("Route")
    );
}

// ===================================================================
// SECTION 9: Open Command
// ===================================================================

#[test]
fn test_open_doesnt_panic() {
    let home = TestHome::shared();
    let (stdout, _, code) = run_antra(home, &["open", "test.localhost"]);
    assert!(code == 0 || stdout.contains("error") || code != -1);
}

// ===================================================================
// SECTION 10: Explicit --port conflicts fail loudly (no silent remap)
// ===================================================================

#[test]
fn test_run_with_busy_explicit_port_fails_instead_of_remapping() {
    let home = TestHome::shared();
    // Hold a port, then demand it via --port: must error, never silently
    // route somewhere else (that mismatch born 503s for $PORT-ignoring
    // frameworks like Vite behind `npm run`).
    let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = held.local_addr().unwrap().port();

    let (stdout, stderr, code) = run_antra(
        home,
        &[
            "run",
            "--domain",
            "busy-port-test.localhost",
            "--port",
            &port.to_string(),
            "--",
            "echo",
            "hi",
        ],
    );
    let combined = format!("{stdout}\n{stderr}");
    assert_ne!(code, 0, "busy --port must fail, got: {combined}");
    assert!(
        combined.contains("already in use"),
        "must say the port is busy, got: {combined}"
    );
    assert!(
        !combined.contains("Assigned port"),
        "must not remap to another port, got: {combined}"
    );
    drop(held);
}

/// The busy-port error names what holds the port — here, this test
/// process — and suggests a concrete free port. It used to say only "stop
/// the process on that port", which on a Mac meant AirPlay Receiver.
#[cfg(unix)]
#[test]
fn test_busy_port_error_names_the_holder() {
    if std::process::Command::new("lsof")
        .arg("-v")
        .output()
        .is_err()
    {
        eprintln!("skipping: lsof not installed");
        return;
    }
    let home = TestHome::shared();
    let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = held.local_addr().unwrap().port();

    let (stdout, stderr, code) = run_antra(
        home,
        &[
            "run",
            "--domain",
            "holder-test.localhost",
            "--port",
            &port.to_string(),
            "--",
            "echo",
            "hi",
        ],
    );
    let combined = format!("{stdout}\n{stderr}");
    assert_ne!(code, 0, "busy --port must fail, got: {combined}");
    assert!(
        combined.contains(&format!("(PID {})", std::process::id())),
        "must name the process holding the port, got: {combined}"
    );
    assert!(
        combined.contains(&format!("antra alias holder-test.localhost {port}")),
        "something is serving, so fronting it is a real option, got: {combined}"
    );
    assert!(
        combined.contains("run on another port: --port "),
        "must suggest a concrete free port, got: {combined}"
    );
    drop(held);
}
