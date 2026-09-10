use std::net::IpAddr;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Route {
    pub domain: String,
    pub host: IpAddr,
    pub port: u16,
    pub pid: Option<u32>,
    /// True for `run`/`dev`-managed routes (die with process, never persisted).
    /// False for static `alias`/`add` routes (persisted to aliases.json).
    pub managed: bool,
    #[allow(dead_code)]
    pub protocol: Protocol,
    pub created_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Http,
    Https,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ResolutionStatus {
    Active,
    Inactive,
    Error,
}
