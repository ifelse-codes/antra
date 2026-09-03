pub mod alias;
pub mod clean;
pub mod dev;
pub mod doctor;
pub mod list;
pub mod open;
pub mod proxy;
pub mod run;
pub mod trust;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::IpcPayload;
use crate::resolver::traits::DomainResolver;

#[derive(Parser)]
#[command(
    name = "antra",
    about = "Native local development proxy — stable domains for localhost servers",
    version,
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a command behind a proxied domain
    Run(run::RunArgs),

    /// Run using antra.toml config
    Dev(dev::DevArgs),

    /// List active routes
    List,

    /// Diagnose Antra setup and configuration
    Doctor,

    /// Manage the local development CA trust
    Trust {
        /// Show current trust status
        #[arg(long)]
        status: bool,

        /// Remove the CA from trust store
        #[arg(long)]
        remove: bool,

        /// Skip prompts and auto-install
        #[arg(short, long)]
        yes: bool,
    },

    /// Manage the Antra proxy daemon
    Proxy {
        #[command(subcommand)]
        command: ProxyCommands,
    },

    /// Remove all Antra state
    Clean {
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// Create a static domain alias to a port
    Alias {
        /// Domain to alias (e.g., api.myapp.localhost)
        domain: String,

        /// Target port (e.g., 8080)
        port: u16,
    },

    /// Open a domain in the default browser
    Open {
        /// Domain to open
        domain: String,
    },

    /// Remove a route or alias
    Remove {
        /// Domain to remove
        domain: String,
    },
}

#[derive(Subcommand)]
pub enum Commands2 {
    /// Stop a running route
    Stop {
        /// Domain to stop
        domain: String,
    },
}

#[derive(Subcommand)]
pub enum ProxyCommands {
    /// Start the proxy daemon
    Start {
        /// Port for HTTPS (default: 443)
        #[arg(long, default_value = "443")]
        port: u16,

        /// Port for HTTP redirect (default: 80)
        #[arg(long, default_value = "80")]
        http_port: u16,

        /// Static route in domain:port format (can be repeated)
        #[arg(long = "route", action = clap::ArgAction::Append)]
        routes: Vec<String>,
    },

    /// Stop the proxy daemon
    Stop,

    /// Show proxy daemon status
    Status,
}

impl Cli {
    pub fn execute(self) -> Result<()> {
        match self.command {
            Commands::Run(args) => run::execute(args),
            Commands::Dev(args) => dev::execute(args),
            Commands::List => list::execute(),
            Commands::Doctor => doctor::execute(),
            Commands::Trust {
                status,
                remove,
                yes,
            } => trust::execute(status, remove, yes),
            Commands::Proxy { command } => proxy::execute(command),
            Commands::Clean { yes } => clean::execute(yes),
            Commands::Alias { domain, port } => alias::execute(&domain, port),
            Commands::Open { domain } => open::execute(&domain),
            Commands::Remove { domain } => {
                println!("  {} Removing route for {}", "→".cyan().bold(), domain);

                // Unregister route via IPC
                if is_daemon_running() {
                    use crate::ipc::protocol::UnregisterRouteRequest;
                    let _ =
                        send_command_sync(IpcPayload::UnregisterRoute(UnregisterRouteRequest {
                            domain: domain.clone(),
                        }));
                }

                // Unresolve domain (remove from hosts if needed)
                let resolver = select_resolver(&domain)?;
                resolver.unregister(&domain)?;
                println!(
                    "  {} {}",
                    "✓".green().bold(),
                    format!("Route removed: {domain}").green()
                );
                Ok(())
            }
        }
    }
}

fn select_resolver(domain: &str) -> Result<Box<dyn DomainResolver>> {
    if domain == "localhost" || domain.ends_with(".localhost") {
        Ok(Box::new(crate::resolver::localhost::LocalhostResolver))
    } else if domain.ends_with(".test") {
        Ok(Box::new(crate::resolver::test::HostsResolver::new()))
    } else {
        Ok(Box::new(crate::resolver::custom::CustomResolver::new()))
    }
}
