use std::collections::HashMap;
use std::sync::RwLock;

use crate::routing::persist::AliasEntry;
use crate::routing::types::Route;

pub struct RouteRegistry {
    routes: RwLock<HashMap<String, Route>>,
    /// Persist PID-less (static alias) routes to disk so they survive
    /// daemon restarts. Disabled for ephemeral (test) registries so unit
    /// tests never touch the user's real state directory.
    persist: bool,
}

/// Snapshot the PID-less (static alias) routes to disk so they survive
/// daemon restarts. Managed `run`/`dev` routes carry a PID and are
/// intentionally excluded — they die with their process.
fn static_alias_snapshot(routes: &HashMap<String, Route>) -> Vec<AliasEntry> {
    routes
        .values()
        .filter(|r| r.pid.is_none())
        .map(|r| AliasEntry {
            domain: r.domain.clone(),
            port: r.port,
        })
        .collect()
}

impl RouteRegistry {
    /// Production registry: static aliases are persisted to disk.
    pub fn new() -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            persist: true,
        }
    }

    /// Non-persisting registry for tests and throwaway use.
    /// (Only exercised by integration tests, hence the allow.)
    #[allow(dead_code)]
    pub fn new_ephemeral() -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            persist: false,
        }
    }

    /// Write the snapshot unless this is an ephemeral registry.
    fn maybe_persist(&self, snapshot: &[AliasEntry]) {
        if self.persist {
            crate::routing::persist::save_aliases(snapshot);
        }
    }

    pub fn register(&self, route: Route) -> anyhow::Result<()> {
        // Snapshot under the lock, persist after it's released.
        let snapshot = {
            let mut routes = self
                .routes
                .write()
                .map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
            routes.insert(route.domain.clone(), route);
            static_alias_snapshot(&routes)
        };
        self.maybe_persist(&snapshot);
        Ok(())
    }

    pub fn unregister(&self, domain: &str) -> anyhow::Result<()> {
        let snapshot = {
            let mut routes = self
                .routes
                .write()
                .map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
            routes.remove(domain);
            static_alias_snapshot(&routes)
        };
        self.maybe_persist(&snapshot);
        Ok(())
    }

    pub fn lookup(&self, domain: &str) -> Option<Route> {
        let routes = self.routes.read().ok()?;
        routes.get(domain).cloned()
    }

    pub fn list(&self) -> Vec<Route> {
        let routes = self.routes.read().unwrap_or_else(|e| e.into_inner());
        routes.values().cloned().collect()
    }
}

impl Default for RouteRegistry {
    fn default() -> Self {
        Self::new()
    }
}
