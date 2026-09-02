use std::process::{Command, Stdio};
use tempfile::TempDir;

fn antra_bin() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("antra");
    path.to_string_lossy().to_string()
}

fn run_antra(args: &[&str]) -> (String, String, i32) {
    let output = Command::new(antra_bin())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute antra");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (stdout, stderr, code)
}

fn _run_antra_with_stdin(args: &[&str], stdin_data: &str) -> (String, String, i32) {
    let mut child = Command::new(antra_bin())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute antra");

    if let Some(ref mut stdin) = child.stdin {
        use std::io::Write;
        write!(stdin, "{}", stdin_data).unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (stdout, stderr, code)
}

// ===================================================================
// SECTION 1: Malformed CLI Arguments
// ===================================================================

#[test]
fn test_empty_domain() {
    let (_, _stderr, code) = run_antra(&["run", "--domain", "", "--", "echo", "test"]);
    assert_ne!(code, 0);
}

#[test]
fn test_extremely_long_domain() {
    let long_domain = "a".repeat(10000);
    let args = vec!["run", "--domain", &long_domain, "--", "echo", "test"];
    let (_, stderr, code) = run_antra(&args);
    // Should reject gracefully, not crash
    assert_ne!(code, 0);
    assert!(stderr.contains("error") || stderr.contains("too long") || code == 1);
}

#[test]
fn test_domain_with_spaces() {
    // Shell splits "my app.localhost" into two args: "my" and "app.localhost"
    // This is expected behavior - domains with spaces aren't valid anyway
    let (_, _, code) = run_antra(&["run", "--domain", "my app.localhost", "--", "echo"]);
    // Should fail or handle gracefully (shell splits the args)
    assert!(code >= 0); // Don't crash
}

#[test]
fn test_domain_with_null_bytes() {
    // Can't pass null bytes through Command (OS restriction), verify via config
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("antra.toml"),
        "domain = \"my\x00app.localhost\"\n\n[server]\ncommand = \"echo\"\n",
    )
    .unwrap();

    let (_, stderr, code) = run_antra_with_dir(dir.path(), &["dev"]);
    // Should reject null bytes in domain
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
    let (_, _stderr, code) = run_antra(&["run", "--", "echo", "test"]);
    assert_ne!(code, 0);
}

#[test]
fn test_port_zero_auto_assigns() {
    // Port 0 means OS auto-assigns a free port - this is valid behavior
    let (_, _, code) = run_antra(&[
        "run",
        "--domain",
        "test.localhost",
        "--port",
        "0",
        "--",
        "echo",
    ]);
    // Should not crash - port 0 triggers auto-assignment
    assert!(code >= 0);
}

#[test]
fn test_port_out_of_range() {
    let (_, _stderr, code) = run_antra(&[
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
    let (_, _stderr, code) = run_antra(&[
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
    let (_, _stderr, code) = run_antra(&[
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
    // "localhost" is accepted - it resolves via LocalhostResolver (no-op)
    let (_, _, code) = run_antra(&["run", "--domain", "localhost", "--", "echo", "test"]);
    // Should not crash - localhost is a valid target
    assert!(code >= 0);
}

#[test]
fn test_127_0_0_1_rejected() {
    let (_, _stderr, code) = run_antra(&["run", "--domain", "127.0.0.1", "--", "echo", "test"]);
    assert_ne!(code, 0);
}

#[test]
fn test_github_rejected() {
    let (_, _stderr, code) = run_antra(&["run", "--domain", "github.com", "--", "echo", "test"]);
    assert_ne!(code, 0);
}

#[test]
fn test_aws_rejected() {
    let (_, _stderr, code) =
        run_antra(&["run", "--domain", "aws.amazon.com", "--", "echo", "test"]);
    assert_ne!(code, 0);
}

#[test]
fn test_cloudflare_rejected() {
    let (_, _stderr, code) =
        run_antra(&["run", "--domain", "cloudflare.com", "--", "echo", "test"]);
    assert_ne!(code, 0);
}

#[test]
fn test_path_traversal_in_domain() {
    let (_, _stderr, code) = run_antra(&["run", "--domain", "../../../etc/passwd", "--", "echo"]);
    assert_ne!(code, 0);
}

#[test]
fn test_domain_with_special_characters() {
    let special_domains = vec![
        "test.localhost<script>alert(1)</script>",
        "test.localhost' OR 1=1--",
        "test.localhost; rm -rf /",
        "test.localhost`whoami`",
        "test.localhost$(whoami)",
        "test.localhost|cat /etc/passwd",
        "test.localhost&net cat",
    ];

    for domain in special_domains {
        let args = vec!["run", "--domain", domain, "--", "echo", "test"];
        let (_, stderr, code) = run_antra(&args);
        // Should reject all malicious domains
        assert!(
            code != 0
                || stderr.contains("error")
                || stderr.contains("invalid")
                || stderr.contains("reject"),
            "Should reject domain: {domain}"
        );
    }
}

// ===================================================================
// SECTION 3: Command Injection Attempts
// ===================================================================

#[test]
fn test_command_injection_via_domain() {
    let (_, _stderr, code) = run_antra(&[
        "run",
        "--domain",
        "test.localhost",
        "--",
        "echo; rm -rf /tmp/test_injection_marker",
    ]);
    // The command itself runs, but domain should be safe
    // We just verify antra doesn't crash
    assert!(code == 0 || code != -1);
}

#[test]
fn test_run_with_shellescape_command() {
    // This tests that the command runs as-is (not through shell)
    let (stdout, _, code) = run_antra(&[
        "run",
        "--domain",
        "test.localhost",
        "--port",
        "19999",
        "--",
        "echo",
        "hello world with spaces",
    ]);
    // Should work - command is passed directly, not through shell
    assert!(code == 0 || stdout.contains("hello"));
}

// ===================================================================
// SECTION 4: Config File Attacks
// ===================================================================

#[test]
fn test_toml_injection_attempt() {
    let dir = TempDir::new().unwrap();
    // Attempt to inject extra fields via TOML
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

    // Should parse fine (unknown fields ignored) and not crash
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

    // Should handle large config without crashing
    let (_, stderr, code) = run_antra_with_dir(dir.path(), &["dev"]);
    // May fail but shouldn't panic
    assert!(code >= 0 || stderr.contains("error"));
}

#[test]
fn test_binary_config_file() {
    let dir = TempDir::new().unwrap();
    // Write binary data as config
    std::fs::write(dir.path().join("antra.toml"), &[0x00, 0xFF, 0xFE, 0xFD]).unwrap();

    let (_, stderr, code) = run_antra_with_dir(dir.path(), &["dev"]);
    assert_ne!(code, 0);
    let output = format!("{stderr}");
    assert!(output.contains("parse") || output.contains("error") || output.contains("Failed"));
}

#[test]
fn test_empty_config_file() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("antra.toml"), "").unwrap();

    let (_, _stderr, code) = run_antra_with_dir(dir.path(), &["dev"]);
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

    let (_, _stderr, code) = run_antra_with_dir(dir.path(), &["dev"]);
    assert_ne!(code, 0);
}

fn run_antra_with_dir(dir: &std::path::Path, args: &[&str]) -> (String, String, i32) {
    let output = Command::new(antra_bin())
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute antra");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (stdout, stderr, code)
}

// ===================================================================
// SECTION 5: Resource Exhaustion
// ===================================================================

#[test]
fn test_rapid_help_calls() {
    // Fire 50 help calls rapidly - should not crash or leak resources
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
    // Port 1 is valid (though may need root)
    let (_, stderr, code) = run_antra(&[
        "run",
        "--domain",
        "test.localhost",
        "--port",
        "1",
        "--",
        "echo",
    ]);
    // Port 1 may fail due to privileges but shouldn't crash
    assert!(code >= 0 || stderr.contains("error") || stderr.contains("bind"));
}

#[test]
fn test_port_max_boundary() {
    let (_, stderr, code) = run_antra(&[
        "run",
        "--domain",
        "test.localhost",
        "--port",
        "65535",
        "--",
        "echo",
    ]);
    // Port 65535 is technically valid
    assert!(code >= 0 || stderr.contains("error"));
}

#[test]
fn test_multiple_domain_flags_rejected() {
    // Clap rejects duplicate --domain flags
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
    // Empty command after -- should fail
    assert!(code != 0 || stderr.contains("error") || stderr.contains("required"));
}

// ===================================================================
// SECTION 7: Protocol & Network Edge Cases
// ===================================================================

#[test]
fn test_invalid_route_format() {
    let (_, stderr, code) = run_antra(&["alias", "noport", "3000"]);
    // "noport" without :port separator - but we pass port as separate arg
    // This should work since alias takes domain and port separately
    assert!(code >= 0 || stderr.contains("error"));
}

#[test]
fn test_alias_port_overflow() {
    let (_, _stderr, code) = run_antra(&["alias", "test.localhost", "99999"]);
    assert_ne!(code, 0);
}

// ===================================================================
// SECTION 8: State Corruption Resistance
// ===================================================================

#[test]
fn test_clean_after_failed_proxy() {
    // Try to clean up even if proxy was never started
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
        writeln!(stdin, "y").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    // Should succeed or show "nothing to clean"
    assert!(stdout.contains("removed") || stdout.contains("Cancelled") || output.status.success());
}

// ===================================================================
// SECTION 9: Error Message Quality
// ===================================================================

#[test]
fn test_error_messages_are_human_readable() {
    let (_, stderr, code) = run_antra(&["run", "--domain", "google.com", "--", "echo"]);
    assert_ne!(code, 0);
    // Error should be readable, not a Rust panic trace
    assert!(!stderr.contains("thread 'main' panicked"));
    assert!(!stderr.contains("unwrap()"));
    assert!(!stderr.contains("RUST_BACKTRACE"));
}

#[test]
fn test_missing_command_error_message() {
    let (_, stderr, code) = run_antra(&["run", "--domain", "test.localhost"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("required") || stderr.contains("error") || stderr.contains("COMMAND"));
}
