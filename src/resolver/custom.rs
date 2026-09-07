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
    allow_public: bool,
}

impl CustomResolver {
    pub fn new() -> Self {
        Self {
            hosts_path: hosts_path(),
            allow_public: false,
        }
    }

    /// Permit known-public domains (explicit user opt-in via --allow-custom-domain).
    pub fn with_allow_public(mut self, allow: bool) -> Self {
        self.allow_public = allow;
        self
    }

    #[allow(dead_code)]
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            hosts_path: path,
            allow_public: false,
        }
    }

    /// Validate that a domain is safe to register.
    /// Returns Ok(()) if safe, or an error with a reason.
    /// Pass `allow_public = true` only via explicit `--allow-custom-domain`.
    pub fn validate_domain_with(domain: &str, allow_public: bool) -> anyhow::Result<()> {
        // Shape first: reject garbage before any policy checks or writes.
        super::util::validate_domain_shape(domain)?;

        // Reject bare localhost
        if domain == "localhost" {
            anyhow::bail!("'localhost' already resolves natively — no hosts entry needed");
        }

        // Reject known public domains (unless explicitly allowed)
        if BLOCKED_DOMAINS.contains(&domain) && !allow_public {
            anyhow::bail!(
                "'{domain}' is a known public domain. Refusing to route locally.\n\
                 To override (not recommended): antra run --domain '{domain}' --allow-custom-domain -- <command>"
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
        Self::validate_domain_with(domain, self.allow_public)?;

        let content = hosts::read_hosts(&self.hosts_path)?;
        let content = hosts::ensure_managed_block(&content);
        let (content, added) = hosts::add_to_managed_block(&content, domain);

        if added {
            hosts::write_hosts_with_hint(&self.hosts_path, &content, domain)?;
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
            hosts::write_hosts_with_hint(&self.hosts_path, &content, domain)?;
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
        assert!(CustomResolver::validate_domain_with("localhost", false).is_err());
    }

    #[test]
    fn test_validate_public_domain_rejected() {
        assert!(CustomResolver::validate_domain_with("google.com", false).is_err());
        assert!(CustomResolver::validate_domain_with("github.com", false).is_err());
    }

    #[test]
    fn test_validate_public_domain_override() {
        assert!(CustomResolver::validate_domain_with("google.com", true).is_ok());
    }

    #[test]
    fn test_validate_custom_domain_allowed() {
        assert!(CustomResolver::validate_domain_with("myapp.custom", false).is_ok());
        assert!(CustomResolver::validate_domain_with("dev.local", false).is_ok());
    }
}
