use anyhow::{Context, Result};
use clap::Args;

use crate::config::project::{config_path, load_project_config, split_command_string};
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

        // `server.command` must be one binary, but `command = "python3 -m ..."`
        // is the natural thing to write. When no explicit `args` are given,
        // split a spaced command (shell-like quoting) instead of trying to
        // execute the whole string as one binary. A literal existing path
        // (e.g. "/Applications/My App/server") is kept whole.
        let command_parts = if config.server.args.is_empty()
            && config.server.command.contains(char::is_whitespace)
            && !std::path::Path::new(&config.server.command).exists()
        {
            let split = split_command_string(&config.server.command);
            if split.is_empty() {
                vec![config.server.command.clone()]
            } else {
                output::print_warning(&format!(
                    "Split server.command into program + args: {}",
                    split.join(" ")
                ));
                output::print_warning(
                    "Tip: prefer `command = \"python3\"` with `args = [\"-m\", \"http.server\"]`.",
                );
                split
            }
        } else {
            let mut parts = vec![config.server.command.clone()];
            parts.extend(config.server.args.clone());
            parts
        };

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
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;

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

    // Build domain from project name (sanitized + lowercased at detection;
    // explicit --domain is folded here so MyApp and myapp share one route).
    let domain = args
        .domain
        .map(|d| d.to_ascii_lowercase())
        .unwrap_or_else(|| format!("{}.localhost", detected.name));

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
