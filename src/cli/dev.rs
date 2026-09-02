use anyhow::{Context, Result};
use clap::Args;

use crate::config::project::{config_path, load_project_config};
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
    let config = load_project_config()
        .with_context(|| format!("Failed to read {}", config_path().display()))?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No {} found in current directory.\n\n\
                 Create one with:\n\n\
                 domain = \"myapp.localhost\"\n\n\
                 [server]\n\
                 command = \"pnpm\"\n\
                 args = [\"dev\"]\n\
                 port = 5173",
                config_path().display()
            )
        })?;

    output::print_success(&format!("Loaded {}", config_path().display()));

    let mut command_parts = vec![config.server.command.clone()];
    command_parts.extend(config.server.args.clone());

    let run_args = run::RunArgs {
        domain: args.domain.unwrap_or_else(|| config.domain.clone()),
        port: args.port.or(config.server.port),
        allow_custom_domain: config.server.allow_custom_domain,
        no_trust_prompt: args.no_trust_prompt,
        command: command_parts,
    };

    run::execute(run_args)
}
