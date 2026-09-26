use std::path::PathBuf;

use crate::resolver::hosts::{self, hosts_path};
use crate::resolver::traits::DomainResolver;
use crate::routing::types::ResolutionStatus;

/// Resolver for custom (non-.localhost, non-.test) domains.
/// Requires explicit --allow-custom-domain flag (enforced at CLI level).
/// Validates domains and manages /etc/hosts entries.
pub struct CustomResolver {
    hosts_path: PathBuf,
    allow_custom: bool,
}

impl CustomResolver {
    pub fn new() -> Self {
        Self {
            hosts_path: hosts_path(),
            allow_custom: false,
        }
    }

    pub fn with_custom_domain_allowed(mut self, allow: bool) -> Self {
        self.allow_custom = allow;
        self
    }

    #[allow(dead_code)]
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            hosts_path: path,
            allow_custom: false,
        }
    }

    pub fn validate_domain_with(domain: &str, allow_custom: bool) -> anyhow::Result<()> {
        super::util::validate_domain_shape(domain)?;

        if domain.eq_ignore_ascii_case("localhost") {
            anyhow::bail!("'localhost' already resolves natively — no hosts entry needed");
        }

        if super::util::is_custom_domain(domain) && !allow_custom {
            anyhow::bail!(
                "Custom domain '{domain}' requires explicit --allow-custom-domain approval"
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
        Self::validate_domain_with(domain, self.allow_custom)?;

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
    fn test_validate_custom_domain_requires_approval() {
        assert!(CustomResolver::validate_domain_with("myapp.custom", false).is_err());
        assert!(CustomResolver::validate_domain_with("myapp.custom", true).is_ok());
        assert!(CustomResolver::validate_domain_with("dev.local", false).is_ok());
    }

    #[test]
    fn test_unregister_custom_domain_does_not_require_approval() {
        let dir = tempfile::tempdir().unwrap();
        let hosts_file = dir.path().join("hosts");
        std::fs::write(&hosts_file, "").unwrap();
        let resolver = CustomResolver::with_path(hosts_file);
        assert!(resolver.unregister("api.example.com").is_ok());
    }
}
