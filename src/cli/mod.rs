pub mod add;
pub mod alias;
pub mod clean;
pub mod dev;
pub mod doctor;
pub mod hosts;
pub mod list;
pub mod open;
pub mod proxy;
pub mod prune;
pub mod run;
pub mod service;
pub mod trust;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::IpcPayload;
use crate::resolver::util::select_resolver;
use crate::util::output;

/// Ensure the daemon is running, starting it if necessary.
/// Returns Ok(true) if daemon was already running, Ok(false) if we started it.
pub(crate) fn ensure_daemon() -> Result<bool> {
    if is_daemon_running() {
        return Ok(true);
    }

    output::print_warning("Daemon not running, starting it...");

    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("proxy")
        .arg("start")
        .env("ANTRA_DAEMON", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null());

    let child = cmd.spawn()?;
    let _child_pid = child.id();

    // Wait for daemon to start
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if is_daemon_running() {
            output::print_success("Daemon started");
            return Ok(false);
        }
    }

    anyhow::bail!("Daemon failed to start within timeout");
}

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

    /// Enable verbose debug output
    #[arg(long, short, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a command behind a proxied domain
    Run(run::RunArgs),

    /// Auto-detect project and run dev server (zero-config)
    Dev(dev::DevArgs),

    /// Add a route to an existing running server (no process spawned)
    Add(add::AddArgs),

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

        /// Install to user login keychain (no sudo needed, macOS only)
        #[arg(long)]
        user_level: bool,
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

    /// Kill orphaned dev servers from crashed sessions
    Prune,

    /// Manage /etc/hosts entries for Safari compatibility
    Hosts {
        #[command(subcommand)]
        command: hosts::HostsCommands,
    },

    /// Manage Antra as a system service
    Service {
        #[command(subcommand)]
        command: service::ServiceCommands,
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
            Commands::Add(args) => add::execute(args),
            Commands::List => {
                let _ = ensure_daemon();
                list::execute()
            }
            Commands::Doctor => doctor::execute(),
            Commands::Trust {
                status,
                remove,
                yes,
                user_level,
            } => trust::execute(status, remove, yes, user_level),
            Commands::Proxy { command } => proxy::execute(command),
            Commands::Clean { yes } => clean::execute(yes),
            Commands::Alias { domain, port } => {
                let _ = ensure_daemon();
                alias::execute(&domain, port)
            }
            Commands::Open { domain } => {
                let _ = ensure_daemon();
                open::execute(&domain)
            }
            Commands::Remove { domain } => {
                let _ = ensure_daemon();
                println!("  {} Removing route for {}", "→".cyan().bold(), domain);

                // Check if route exists before attempting removal
                let mut route_found = false;
                if let Ok(resp) = send_command_sync(IpcPayload::ListRoutes) {
                    if let IpcPayload::RoutesList(list) = resp.payload {
                        if list.routes.iter().any(|r| r.domain == *domain) {
                            route_found = true;
                        }
                    }
                }

                if !route_found {
                    println!(
                        "  {} {}",
                        "⚠".yellow().bold(),
                        format!("No route found for '{domain}'").yellow()
                    );
                    std::process::exit(1);
                }

                // Unregister route via IPC
                use crate::ipc::protocol::UnregisterRouteRequest;
                match send_command_sync(IpcPayload::UnregisterRoute(UnregisterRouteRequest {
                    domain: domain.clone(),
                })) {
                    Ok(_) => {}
                    Err(e) => {
                        println!(
                            "  {} {}",
                            "✗".red().bold(),
                            format!("Failed to remove route: {e}").red()
                        );
                        std::process::exit(1);
                    }
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
            Commands::Prune => {
                let _ = ensure_daemon();
                prune::execute()
            }
            Commands::Hosts { command } => hosts::execute(command),
            Commands::Service { command } => service::execute(command),
        }
    }
}
