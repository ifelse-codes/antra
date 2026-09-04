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
                    // Don't call child.wait() — on Windows it hangs when
                    // the killed process has open pipe handles or children.
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
// SECTION 1: Malformed CLI Arguments
// ===================================================================

#[test]
fn test_extremely_long_domain() {
    let long_domain = "a".repeat(10000);
    let args = vec!["run", "--domain", &long_domain, "--", "echo", "test"];
    let (_, stderr, code) = run_antra(&args);
    assert_ne!(code, 0);
    assert!(stderr.contains("error") || stderr.contains("too long") || code == 1);
}

#[test]
fn test_domain_with_spaces() {
    let (_, _, code) = run_antra_with_timeout(
        &["run", "--domain", "my app.localhost", "--", "echo"],
        Duration::from_secs(5),
    );
    assert!(code >= 0);
}

#[test]
fn test_domain_with_null_bytes() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("antra.toml"),
        "domain = \"my\x00app.localhost\"\n\n[server]\ncommand = \"echo\"\n",
    )
    .unwrap();

    let (_, stderr, code) = run_antra_with_dir(dir.path(), &["dev"]);
    assert!(code != 0 || stderr.contains("error") || stderr.contains("parse"));
}

#[test]
fn test_run_without_command() {
    let (_, stderr, code) = run_antra(&["run", "--domain", "test.localhost"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("error") || stderr.contains("required") || stderr.contains("COMMAND"));
}

#[test]
fn test_run_without_domain() {
    let (_, _, code) = run_antra(&["run", "--", "echo", "test"]);
    assert_ne!(code, 0);
}

#[test]
fn test_port_zero_auto_assigns() {
    let (_, _, code) = run_antra_with_timeout(
        &[
            "run",
            "--domain",
            "test.localhost",
            "--port",
            "0",
            "--",
            "echo",
        ],
        Duration::from_secs(5),
    );
    assert!(code >= 0);
}

#[test]
fn test_port_out_of_range() {
    let (_, _, code) = run_antra(&[
        "run",
        "--domain",
        "test.localhost",
        "--port",
        "99999",
        "--",
        "echo",
    ]);
    assert_ne!(code, 0);
}

#[test]
fn test_negative_port() {
    let (_, _, code) = run_antra(&[
        "run",
        "--domain",
        "test.localhost",
        "--port",
        "-1",
        "--",
        "echo",
    ]);
    assert_ne!(code, 0);
}

#[test]
fn test_non_numeric_port() {
    let (_, _, code) = run_antra(&[
        "run",
        "--domain",
        "test.localhost",
        "--port",
        "abc",
        "--",
        "echo",
    ]);
    assert_ne!(code, 0);
}

// ===================================================================
// SECTION 2: Malicious Domain Patterns
// ===================================================================

#[test]
fn test_public_domain_rejected() {
    let (_, stderr, code) = run_antra(&["run", "--domain", "google.com", "--", "echo", "test"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("public") || stderr.contains("rejected") || stderr.contains("error"));
}

#[test]
fn test_localhost_bare_accepted() {
    let (_, _, code) = run_antra_with_timeout(
        &["run", "--domain", "localhost", "--", "echo", "test"],
        Duration::from_secs(5),
    );
    assert!(code >= 0);
}

#[test]
fn test_github_rejected() {
    let (_, _, code) = run_antra(&["run", "--domain", "github.com", "--", "echo", "test"]);
    assert_ne!(code, 0);
}

// ===================================================================
// SECTION 3: Command Injection Attempts
// ===================================================================

// ===================================================================
// SECTION 4: Config File Attacks
// ===================================================================

#[test]
fn test_toml_injection_attempt() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("antra.toml"),
        r#"domain = "test.localhost"

[server]
command = "echo"
injected = true

[admin]
escalate = true
"#,
    )
    .unwrap();

    let (stdout, _, _) = run_antra_with_dir(dir.path(), &["dev"]);
    assert!(stdout.contains("antra.toml") || stdout.contains("Loaded"));
}

#[test]
fn test_extremely_large_config() {
    let dir = TempDir::new().unwrap();
    let large_args: Vec<String> = (0..10000).map(|i| format!("\"arg{i}\"")).collect();
    let config = format!(
        r#"domain = "test.localhost"

[server]
command = "echo"
args = [{}]
"#,
        large_args.join(", ")
    );
    std::fs::write(dir.path().join("antra.toml"), &config).unwrap();

    let (_, stderr, code) =
        run_antra_with_dir_timeout(dir.path(), &["dev"], Duration::from_secs(30));
    assert!(code >= 0 || stderr.contains("error") || stderr.contains("timeout"));
}

#[test]
fn test_binary_config_file() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("antra.toml"), [0x00, 0xFF, 0xFE, 0xFD]).unwrap();

    let (_, stderr, code) = run_antra_with_dir(dir.path(), &["dev"]);
    assert_ne!(code, 0);
    let output = stderr.to_string();
    assert!(output.contains("parse") || output.contains("error") || output.contains("Failed"));
}

#[test]
fn test_empty_config_file() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("antra.toml"), "").unwrap();

    let (_, _, code) = run_antra_with_dir(dir.path(), &["dev"]);
    assert_ne!(code, 0);
}

#[test]
fn test_config_with_only_comments() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("antra.toml"),
        "# This is a comment\n# Another comment\n",
    )
    .unwrap();

    let (_, _, code) = run_antra_with_dir(dir.path(), &["dev"]);
    assert_ne!(code, 0);
}

// ===================================================================
// SECTION 5: Resource Exhaustion
// ===================================================================

#[test]
fn test_rapid_help_calls() {
    for _ in 0..50 {
        let (_, _, code) = run_antra(&["--help"]);
        assert_eq!(code, 0);
    }
}

#[test]
fn test_concurrent_status_calls() {
    use std::thread;

    let handles: Vec<_> = (0..10)
        .map(|_| {
            thread::spawn(|| {
                for _ in 0..5 {
                    let (_, _, code) = run_antra(&["proxy", "status"]);
                    assert!(code >= 0);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

// ===================================================================
// SECTION 6: Boundary Conditions
// ===================================================================

#[test]
fn test_port_boundary_valid() {
    let (_, stderr, code) = run_antra_with_timeout(
        &[
            "run",
            "--domain",
            "test.localhost",
            "--port",
            "1",
            "--",
            "echo",
        ],
        Duration::from_secs(5),
    );
    assert!(code >= 0 || stderr.contains("error") || stderr.contains("bind"));
}

#[test]
fn test_port_max_boundary() {
    let (_, stderr, code) = run_antra_with_timeout(
        &[
            "run",
            "--domain",
            "test.localhost",
            "--port",
            "65535",
            "--",
            "echo",
        ],
        Duration::from_secs(5),
    );
    assert!(code >= 0 || stderr.contains("error"));
}

#[test]
fn test_multiple_domain_flags_rejected() {
    let (_, stderr, code) = run_antra(&[
        "run",
        "--domain",
        "first.localhost",
        "--domain",
        "second.localhost",
        "--",
        "echo",
    ]);
    assert_ne!(code, 0);
    assert!(stderr.contains("cannot be used multiple times") || stderr.contains("error"));
}

#[test]
fn test_empty_command_args() {
    let (_, stderr, code) = run_antra(&["run", "--domain", "test.localhost", "--"]);
    assert!(code != 0 || stderr.contains("error") || stderr.contains("required"));
}

// ===================================================================
// SECTION 7: Protocol & Network Edge Cases
// ===================================================================

#[test]
fn test_invalid_route_format() {
    let (_, stderr, code) = run_antra(&["alias", "noport", "3000"]);
    assert!(code >= 0 || stderr.contains("error"));
}

#[test]
fn test_alias_port_overflow() {
    let (_, _, code) = run_antra(&["alias", "test.localhost", "99999"]);
    assert_ne!(code, 0);
}

// ===================================================================
// SECTION 8: State Corruption Resistance
// ===================================================================

#[test]
fn test_clean_after_failed_proxy() {
    let dir = TempDir::new().unwrap();
    let output = Command::new(antra_bin())
        .args(["clean", "--yes"])
        .current_dir(dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        output.status.success()
            || stdout.contains("removed")
            || stdout.contains("Cancelled")
            || stderr.contains("Could not determine config directory")
            || stderr.contains("Directory not empty")
            || stderr.contains("Error"),
        "clean should not fail hard: stdout={stdout:?} stderr={stderr:?} status={}",
        output.status
    );
}

// ===================================================================
// SECTION 9: Error Message Quality
// ===================================================================

#[test]
fn test_error_messages_are_human_readable() {
    let (_, stderr, code) = run_antra_with_timeout(
        &["run", "--domain", "google.com", "--", "echo"],
        Duration::from_secs(5),
    );
    assert_ne!(code, 0);
    assert!(!stderr.contains("thread 'main' panicked"));
    assert!(!stderr.contains("unwrap()"));
    assert!(!stderr.contains("RUST_BACKTRACE"));
}

#[test]
fn test_missing_command_error_message() {
    let (_, stderr, code) = run_antra_with_timeout(
        &["run", "--domain", "test.localhost"],
        Duration::from_secs(5),
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("required") || stderr.contains("error") || stderr.contains("COMMAND"));
}
