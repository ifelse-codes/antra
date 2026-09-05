use anyhow::{Context, Result};
use clap::Args;

use crate::config::project::{config_path, load_project_config};
use crate::util::detect;
use crate::util::output;

use super::run;

#[derive(Args)]
pub struct DevArgs {
    /// Override domain from config
    #[arg(long)]
    pub domain: Option<String>,

    /// Override port from config
    #[arg(long)]
    pub port: Option<u16>,

    /// Skip the trust CA prompt on first run
    #[arg(long)]
    pub no_trust_prompt: bool,
}

pub fn execute(args: DevArgs) -> Result<()> {
    // First try to load antra.toml
    let config = load_project_config()
        .with_context(|| format!("Failed to read {}", config_path().display()))?;

    if let Some(config) = config {
        // Existing behavior: use antra.toml config
        output::print_success(&format!("Loaded {}", config_path().display()));

        let mut command_parts = vec![config.server.command.clone()];
        command_parts.extend(config.server.args.clone());

        let run_args = run::RunArgs {
            domain: args.domain.unwrap_or_else(|| config.domain.clone()),
            port: args.port.or(config.server.port),
            tld: None,
            allow_custom_domain: config.server.allow_custom_domain,
            no_trust_prompt: args.no_trust_prompt,
            yes: false,
            force: false,
            command: command_parts,
        };

        return run::execute(run_args);
    }

    // No antra.toml found — try auto-detection
    let current_dir = std::env::current_dir()
        .context("Failed to get current directory")?;

    let detected = detect::detect_project(&current_dir)
        .context("Failed to detect project type")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No antra.toml found and could not detect project type.\n\n\
                 Supported frameworks:\n\
                 • Node.js (package.json)\n\
                 • Rust (Cargo.toml)\n\
                 • Go (go.mod)\n\
                 • Python (pyproject.toml)\n\
                 • Ruby (Gemfile)\n\
                 • Elixir (mix.exs)\n\
                 • PHP (composer.json)\n\n\
                 Create an antra.toml for manual configuration:\n\n\
                 domain = \"myapp.localhost\"\n\n\
                 [server]\n\
                 command = \"pnpm\"\n\
                 args = [\"dev\"]\n\
                 port = 5173"
            )
        })?;

    output::print_success(&format!(
        "Detected {} project: {}",
        detected.framework, detected.name
    ));

    // Build domain from project name
    let domain = args.domain.unwrap_or_else(|| {
        format!("{}.localhost", detected.name)
    });

    // Build command
    let mut command_parts = vec![detected.command];
    command_parts.extend(detected.args);

    // Determine port
    let port = args.port.or(detected.default_port);

    let run_args = run::RunArgs {
        domain,
        port,
        tld: None,
        allow_custom_domain: false,
        no_trust_prompt: args.no_trust_prompt,
        yes: false,
        force: false,
        command: command_parts,
    };

    run::execute(run_args)
}
