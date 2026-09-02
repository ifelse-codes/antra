// Pure function tests for CLI utilities
// These are private functions, so we re-test the expected behavior here.

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn parse_route(s: &str) -> Result<(String, u16), String> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid route format '{s}'. Expected domain:port"));
    }
    let domain = parts[0].to_string();
    let port: u16 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid port in route '{s}'"))?;
    Ok((domain, port))
}

// ===== format_duration tests =====

#[test]
fn test_format_duration_zero() {
    assert_eq!(format_duration(0), "0s");
}

#[test]
fn test_format_duration_seconds() {
    assert_eq!(format_duration(1), "1s");
    assert_eq!(format_duration(30), "30s");
    assert_eq!(format_duration(59), "59s");
}

#[test]
fn test_format_duration_minutes() {
    assert_eq!(format_duration(60), "1m 0s");
    assert_eq!(format_duration(61), "1m 1s");
    assert_eq!(format_duration(90), "1m 30s");
    assert_eq!(format_duration(3599), "59m 59s");
}

#[test]
fn test_format_duration_hours() {
    assert_eq!(format_duration(3600), "1h 0m");
    assert_eq!(format_duration(3661), "1h 1m");
    assert_eq!(format_duration(7200), "2h 0m");
    assert_eq!(format_duration(90000), "25h 0m");
}

// ===== parse_route tests =====

#[test]
fn test_parse_route_valid() {
    let (domain, port) = parse_route("myapp.test:8080").unwrap();
    assert_eq!(domain, "myapp.test");
    assert_eq!(port, 8080);
}

#[test]
fn test_parse_route_high_port() {
    let (domain, port) = parse_route("api.localhost:65535").unwrap();
    assert_eq!(domain, "api.localhost");
    assert_eq!(port, 65535);
}

#[test]
fn test_parse_route_no_port() {
    assert!(parse_route("myapp.test").is_err());
}

#[test]
fn test_parse_route_non_numeric_port() {
    assert!(parse_route("myapp.test:abc").is_err());
}

#[test]
fn test_parse_route_empty_string() {
    assert!(parse_route("").is_err());
}

#[test]
fn test_parse_route_multiple_colons() {
    // splitn(2, ':') means only first colon splits
    let result = parse_route("a.b:80:extra");
    // "80:extra" fails to parse as u16
    assert!(result.is_err());
}
