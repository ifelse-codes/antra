use crate::routing::types::ResolutionStatus;

pub trait DomainResolver: Send + Sync {
    /// Register a domain for local resolution
    fn register(&self, domain: &str) -> anyhow::Result<()>;

    /// Unregister a domain from local resolution
    fn unregister(&self, domain: &str) -> anyhow::Result<()>;

    /// Check the resolution status of a domain
    #[allow(dead_code)]
    fn status(&self, domain: &str) -> anyhow::Result<ResolutionStatus>;
}
