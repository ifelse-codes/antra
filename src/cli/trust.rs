use anyhow::Result;
use colored::Colorize;

use crate::trust;

/// What a set of `antra trust` flags resolves to.
///
/// Split out from `execute` because the bug this replaced was a routing bug:
/// `--remove` was matched before `--yes` was read, so `antra trust --remove
/// --yes` fell into the interactive prompt, read EOF from a script's stdin, and
/// exited 0 with "Skipped. CA remains trusted." Deciding the action as a pure
/// function of the flags is what makes that order testable without a trust
/// store, a keychain, or a TTY.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TrustAction {
    Status,
    /// Remove, asking first.
    RemovePrompted,
    /// Remove without asking — `antra trust --remove --yes`.
    RemoveNonInteractive,
    /// Install into the login keychain (macOS, no sudo).
    InstallUserLevel,
    /// Install into the system store without asking.
    InstallNonInteractive,
    /// Install into the system store, asking first.
    InstallPrompted,
}

pub(crate) fn action_for(status: bool, remove: bool, yes: bool, user_level: bool) -> TrustAction {
    // `--status` wins over everything: it reports and changes nothing, so
    // combining it with a mutating flag must not mutate.
    if status {
        return TrustAction::Status;
    }
    if remove {
        return if yes {
            TrustAction::RemoveNonInteractive
        } else {
            TrustAction::RemovePrompted
        };
    }
    if user_level {
        return TrustAction::InstallUserLevel;
    }
    if yes {
        TrustAction::InstallNonInteractive
    } else {
        TrustAction::InstallPrompted
    }
}

pub fn execute(status: bool, remove: bool, yes: bool, user_level: bool) -> Result<()> {
    // Under sudo, act on the invoking user's CA, not root's (C28). `sudo`
    // on Linux resets HOME to /root, so without this `sudo antra trust`
    // mints a second CA under /root and installs *that* — while the user's
    // daemon keeps serving the user's CA and `trust --status` stays red.
    // Before anything else: it sets environment variables, and no thread
    // exists yet (same rule as `proxy start` under C27).
    #[cfg(unix)]
    if let Some(user) = crate::platform::sudo::invoking_user() {
        crate::platform::sudo::adopt_invoking_user_paths(&user);
    }
    let action = action_for(status, remove, yes, user_level);
    // A root process acting in the user's HOME must not leave root-owned
    // config dirs behind: the next unprivileged run could not write its
    // own CA. Status only reads, so it takes no state.
    #[cfg(unix)]
    if action != TrustAction::Status {
        crate::trust::hand_back_config_dirs()?;
    }
    match action {
        TrustAction::Status => show_status(),
        TrustAction::RemovePrompted => {
            println!("{}", "ANTRA TRUST — Remove".bold());
            println!();
            trust::remove_ca()
        }
        TrustAction::RemoveNonInteractive => {
            println!("{}", "ANTRA TRUST — Remove".bold());
            println!();
            trust::remove_ca_noninteractive()
        }
        TrustAction::InstallUserLevel => {
            println!("{}", "ANTRA TRUST — Install (User Keychain)".bold());
            println!();
            trust::install_ca_user_level()
        }
        TrustAction::InstallNonInteractive => {
            println!("{}", "ANTRA TRUST — Install".bold());
            println!();
            trust::install_ca_noninteractive()
        }
        TrustAction::InstallPrompted => {
            println!("{}", "ANTRA TRUST — Install".bold());
            println!();
            trust::install_ca()
        }
    }
}

fn show_status() -> Result<()> {
    println!("{}", "ANTRA TRUST — Status".bold());
    println!();

    let installed = trust::check_trust_status()?;

    if installed {
        println!("  {} {}", "✓".green(), "CA is trusted".green());
    } else if trust::check_user_level_trust()? {
        println!(
            "  {} {}",
            "✓".green(),
            "CA is trusted via your login keychain (user-level, no sudo)".green()
        );
        println!();
        println!(
            "  {}",
            "HTTPS works with no warnings. For system-wide trust, run: sudo antra trust".dimmed()
        );
    } else {
        println!(
            "  {} {}",
            "✗".red(),
            "CA is NOT trusted by the system".red()
        );
        println!();
        #[cfg(target_os = "macos")]
        println!(
            "  Run {} (no sudo) or {} (system-wide).",
            "antra trust --user-level".cyan(),
            "sudo antra trust".cyan()
        );
        #[cfg(not(target_os = "macos"))]
        println!("  Run {} to install the CA.", "antra trust".cyan());
        if trust::retired_ca_pending() {
            println!();
            println!(
                "  {}",
                "A CA Antra replaced is still installed in a trust store.".yellow()
            );
            println!(
                "  Run {} to trust the current one and remove the old.",
                "antra trust".bold()
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // `antra trust --remove --yes` used to ignore `--yes`. The route was
    // `if remove { remove_ca() } else if ... { if yes { ... } }`, so `--yes`
    // was only ever consulted on the install path. With stdin not a TTY the
    // prompt read returned EOF, the answer was neither "y" nor "yes", and the
    // command printed "Skipped. CA remains trusted." and exited 0.
    //
    // These are unit tests on purpose. An end-to-end version was written and
    // removed: in a hermetic test home there is no ca.pem, so both the prompted
    // and the non-interactive path print "ca.pem not found" and exit 0 before
    // reaching the prompt. It passed against the broken code — a green check
    // that could not fail, which is the failure mode AGENT.md warns about.
    // Asserting the prompt's absence needs a trusted CA, and a trusted CA means
    // the real system store, so it cannot be tested hermetically. Pinning the
    // routing is what is actually testable here, and the routing was the bug.
    //
    // Each case below was checked against a deliberately inverted `if yes` to
    // confirm it fails: 3 of the 5 went red.

    #[test]
    fn remove_with_yes_is_non_interactive() {
        assert_eq!(
            action_for(false, true, true, false),
            TrustAction::RemoveNonInteractive
        );
    }

    #[test]
    fn remove_with_yes_ignores_user_level() {
        // `--user-level` is an install flag. A script passing all three is
        // removing, not installing into the keychain.
        assert_eq!(
            action_for(false, true, true, true),
            TrustAction::RemoveNonInteractive
        );
    }

    #[test]
    fn remove_without_yes_still_prompts() {
        assert_eq!(
            action_for(false, true, false, false),
            TrustAction::RemovePrompted
        );
    }

    #[test]
    fn install_yes_and_prompted_paths_are_unchanged() {
        assert_eq!(
            action_for(false, false, true, false),
            TrustAction::InstallNonInteractive
        );
        assert_eq!(
            action_for(false, false, false, false),
            TrustAction::InstallPrompted
        );
        assert_eq!(
            action_for(false, false, true, true),
            TrustAction::InstallUserLevel
        );
    }

    #[test]
    fn status_wins_over_every_mutating_flag() {
        // `--status` must report without removing, even alongside `--remove
        // --yes`. Reordering the match to put remove first would make this
        // silently destructive.
        assert_eq!(action_for(true, true, true, true), TrustAction::Status);
    }
}
