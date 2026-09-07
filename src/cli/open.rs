use anyhow::Result;
use colored::Colorize;

use crate::util::output;

pub fn execute(domain: &str) -> Result<()> {
    // Open the URL the user can actually reach: includes the fallback
    // port (e.g. :8443) when the daemon isn't on 443.
    let url = output::route_url(domain);

    println!("  {} Opening {}", "→".cyan().bold(), url.underline());

    open::that(&url).map_err(|e| anyhow::anyhow!("Failed to open browser: {e}"))?;

    Ok(())
}
