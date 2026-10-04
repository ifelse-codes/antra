use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::platform;

const GLOBAL_CONFIG_FILE: &str = "config.toml";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Whether the trust prompt has been shown at least once
    #[serde(default)]
    pub trust_prompted: bool,

    /// The answer to "Use port 443?" (ROADMAP C27). `None` until asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_port_443: Option<bool>,
}

fn global_config_path() -> PathBuf {
    platform::config_dir().join(GLOBAL_CONFIG_FILE)
}

/// Load the global config from disk. Returns default if file doesn't exist.
pub fn load_global_config() -> GlobalConfig {
    let path = global_config_path();
    if !path.exists() {
        return GlobalConfig::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_default()
}

/// Save the global config to disk.
pub fn save_global_config(config: &GlobalConfig) -> Result<()> {
    let path = global_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Mark the trust prompt as having been shown.
pub fn mark_trust_prompted() -> Result<()> {
    let mut config = load_global_config();
    config.trust_prompted = true;
    save_global_config(&config)
}

/// Check if the trust prompt has been shown before.
pub fn was_trust_prompted() -> bool {
    load_global_config().trust_prompted
}

/// The saved answer to "Use port 443?", or `None` if it was never asked.
#[cfg(unix)]
pub fn port_443_answer() -> Option<bool> {
    load_global_config().use_port_443
}

/// Remember the answer to "Use port 443?".
#[cfg(unix)]
pub fn remember_port_443(answer: bool) -> Result<()> {
    let mut config = load_global_config();
    config.use_port_443 = Some(answer);
    save_global_config(&config)
}
