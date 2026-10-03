use std::collections::HashMap;
use std::sync::RwLock;

use crate::routing::persist::AliasEntry;
use crate::routing::types::Route;

pub struct RouteRegistry {
    routes: RwLock<HashMap<String, Route>>,
    /// Persist unmanaged (static alias) routes to disk so they survive
    /// daemon restarts. Disabled for ephemeral (test) registries so unit
    /// tests never touch the user's real state directory.
    persist: bool,
}

/// Snapshot routes to disk so they survive daemon restarts.
///
/// - Static aliases (`managed=false`) are always persisted.
/// - Managed `run`/`dev` routes (`managed=true`) persist only with a real
///   child PID so a restart can keep them when the owner is still alive and
///   drop them when it is gone. Pid-less managed routes are zombies from a
///   pre-spawn registration and must never reach disk.
///
/// Hot path (`lookup`/`list`) never touches disk; only register/unregister
/// write, and restore reads once at daemon start.
fn persist_snapshot(routes: &HashMap<String, Route>) -> Vec<AliasEntry> {
    routes
        .values()
        .filter(|r| !r.managed || r.pid.is_some())
        .map(|r| AliasEntry {
            domain: r.domain.clone(),
            port: r.port,
            pid: r.pid,
            managed: r.managed,
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
            persist_snapshot(&routes)
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
            persist_snapshot(&routes)
        };
        self.maybe_persist(&snapshot);
        Ok(())
    }

    /// Remove managed routes whose owner process has exited, and return them.
    ///
    /// `antra run` unregisters its route when its child exits or on Ctrl+C,
    /// but a `SIGKILL` or a closed terminal gives it no chance to: the route
    /// then outlived its process, `antra list` and `doctor` kept counting it
    /// as live, and it held the daemon's idle shutdown off forever. Static
    /// routes (`alias`, `add`) have no owner and are never touched. `alive`
    /// is a parameter so the rule can be tested without real processes.
    pub fn reap_dead_owners(&self, alive: impl Fn(u32) -> bool) -> Vec<Route> {
        // Probe outside the write lock: on Windows `alive` spawns `tasklist`.
        let suspects: Vec<(String, u32)> = self
            .list()
            .into_iter()
            .filter(|r| r.managed)
            .filter_map(|r| r.pid.map(|pid| (r.domain, pid)))
            .filter(|(_, pid)| !alive(*pid))
            .collect();
        if suspects.is_empty() {
            return Vec::new();
        }
        let (reaped, snapshot) = {
            let mut routes = self.routes.write().unwrap_or_else(|e| e.into_inner());
            let mut reaped = Vec::new();
            for (domain, pid) in suspects {
                // Only if it is still the same owner: `antra run` may have
                // re-registered the domain for a new process since the probe.
                if routes.get(&domain).is_some_and(|r| r.pid == Some(pid)) {
                    reaped.extend(routes.remove(&domain));
                }
            }
            (reaped, persist_snapshot(&routes))
        };
        if !reaped.is_empty() {
            self.maybe_persist(&snapshot);
        }
        reaped
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::types::Protocol;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Instant;

    fn route(domain: &str, port: u16, pid: Option<u32>, managed: bool) -> Route {
        Route {
            domain: domain.to_string(),
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
            pid,
            managed,
            protocol: Protocol::Http,
            created_at: Instant::now(),
        }
    }

    #[test]
    fn persist_snapshot_keeps_static_and_managed_with_pid() {
        let mut map = HashMap::new();
        map.insert(
            "static.localhost".to_string(),
            route("static.localhost", 3000, None, false),
        );
        map.insert(
            "run.localhost".to_string(),
            route("run.localhost", 5173, Some(1234), true),
        );
        let snap = persist_snapshot(&map);
        assert_eq!(snap.len(), 2);
        let managed = snap.iter().find(|e| e.domain == "run.localhost").unwrap();
        assert!(managed.managed);
        assert_eq!(managed.pid, Some(1234));
        let statics: Vec<_> = snap.iter().filter(|e| !e.managed).collect();
        assert_eq!(statics.len(), 1);
        assert_eq!(statics[0].domain, "static.localhost");
    }

    fn registry_with(routes: &[Route]) -> RouteRegistry {
        let registry = RouteRegistry::new_ephemeral();
        for r in routes {
            registry.register(r.clone()).unwrap();
        }
        registry
    }

    #[test]
    fn reaper_removes_only_managed_routes_with_a_dead_owner() {
        let registry = registry_with(&[
            route("dead.localhost", 4000, Some(111), true),
            route("live.localhost", 4001, Some(222), true),
            route("static.localhost", 3000, None, false),
            // Legacy unmanaged route that happens to carry a pid: static.
            route("legacy.localhost", 3001, Some(111), false),
        ]);
        let reaped = registry.reap_dead_owners(|pid| pid != 111);
        let reaped: Vec<_> = reaped.iter().map(|r| r.domain.as_str()).collect();
        assert_eq!(reaped, vec!["dead.localhost"]);
        let mut left: Vec<_> = registry.list().into_iter().map(|r| r.domain).collect();
        left.sort();
        assert_eq!(
            left,
            vec!["legacy.localhost", "live.localhost", "static.localhost"]
        );
    }

    #[test]
    fn reaper_with_every_owner_alive_changes_nothing() {
        let registry = registry_with(&[route("live.localhost", 4001, Some(222), true)]);
        assert!(registry.reap_dead_owners(|_| true).is_empty());
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn reaper_spares_a_domain_taken_over_since_the_probe() {
        // The probe runs outside the lock. If `antra run` re-registers the
        // domain for a new process in between, the new route must survive.
        let registry = registry_with(&[route("app.localhost", 4000, Some(111), true)]);
        let reaped = registry.reap_dead_owners(|pid| {
            registry
                .register(route("app.localhost", 4002, Some(333), true))
                .unwrap();
            pid != 111
        });
        assert!(reaped.is_empty());
        assert_eq!(registry.lookup("app.localhost").unwrap().pid, Some(333));
    }

    #[test]
    fn managed_route_without_pid_still_excluded() {
        // Defensive: even a pid-less managed route must never reach disk.
        let mut map = HashMap::new();
        map.insert(
            "orphan.localhost".to_string(),
            route("orphan.localhost", 4000, None, true),
        );
        assert!(persist_snapshot(&map).is_empty());
    }
}
