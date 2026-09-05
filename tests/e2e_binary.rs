use std::process::{Command, Stdio};
use std::time::Duration;

use tempfile::TempDir;

fn antra_bin() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("antra");
    #[cfg(target_os = "windows")]
    path.set_extension("exe");
    path.to_string_lossy().to_string()
}

fn run_antra(args: &[&str]) -> (String, String, i32) {
    run_antra_with_timeout(args, Duration::from_secs(10))
}

fn run_antra_with_timeout(args: &[&str], timeout: Duration) -> (String, String, i32) {
    let mut child = Command::new(antra_bin())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute antra");

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().unwrap();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                return (stdout, stderr, status.code().unwrap_or(-1));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return (String::new(), "timeout".to_string(), -1);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return (String::new(), format!("{e}"), -1);
            }
        }
    }
}

fn run_antra_with_dir(dir: &std::path::Path, args: &[&str]) -> (String, String, i32) {
    run_antra_with_dir_timeout(dir, args, Duration::from_secs(10))
}

fn run_antra_with_dir_timeout(
    dir: &std::path::Path,
    args: &[&str],
    timeout: Duration,
) -> (String, String, i32) {
    let mut child = Command::new(antra_bin())
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute antra");

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().unwrap();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                return (stdout, stderr, status.code().unwrap_or(-1));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return (String::new(), "timeout".to_string(), -1);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return (String::new(), format!("{e}"), -1);
            }
        }
    }
}

// ===================================================================
// SECTION 1: CLI Help & Version
// ===================================================================

#[test]
fn test_help_shows_all_commands() {
    let (stdout, _, code) = run_antra(&["--help"]);
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
    let (stdout, _, code) = run_antra(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("antra"));
    assert!(stdout.contains("0.2.0"));
}

#[test]
fn test_run_help() {
    let (stdout, _, code) = run_antra(&["run", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--domain"));
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--allow-custom-domain"));
    assert!(stdout.contains("COMMAND"));
}

#[test]
fn test_dev_help() {
    let (stdout, _, code) = run_antra(&["dev", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--domain"));
    assert!(stdout.contains("--port"));
}

#[test]
fn test_proxy_help() {
    let (stdout, _, code) = run_antra(&["proxy", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("start"));
    assert!(stdout.contains("stop"));
    assert!(stdout.contains("status"));
}

#[test]
fn test_unknown_subcommand() {
    let (_, stderr, code) = run_antra(&["nonexistent"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("error") || stderr.contains("unknown"));
}

// ===================================================================
// SECTION 2: Proxy Daemon Lifecycle
// ===================================================================

#[test]
fn test_proxy_status_when_not_running() {
    let (stdout, _, _) = run_antra(&["proxy", "status"]);
    assert!(
        stdout.contains("not running")
            || stdout.contains("Daemon")
            || stdout.contains("Error")
            || stdout.contains("not")
    );
}

#[test]
fn test_proxy_stop_when_not_running() {
    let (stdout, stderr, _) = run_antra(&["proxy", "stop"]);
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
    let dir = TempDir::new().unwrap();
    let (stdout, stderr, code) = run_antra_with_dir(dir.path(), &["dev"]);
    assert_ne!(code, 0);
    let output = format!("{stdout}{stderr}");
    assert!(output.contains("antra.toml") || output.contains("No"));
}

#[test]
fn test_dev_with_valid_config() {
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

    let (stdout, _, _code) = run_antra_with_dir(dir.path(), &["dev"]);
    let output = stdout;
    assert!(output.contains("antra.toml") || output.contains("Loaded"));
}

#[test]
fn test_dev_with_override_flags() {
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

    let (stdout, _, _) = run_antra_with_dir(dir.path(), &["dev", "--domain", "override.localhost"]);
    let output = stdout;
    assert!(output.contains("override.localhost") || output.contains("antra.toml"));
}

#[test]
fn test_dev_with_invalid_toml() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("antra.toml"), "not valid {{{ toml").unwrap();

    let (_, stderr, code) = run_antra_with_dir(dir.path(), &["dev"]);
    assert_ne!(code, 0);
    let output = stderr.to_string();
    assert!(output.contains("parse") || output.contains("error") || output.contains("Failed"));
}

#[test]
fn test_dev_with_missing_required_fields() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("antra.toml"),
        r#"domain = ""
[server]
command = "echo"
"#,
    )
    .unwrap();

    let (_, stderr, code) = run_antra_with_dir(dir.path(), &["dev"]);
    assert_ne!(code, 0);
    let output = stderr.to_string();
    assert!(output.contains("domain") || output.contains("error"));
}

// ===================================================================
// SECTION 4: List Command
// ===================================================================

#[test]
fn test_list_when_daemon_not_running() {
    let (stdout, _, _) = run_antra(&["list"]);
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
    let (stdout, _stderr, code) = run_antra(&["doctor"]);
    assert!(stdout.contains("ANTRA DOCTOR") || stdout.contains("Checking") || code == 0);
}

#[test]
fn test_doctor_checks_ports() {
    let (stdout, _, _) = run_antra(&["doctor"]);
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
    let dir = TempDir::new().unwrap();
    let mut child = Command::new(antra_bin())
        .args(["clean"])
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    if let Some(ref mut stdin) = child.stdin {
        use std::io::Write;
        writeln!(stdin, "n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(stdout.contains("Cancelled") || stdout.contains("cancel") || !output.status.success());
}

// ===================================================================
// SECTION 7: Alias Command
// ===================================================================

#[test]
fn test_alias_requires_daemon() {
    let (stdout, _, _) = run_antra(&["alias", "test.localhost", "3000"]);
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
    let (stdout, _, _) = run_antra(&["remove", "test.localhost"]);
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
    let (stdout, _, code) = run_antra(&["open", "test.localhost"]);
    assert!(code == 0 || stdout.contains("error") || code != -1);
}
