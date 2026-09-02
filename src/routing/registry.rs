use std::collections::HashMap;
use std::sync::RwLock;

use crate::routing::types::Route;

pub struct RouteRegistry {
    routes: RwLock<HashMap<String, Route>>,
}

impl RouteRegistry {
    pub fn new() -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, route: Route) -> anyhow::Result<()> {
        let mut routes = self
            .routes
            .write()
            .map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        routes.insert(route.domain.clone(), route);
        Ok(())
    }

    pub fn unregister(&self, domain: &str) -> anyhow::Result<()> {
        let mut routes = self
            .routes
            .write()
            .map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        routes.remove(domain);
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
