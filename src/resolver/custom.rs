use std::path::PathBuf;

use crate::resolver::hosts::{self, hosts_path};
use crate::resolver::traits::DomainResolver;
use crate::routing::types::ResolutionStatus;

/// Known public domains that should never be routed locally.
const BLOCKED_DOMAINS: &[&str] = &[
    "google.com",
    "github.com",
    "youtube.com",
    "facebook.com",
    "twitter.com",
    "x.com",
    "instagram.com",
    "linkedin.com",
    "microsoft.com",
    "apple.com",
    "amazon.com",
    "netflix.com",
    "reddit.com",
    "wikipedia.org",
    "stackoverflow.com",
    "npmjs.com",
    "crates.io",
    "docs.rs",
];

/// Resolver for custom (non-.localhost, non-.test) domains.
/// Requires explicit --allow-custom-domain flag (enforced at CLI level).
/// Validates domains and manages /etc/hosts entries.
pub struct CustomResolver {
    hosts_path: PathBuf,
}

impl CustomResolver {
    pub fn new() -> Self {
        Self {
            hosts_path: hosts_path(),
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { hosts_path: path }
    }

    /// Validate that a domain is safe to register.
    /// Returns Ok(()) if safe, or an error with a reason.
    pub fn validate_domain(domain: &str) -> anyhow::Result<()> {
        // Reject bare localhost
        if domain == "localhost" {
            anyhow::bail!("'localhost' already resolves natively — no hosts entry needed");
        }

        // Reject known public domains
        if BLOCKED_DOMAINS.contains(&domain) {
            anyhow::bail!(
                "'{domain}' is a known public domain. Refusing to route locally.\n\
                 Use --allow-public-domain to override (not recommended)."
            );
        }

        // Reject domains that look like they could be production
        if domain.ends_with(".com")
            || domain.ends_with(".org")
            || domain.ends_with(".net")
            || domain.ends_with(".io")
            || domain.ends_with(".dev")
        {
            // Only warn, don't reject — user must have used --allow-custom-domain
            tracing::warn!(
                %domain,
                "Domain looks like a public TLD. Ensure this is intentional."
            );
        }

        Ok(())
    }
}

impl Default for CustomResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainResolver for CustomResolver {
    fn register(&self, domain: &str) -> anyhow::Result<()> {
        Self::validate_domain(domain)?;

        let content = hosts::read_hosts(&self.hosts_path)?;
        let content = hosts::ensure_managed_block(&content);
        let (content, added) = hosts::add_to_managed_block(&content, domain);

        if added {
            hosts::write_hosts_atomic(&self.hosts_path, &content)?;
            tracing::info!(%domain, "Added to hosts file");
        } else {
            tracing::debug!(%domain, "Already in hosts file");
        }

        Ok(())
    }

    fn unregister(&self, domain: &str) -> anyhow::Result<()> {
        let content = hosts::read_hosts(&self.hosts_path)?;
        let (content, removed) = hosts::remove_from_managed_block(&content, domain);

        if removed {
            hosts::write_hosts_atomic(&self.hosts_path, &content)?;
            tracing::info!(%domain, "Removed from hosts file");
        } else {
            tracing::debug!(%domain, "Not found in hosts file");
        }

        Ok(())
    }

    fn status(&self, domain: &str) -> anyhow::Result<ResolutionStatus> {
        let content = hosts::read_hosts(&self.hosts_path)?;
        if hosts::domain_in_managed_block(&content, domain) {
            Ok(ResolutionStatus::Active)
        } else {
            Ok(ResolutionStatus::Inactive)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_localhost_rejected() {
        assert!(CustomResolver::validate_domain("localhost").is_err());
    }

    #[test]
    fn test_validate_public_domain_rejected() {
        assert!(CustomResolver::validate_domain("google.com").is_err());
        assert!(CustomResolver::validate_domain("github.com").is_err());
    }

    #[test]
    fn test_validate_custom_domain_allowed() {
        assert!(CustomResolver::validate_domain("myapp.custom").is_ok());
        assert!(CustomResolver::validate_domain("dev.local").is_ok());
    }
}
