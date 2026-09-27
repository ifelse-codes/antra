mod common;

use common::*;
use std::time::Duration;

use tempfile::TempDir;
// ===================================================================
// SECTION 1: Malformed CLI Arguments
// ===================================================================

#[test]
fn test_extremely_long_domain() {
    let home = TestHome::shared();
    let long_domain = "a".repeat(10000);
    let args = vec!["run", "--domain", &long_domain, "--", "echo", "test"];
    let (_, stderr, code) = run_antra(home, &args);
    assert_ne!(code, 0);
    assert!(stderr.contains("error") || stderr.contains("too long") || code == 1);
}

#[test]
fn test_domain_with_spaces() {
    let home = TestHome::shared();
    let (_, _, code) = run_antra_with_timeout(
        home,
        &["run", "--domain", "my app.localhost", "--", "echo"],
        Duration::from_secs(5),
    );
    assert!(code >= 0);
}

#[test]
fn test_domain_with_null_bytes() {
    let home = TestHome::shared();
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("antra.toml"),
        "domain = \"my\x00app.localhost\"\n\n[server]\ncommand = \"echo\"\n",
    )
    .unwrap();

    let (_, stderr, code) = run_antra_with_dir(home, dir.path(), &["dev"]);
    assert!(code != 0 || stderr.contains("error") || stderr.contains("parse"));
}

#[test]
fn test_run_without_command() {
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra(home, &["run", "--domain", "test.localhost"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("error") || stderr.contains("required") || stderr.contains("COMMAND"));
}

#[test]
fn test_run_without_domain() {
    let home = TestHome::shared();
    let (_, _, code) = run_antra(home, &["run", "--", "echo", "test"]);
    assert_ne!(code, 0);
}

#[test]
fn test_port_zero_auto_assigns() {
    let home = TestHome::shared();
    let (_, _, code) = run_antra_with_timeout(
        home,
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
    let home = TestHome::shared();
    let (_, _, code) = run_antra(
        home,
        &[
            "run",
            "--domain",
            "test.localhost",
            "--port",
            "99999",
            "--",
            "echo",
        ],
    );
    assert_ne!(code, 0);
}

#[test]
fn test_negative_port() {
    let home = TestHome::shared();
    let (_, _, code) = run_antra(
        home,
        &[
            "run",
            "--domain",
            "test.localhost",
            "--port",
            "-1",
            "--",
            "echo",
        ],
    );
    assert_ne!(code, 0);
}

#[test]
fn test_non_numeric_port() {
    let home = TestHome::shared();
    let (_, _, code) = run_antra(
        home,
        &[
            "run",
            "--domain",
            "test.localhost",
            "--port",
            "abc",
            "--",
            "echo",
        ],
    );
    assert_ne!(code, 0);
}

// ===================================================================
// SECTION 2: Malicious Domain Patterns
// ===================================================================

#[test]
fn test_public_domain_rejected() {
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra(
        home,
        &["run", "--domain", "google.com", "--", "echo", "test"],
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("--allow-custom-domain"));
}

#[test]
fn test_custom_domain_rejected_without_approval() {
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra(
        home,
        &[
            "run",
            "--domain",
            "api.customer.example",
            "--",
            "echo",
            "test",
        ],
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("--allow-custom-domain"));
}

#[test]
fn test_localhost_bare_accepted() {
    let home = TestHome::shared();
    let (_, _, code) = run_antra_with_timeout(
        home,
        &["run", "--domain", "localhost", "--", "echo", "test"],
        Duration::from_secs(5),
    );
    assert!(code >= 0);
}

#[test]
fn test_github_rejected() {
    let home = TestHome::shared();
    let (_, _, code) = run_antra(
        home,
        &["run", "--domain", "github.com", "--", "echo", "test"],
    );
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
    let home = TestHome::shared();
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

    let (stdout, _, _) = run_antra_with_dir(home, dir.path(), &["dev"]);
    assert!(stdout.contains("antra.toml") || stdout.contains("Loaded"));
}

#[test]
fn test_extremely_large_config() {
    let home = TestHome::shared();
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
        run_antra_with_dir_timeout(home, dir.path(), &["dev"], Duration::from_secs(30));
    assert!(code >= 0 || stderr.contains("error") || stderr.contains("timeout"));
}

#[test]
fn test_binary_config_file() {
    let home = TestHome::shared();
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("antra.toml"), [0x00, 0xFF, 0xFE, 0xFD]).unwrap();

    let (_, stderr, code) = run_antra_with_dir(home, dir.path(), &["dev"]);
    assert_ne!(code, 0);
    let output = stderr.to_string();
    assert!(output.contains("parse") || output.contains("error") || output.contains("Failed"));
}

#[test]
fn test_empty_config_file() {
    let home = TestHome::shared();
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("antra.toml"), "").unwrap();

    let (_, _, code) = run_antra_with_dir(home, dir.path(), &["dev"]);
    assert_ne!(code, 0);
}

#[test]
fn test_config_with_only_comments() {
    let home = TestHome::shared();
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("antra.toml"),
        "# This is a comment\n# Another comment\n",
    )
    .unwrap();

    let (_, _, code) = run_antra_with_dir(home, dir.path(), &["dev"]);
    assert_ne!(code, 0);
}

// ===================================================================
// SECTION 5: Resource Exhaustion
// ===================================================================

#[test]
fn test_rapid_help_calls() {
    let home = TestHome::shared();
    for _ in 0..50 {
        let (_, _, code) = run_antra(home, &["--help"]);
        assert_eq!(code, 0);
    }
}

#[test]
fn test_concurrent_status_calls() {
    let home = TestHome::shared();
    use std::thread;

    let handles: Vec<_> = (0..10)
        .map(|_| {
            thread::spawn(move || {
                for _ in 0..5 {
                    let (_, _, code) = run_antra(home, &["proxy", "status"]);
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
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra_with_timeout(
        home,
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
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra_with_timeout(
        home,
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
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra(
        home,
        &[
            "run",
            "--domain",
            "first.localhost",
            "--domain",
            "second.localhost",
            "--",
            "echo",
        ],
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("cannot be used multiple times") || stderr.contains("error"));
}

#[test]
fn test_empty_command_args() {
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra(home, &["run", "--domain", "test.localhost", "--"]);
    assert!(code != 0 || stderr.contains("error") || stderr.contains("required"));
}

// ===================================================================
// SECTION 7: Protocol & Network Edge Cases
// ===================================================================

#[test]
fn test_invalid_route_format() {
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra(home, &["alias", "noport", "3000"]);
    assert!(code >= 0 || stderr.contains("error"));
}

#[test]
fn test_alias_port_overflow() {
    let home = TestHome::shared();
    let (_, _, code) = run_antra(home, &["alias", "test.localhost", "99999"]);
    assert_ne!(code, 0);
}

// ===================================================================
// SECTION 8: State Corruption Resistance
// ===================================================================

// ===================================================================
// SECTION 9: Error Message Quality
// ===================================================================

#[test]
fn test_error_messages_are_human_readable() {
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra_with_timeout(
        home,
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
    let home = TestHome::shared();
    let (_, stderr, code) = run_antra_with_timeout(
        home,
        &["run", "--domain", "test.localhost"],
        Duration::from_secs(5),
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("required") || stderr.contains("error") || stderr.contains("COMMAND"));
}
