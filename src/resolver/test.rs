use std::path::PathBuf;

use crate::resolver::hosts::{self, hosts_path};
use crate::resolver::traits::DomainResolver;
use crate::routing::types::ResolutionStatus;

/// Resolver for .test domains via /etc/hosts management.
/// Manages entries within BEGIN/END ANTRA MANAGED HOSTS markers.
pub struct HostsResolver {
    hosts_path: PathBuf,
}

impl HostsResolver {
    pub fn new() -> Self {
        Self {
            hosts_path: hosts_path(),
        }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { hosts_path: path }
    }
}

impl Default for HostsResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainResolver for HostsResolver {
    fn register(&self, domain: &str) -> anyhow::Result<()> {
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
    use tempfile::tempdir;

    #[test]
    fn test_register_and_unregister() {
        let dir = tempdir().unwrap();
        let hosts_file = dir.path().join("hosts");
        std::fs::write(&hosts_file, "127.0.0.1 localhost\n").unwrap();

        let resolver = HostsResolver::with_path(hosts_file.clone());

        // Register
        resolver.register("myapp.test").unwrap();
        let content = std::fs::read_to_string(&hosts_file).unwrap();
        assert!(content.contains("127.0.0.1 myapp.test"));

        // Status
        let status = resolver.status("myapp.test").unwrap();
        assert!(matches!(status, ResolutionStatus::Active));

        // Unregister
        resolver.unregister("myapp.test").unwrap();
        let content = std::fs::read_to_string(&hosts_file).unwrap();
        assert!(!content.contains("myapp.test"));

        // Status after unregister
        let status = resolver.status("myapp.test").unwrap();
        assert!(matches!(status, ResolutionStatus::Inactive));
    }

    #[test]
    fn test_register_preserves_existing_entries() {
        let dir = tempdir().unwrap();
        let hosts_file = dir.path().join("hosts");
        std::fs::write(
            &hosts_file,
            "127.0.0.1 localhost\n255.255.255.255 broadcasthost\n",
        )
        .unwrap();

        let resolver = HostsResolver::with_path(hosts_file.clone());
        resolver.register("myapp.test").unwrap();

        let content = std::fs::read_to_string(&hosts_file).unwrap();
        assert!(content.contains("127.0.0.1 localhost"));
        assert!(content.contains("255.255.255.255 broadcasthost"));
        assert!(content.contains("127.0.0.1 myapp.test"));
    }

    #[test]
    fn test_register_idempotent() {
        let dir = tempdir().unwrap();
        let hosts_file = dir.path().join("hosts");
        std::fs::write(&hosts_file, "").unwrap();

        let resolver = HostsResolver::with_path(hosts_file.clone());
        resolver.register("myapp.test").unwrap();
        resolver.register("myapp.test").unwrap();

        let content = std::fs::read_to_string(&hosts_file).unwrap();
        let count = content.matches("127.0.0.1 myapp.test").count();
        assert_eq!(count, 1);
    }
}
