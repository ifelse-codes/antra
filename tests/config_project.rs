use tempfile::TempDir;

use antra::config::project::{config_path, load_from_path};

#[test]
fn test_config_path_returns_antra_toml() {
    let path = config_path();
    assert_eq!(path.to_str().unwrap(), "antra.toml");
}

#[test]
fn test_load_from_path_nonexistent() {
    let result = load_from_path(std::path::Path::new("/nonexistent/path/antra.toml")).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_load_from_path_valid_config() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("antra.toml");

    std::fs::write(
        &path,
        r#"domain = "myapp.localhost"

[server]
command = "pnpm"
args = ["dev"]
port = 5173
"#,
    )
    .unwrap();

    let config = load_from_path(&path).unwrap().unwrap();
    assert_eq!(config.domain, "myapp.localhost");
    assert_eq!(config.server.command, "pnpm");
    assert_eq!(config.server.args, vec!["dev"]);
    assert_eq!(config.server.port, Some(5173));
    assert!(!config.server.allow_custom_domain);
}

#[test]
fn test_load_from_path_minimal_config() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("antra.toml");

    std::fs::write(
        &path,
        r#"domain = "test.localhost"

[server]
command = "node"
"#,
    )
    .unwrap();

    let config = load_from_path(&path).unwrap().unwrap();
    assert_eq!(config.domain, "test.localhost");
    assert_eq!(config.server.command, "node");
    assert!(config.server.args.is_empty());
    assert_eq!(config.server.port, None);
}

#[test]
fn test_load_from_path_invalid_toml() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("antra.toml");

    std::fs::write(&path, "this is not valid toml {{{").unwrap();

    let result = load_from_path(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_from_path_missing_domain() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("antra.toml");

    std::fs::write(
        &path,
        r#"[server]
command = "pnpm"
"#,
    )
    .unwrap();

    let result = load_from_path(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_from_path_empty_domain() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("antra.toml");

    std::fs::write(
        &path,
        r#"domain = ""

[server]
command = "pnpm"
"#,
    )
    .unwrap();

    let result = load_from_path(&path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("domain"));
}

#[test]
fn test_load_from_path_empty_command() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("antra.toml");

    std::fs::write(
        &path,
        r#"domain = "test.localhost"

[server]
command = ""
"#,
    )
    .unwrap();

    let result = load_from_path(&path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("command"));
}

#[test]
fn test_load_from_path_allow_custom_domain() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("antra.toml");

    std::fs::write(
        &path,
        r#"domain = "custom.example.com"

[server]
command = "vite"
allow_custom_domain = true
"#,
    )
    .unwrap();

    let config = load_from_path(&path).unwrap().unwrap();
    assert!(config.server.allow_custom_domain);
}

#[test]
fn test_load_from_path_extra_fields_ignored() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("antra.toml");

    std::fs::write(
        &path,
        r#"domain = "test.localhost"
unknown_field = "value"

[server]
command = "node"
also_unknown = 42
"#,
    )
    .unwrap();

    let config = load_from_path(&path).unwrap().unwrap();
    assert_eq!(config.domain, "test.localhost");
}
