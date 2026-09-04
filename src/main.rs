#![allow(dead_code)]

mod certs;
mod cli;
mod config;
mod daemon;
mod ipc;
mod platform;
mod process;
mod proxy;
mod resolver;
mod routing;
mod trust;
mod util;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set tracing level based on --verbose flag
    let filter = if cli.verbose {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug"))
    } else {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();

    cli.execute()
}
