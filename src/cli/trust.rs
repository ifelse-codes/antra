use anyhow::Result;
use colored::Colorize;

use crate::trust;

pub fn execute(status: bool, remove: bool, yes: bool, user_level: bool) -> Result<()> {
    if status {
        return show_status();
    }

    if remove {
        println!("{}", "ANTRA TRUST — Remove".bold());
        println!();
        trust::remove_ca()
    } else if user_level {
        println!("{}", "ANTRA TRUST — Install (User Keychain)".bold());
        println!();
        trust::install_ca_user_level()
    } else if yes {
        println!("{}", "ANTRA TRUST — Install".bold());
        println!();
        trust::install_ca_noninteractive()
    } else {
        println!("{}", "ANTRA TRUST — Install".bold());
        println!();
        trust::install_ca()
    }
}

fn show_status() -> Result<()> {
    println!("{}", "ANTRA TRUST — Status".bold());
    println!();

    let installed = trust::check_trust_status()?;

    if installed {
        println!(
            "  {} {}",
            "✓".green(),
            "CA is trusted by the system".green()
        );
    } else {
        println!(
            "  {} {}",
            "✗".red(),
            "CA is NOT trusted by the system".red()
        );
        println!();
        println!("  Run {} to install the CA.", "antra trust".cyan());
    }

    Ok(())
}
