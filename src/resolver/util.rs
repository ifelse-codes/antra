use anyhow::Result;

use crate::resolver::traits::DomainResolver;

const RISKY_PUBLIC_TLDS: &[&str] = &["com", "org", "net", "io", "dev", "app"];

pub fn select_resolver(domain: &str) -> Result<Box<dyn DomainResolver>> {
    validate_domain_shape(domain)?;
    let domain = domain.to_ascii_lowercase();
    if domain == "localhost" || domain.ends_with(".localhost") {
        Ok(Box::new(crate::resolver::localhost::LocalhostResolver))
    } else if domain.ends_with(".test")
        || domain.ends_with(".internal")
        || domain.ends_with(".local")
    {
        Ok(Box::new(crate::resolver::test::HostsResolver::new()))
    } else {
        Ok(Box::new(crate::resolver::custom::CustomResolver::new()))
    }
}

pub fn is_custom_domain(domain: &str) -> bool {
    let domain = domain.to_ascii_lowercase();
    !(domain == "localhost"
        || domain.ends_with(".localhost")
        || domain.ends_with(".test")
        || domain.ends_with(".internal")
        || domain.ends_with(".local"))
}

pub fn validate_domain_for_registration(domain: &str, allow_custom_domain: bool) -> Result<()> {
    validate_domain_shape(domain)?;
    let domain = domain.to_ascii_lowercase();
    if domain == "localhost" || domain.ends_with(".localhost") || domain.ends_with(".test") {
        return Ok(());
    }
    if domain.ends_with(".internal") || domain.ends_with(".local") {
        tracing::warn!(%domain, "Domain uses a collision-prone local TLD");
        return Ok(());
    }
    if !allow_custom_domain {
        anyhow::bail!("Custom domain '{domain}' requires explicit --allow-custom-domain approval");
    }
    if let Some(tld) = domain.rsplit('.').next() {
        if RISKY_PUBLIC_TLDS.contains(&tld) {
            tracing::warn!(%domain, %tld, "Domain uses a public TLD. Ensure this is intentional");
        }
    }
    Ok(())
}

pub fn select_resolver_for_registration(
    domain: &str,
    allow_custom_domain: bool,
) -> Result<Box<dyn DomainResolver>> {
    validate_domain_for_registration(domain, allow_custom_domain)?;
    if is_custom_domain(domain) {
        Ok(Box::new(
            crate::resolver::custom::CustomResolver::new().with_custom_domain_allowed(true),
        ))
    } else {
        select_resolver(domain)
    }
}

/// Validate that a string is shaped like a DNS hostname: dot-separated
/// labels of letters, digits, hyphens, and underscores.
///
/// This is a *shape* check only (no policy about which suffixes are
/// allowed). It runs before any resolver touches the system so garbage
/// input fails with a clear error instead of a misleading privileged-write
/// failure (e.g. "Permission denied writing /etc/hosts … try sudo").
pub fn validate_domain_shape(domain: &str) -> anyhow::Result<()> {
    const EXAMPLE: &str = "e.g. myapp.localhost";
    if domain.is_empty() {
        anyhow::bail!("Invalid domain: empty ({EXAMPLE})");
    }
    if domain.len() > 253 {
        anyhow::bail!("Invalid domain '{domain}': too long (max 253 characters)");
    }
    if domain.contains(char::is_whitespace) {
        anyhow::bail!("Invalid domain '{domain}': must not contain spaces ({EXAMPLE})");
    }
    if !domain
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        anyhow::bail!(
            "Invalid domain '{domain}': only letters, digits, dots, hyphens, and underscores are allowed ({EXAMPLE})"
        );
    }
    for label in domain.split('.') {
        if label.is_empty() {
            anyhow::bail!(
                "Invalid domain '{domain}': empty label (leading, trailing, or double dots are not allowed)"
            );
        }
        if label.len() > 63 {
            anyhow::bail!("Invalid domain '{domain}': label '{label}' exceeds 63 characters");
        }
        if label.starts_with('-') || label.ends_with('-') {
            anyhow::bail!(
                "Invalid domain '{domain}': label '{label}' must not start or end with a hyphen"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_accepts_normal_domains() {
        for d in [
            "myapp.localhost",
            "localhost",
            "api.myapp.localhost",
            "myapp.test",
            "myapp.custom",
            "dev.local",
            "a",
            "my_app.test",
            "my-app123.example",
        ] {
            assert!(validate_domain_shape(d).is_ok(), "{d} should be valid");
        }
    }

    #[test]
    fn test_shape_rejects_garbage() {
        for d in [
            "",
            "not a domain",
            "has space.test",
            "foo bar",
            ".localhost",
            "myapp.",
            "foo..test",
            "-foo.test",
            "foo-.test",
            "foo/bar.test",
            "foo:8080",
            "under_score!.test",
        ] {
            assert!(validate_domain_shape(d).is_err(), "{d} should be invalid");
        }
    }

    #[test]
    fn test_shape_rejects_too_long() {
        let long_label = "a".repeat(64);
        assert!(validate_domain_shape(&format!("{long_label}.test")).is_err());
        let long_domain = format!("{}.test", "a".repeat(250));
        assert!(validate_domain_shape(&long_domain).is_err());
    }

    #[test]
    fn test_select_resolver_rejects_invalid_before_privileged_write() {
        // Previously fell through to CustomResolver and attempted an
        // /etc/hosts write, surfacing as a bogus "needs sudo" hint.
        assert!(select_resolver("not a domain").is_err());
    }

    #[test]
    fn test_registration_policy_allows_automatic_suffixes_without_approval() {
        for domain in [
            "localhost",
            "app.localhost",
            "api.app.localhost",
            "app.test",
            "app.internal",
            "app.local",
        ] {
            assert!(
                validate_domain_for_registration(domain, false).is_ok(),
                "{domain} should be automatic"
            );
        }
    }

    #[test]
    fn test_registration_policy_rejects_custom_and_public_domains_without_approval() {
        for domain in ["api.example", "preview.customer.dev", "github.com"] {
            let error = validate_domain_for_registration(domain, false).unwrap_err();
            assert!(error.to_string().contains("--allow-custom-domain"));
        }
    }

    #[test]
    fn test_registration_policy_accepts_custom_and_public_domains_with_approval() {
        for domain in ["api.example", "preview.customer.dev", "github.com"] {
            assert!(validate_domain_for_registration(domain, true).is_ok());
        }
    }
}
