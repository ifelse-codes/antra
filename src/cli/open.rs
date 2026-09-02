use anyhow::Result;
use colored::Colorize;

pub fn execute(domain: &str) -> Result<()> {
    let url = format!("https://{domain}");

    println!("  {} Opening {}", "→".cyan().bold(), url.underline());

    open::that(&url).map_err(|e| anyhow::anyhow!("Failed to open browser: {e}"))?;

    Ok(())
}
