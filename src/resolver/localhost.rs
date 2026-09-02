use crate::resolver::traits::DomainResolver;
use crate::routing::types::ResolutionStatus;

/// Resolver for .localhost domains.
/// No-op: browsers resolve *.localhost natively to 127.0.0.1.
pub struct LocalhostResolver;

impl DomainResolver for LocalhostResolver {
    fn register(&self, _domain: &str) -> anyhow::Result<()> {
        // No-op: browsers handle this natively
        Ok(())
    }

    fn unregister(&self, _domain: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn status(&self, _domain: &str) -> anyhow::Result<ResolutionStatus> {
        Ok(ResolutionStatus::Active)
    }
}
