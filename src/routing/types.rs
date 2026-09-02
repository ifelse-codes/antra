use std::net::IpAddr;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Route {
    pub domain: String,
    pub host: IpAddr,
    pub port: u16,
    pub pid: Option<u32>,
    pub protocol: Protocol,
    pub created_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Http,
    Https,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStatus {
    Active,
    Inactive,
    Error,
}
