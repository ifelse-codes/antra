use anyhow::Result;

use crate::resolver::traits::DomainResolver;

/// Select the appropriate resolver based on the domain suffix.
///
/// - `.localhost` domains use `LocalhostResolver` (browser-native, no hosts file)
/// - `.test` domains use `HostsResolver` (managed hosts block)
/// - `.internal`/`.local` domains use `HostsResolver` with a warning
/// - Custom domains use `CustomResolver` (validates against public domain blocklist)
pub fn select_resolver(domain: &str) -> Result<Box<dyn DomainResolver>> {
    if domain == "localhost" || domain.ends_with(".localhost") {
        Ok(Box::new(crate::resolver::localhost::LocalhostResolver))
    } else if domain.ends_with(".test") {
        Ok(Box::new(crate::resolver::test::HostsResolver::new()))
    } else if domain.ends_with(".internal") || domain.ends_with(".local") {
        // Warn but allow
        tracing::warn!(%domain, "Using .internal/.local domain — ensure DNS resolves to 127.0.0.1");
        Ok(Box::new(crate::resolver::test::HostsResolver::new()))
    } else {
        // Custom domain — validation happens inside CustomResolver
        Ok(Box::new(crate::resolver::custom::CustomResolver::new()))
    }
}
