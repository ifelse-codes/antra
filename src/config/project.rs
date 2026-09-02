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
