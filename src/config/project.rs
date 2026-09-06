use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

const CONFIG_FILE_NAME: &str = "antra.toml";

#[derive(Debug, Deserialize)]
pub struct ProjectConfig {
    /// Domain to proxy (e.g., "myapp.localhost")
    pub domain: String,

    /// Server configuration
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    /// Command to run (e.g., "pnpm", "npm", "vite")
    pub command: String,

    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,

    /// Port the application listens on (auto-detected if omitted)
    pub port: Option<u16>,

    /// Allow custom (non-.localhost, non-.test) domains
    #[serde(default)]
    pub allow_custom_domain: bool,
}

/// Load project config from the current directory.
/// Returns Ok(None) if no antra.toml exists.
pub fn load_project_config() -> Result<Option<ProjectConfig>> {
    let path = Path::new(CONFIG_FILE_NAME);
    if !path.exists() {
        return Ok(None);
    }
    load_from_path(path)
}

/// Load project config from a specific path.
pub fn load_from_path(path: &Path) -> Result<Option<ProjectConfig>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let config: ProjectConfig =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;

    validate(&config, path)?;

    Ok(Some(config))
}

/// Get the path to antra.toml in the current directory.
pub fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE_NAME)
}

fn validate(config: &ProjectConfig, path: &Path) -> Result<()> {
    if config.domain.is_empty() {
        anyhow::bail!(
            "{}: `domain` field is required and cannot be empty",
            path.display()
        );
    }

    if config.server.command.is_empty() {
        anyhow::bail!(
            "{}: `server.command` field is required and cannot be empty",
            path.display()
        );
    }

    Ok(())
}

/// Split a command string into program + args using shell-like quoting.
///
/// `server.command` must be a single binary, but users naturally write
/// `command = "python3 -m http.server 8000"`. Splitting (with support for
/// single/double quotes and backslash escapes) turns that into
/// `["python3", "-m", "http.server", "8000"]` instead of failing with
/// "No such file or directory".
pub fn split_command_string(command: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut has_content = false;

    while let Some(c) = chars.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                current.push(c);
                has_content = true;
            }
        } else if in_double {
            match c {
                '"' => in_double = false,
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                    has_content = true;
                }
                _ => {
                    current.push(c);
                    has_content = true;
                }
            }
        } else {
            match c {
                '\'' => {
                    in_single = true;
                    has_content = true;
                }
                '"' => {
                    in_double = true;
                    has_content = true;
                }
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                    has_content = true;
                }
                _ if c.is_whitespace() => {
                    if has_content {
                        parts.push(std::mem::take(&mut current));
                        has_content = false;
                    }
                }
                _ => {
                    current.push(c);
                    has_content = true;
                }
            }
        }
    }
    if has_content {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_basic() {
        assert_eq!(
            split_command_string("python3 -m http.server 8000"),
            vec!["python3", "-m", "http.server", "8000"]
        );
    }

    #[test]
    fn test_split_single_word() {
        assert_eq!(split_command_string("pnpm"), vec!["pnpm"]);
    }

    #[test]
    fn test_split_quotes() {
        assert_eq!(
            split_command_string("npm run \"my script\""),
            vec!["npm", "run", "my script"]
        );
        assert_eq!(
            split_command_string("echo 'hello world'"),
            vec!["echo", "hello world"]
        );
    }

    #[test]
    fn test_split_escaped_space() {
        assert_eq!(
            split_command_string("echo hello\\ world"),
            vec!["echo", "hello world"]
        );
    }

    #[test]
    fn test_split_blank() {
        assert!(split_command_string("   ").is_empty());
    }
}
