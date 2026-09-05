use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

use crate::ipc::client::send_command_sync;
use crate::ipc::protocol::IpcPayload;
use crate::resolver::util::select_resolver;
use crate::util::output;

#[derive(Args)]
pub struct AddArgs {
    #[command(subcommand)]
    pub command: Option<AddCommands>,

    /// Domain to alias (e.g., myapp.localhost) — used with implicit route mode
    #[arg(long)]
    pub domain: Option<String>,

    /// Port the server is listening on — used with implicit route mode
    #[arg(long)]
    pub port: Option<u16>,

    /// Custom TLD (e.g., dev.example.com for myapp.dev.example.com)
    #[arg(long)]
    pub tld: Option<String>,
}

#[derive(Subcommand)]
pub enum AddCommands {
    /// Add a route to an existing running server (no process spawned)
    Route {
        /// Domain to alias (e.g., myapp.localhost)
        #[arg(long)]
        domain: String,

        /// Port the server is listening on
        #[arg(long)]
        port: u16,

        /// Custom TLD
        #[arg(long)]
        tld: Option<String>,
    },

    /// Wrap a package.json script to run through antra
    WrapScript {
        /// Name for the antra route (e.g., myapp)
        name: String,

        /// The dev command to wrap (e.g., "npm run dev")
        #[arg(long)]
        command: String,

        /// Port the dev server uses (e.g., 3000)
        #[arg(long)]
        port: u16,

        /// Overwrite existing antra script if present
        #[arg(long)]
        force: bool,
    },
}

pub fn execute(args: AddArgs) -> Result<()> {
    match args.command {
        Some(AddCommands::Route { domain, port, tld }) => {
            execute_route(AddRouteArgs { domain, port, tld })
        }
        Some(AddCommands::WrapScript {
            name,
            command,
            port,
            force,
        }) => execute_wrap_script(WrapScriptArgs {
            name,
            command,
            port,
            force,
        }),
        None => {
            // Legacy mode: use top-level args
            match (args.domain, args.port) {
                (Some(domain), Some(port)) => execute_route(AddRouteArgs {
                    domain,
                    port,
                    tld: args.tld,
                }),
                _ => {
                    output::print_error("Please specify --domain and --port, or use a subcommand:");
                    println!("  antra add route --domain myapp.localhost --port 3000");
                    println!("  antra add wrap-script myapp --command \"npm run dev\" --port 3000");
                    std::process::exit(1);
                }
            }
        }
    }
}

struct AddRouteArgs {
    domain: String,
    port: u16,
    tld: Option<String>,
}

fn execute_route(args: AddRouteArgs) -> Result<()> {
    output::print_header();

    let domain = if let Some(tld) = &args.tld {
        let app_name = args.domain.split('.').next().unwrap_or(&args.domain);
        format!("{app_name}.{tld}")
    } else {
        args.domain.clone()
    };

    // Resolve domain to 127.0.0.1
    let resolver = select_resolver(&domain)?;
    resolver.register(&domain)?;
    output::print_success(&format!("Domain resolved: {}", domain));

    // Ensure daemon is running
    let _ = super::ensure_daemon();

    // Register route via IPC
    match send_command_sync(IpcPayload::RegisterRoute(
        crate::ipc::protocol::RegisterRouteRequest {
            domain: domain.clone(),
            port: args.port,
            pid: None,
        },
    )) {
        Ok(msg) => match msg.payload {
            IpcPayload::Ok(ok) => {
                output::print_success(&ok.message);
            }
            IpcPayload::Error(err) => {
                output::print_error(&err.message);
                std::process::exit(1);
            }
            other => {
                output::print_error(&format!("Unexpected response: {other:?}"));
                std::process::exit(1);
            }
        },
        Err(e) => {
            output::print_error(&format!("Failed to register route: {e}"));
            std::process::exit(1);
        }
    }

    println!();
    if let Ok(status) = crate::ipc::client::get_startup_status() {
        if status.https_port != 443 {
            println!("  {} Note: HTTPS on port {}", "ℹ".cyan(), status.https_port);
            let host = if domain.ends_with(".localhost") {
                domain.clone()
            } else {
                format!("{}.localhost", domain)
            };
            println!("  → https://{}:{}", host, status.https_port);
        } else {
            println!("  → https://{}", domain);
        }
    } else {
        println!("  → https://{}", domain);
    }
    println!();

    output::print_success(&format!(
        "Added route: {} → port {} (no process spawned)",
        domain, args.port
    ));

    Ok(())
}

struct WrapScriptArgs {
    name: String,
    command: String,
    port: u16,
    force: bool,
}

fn execute_wrap_script(args: WrapScriptArgs) -> Result<()> {
    output::print_header();

    let package_json_path = find_package_json()?;

    let content = fs::read_to_string(&package_json_path)?;
    let mut package_json: serde_json::Value = serde_json::from_str(&content)?;

    // Get or create scripts object
    let scripts = package_json
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("package.json is not a JSON object"))?
        .entry("scripts")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("scripts is not a JSON object"))?;

    let antra_script = format!(
        "antra run --domain {}.localhost --port {} -- {}",
        args.name, args.port, args.command
    );
    let key = format!("antra:{}", args.name);

    if scripts.contains_key(&key) && !args.force {
        output::print_error(&format!(
            "Script '{}' already exists in package.json. Use --force to overwrite.",
            key
        ));
        std::process::exit(1);
    }

    scripts.insert(key.clone(), serde_json::Value::String(antra_script.clone()));

    // Write back with proper formatting
    let updated = serde_json::to_string_pretty(&package_json)?;
    fs::write(&package_json_path, updated)?;

    println!();
    output::print_success(&format!(
        "Added script '{}' to {}",
        key,
        package_json_path.display()
    ));
    println!();
    println!(
        "  {} Run with: {}",
        "→".cyan().bold(),
        format!("npm run {key}").bold()
    );
    println!(
        "  {} Or directly: {}",
        "→".cyan().bold(),
        antra_script.bold()
    );
    println!();

    Ok(())
}

fn find_package_json() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        let path = dir.join("package.json");
        if path.exists() {
            return Ok(path);
        }
        if !dir.pop() {
            break;
        }
    }
    anyhow::bail!("No package.json found in current directory or any parent directory")
}
