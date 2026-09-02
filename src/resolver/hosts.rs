use std::path::{Path, PathBuf};

use anyhow::Result;

const BEGIN_MARKER: &str = "# BEGIN ANTRA MANAGED HOSTS";
const END_MARKER: &str = "# END ANTRA MANAGED HOSTS";

/// Get the platform-specific hosts file path.
pub fn hosts_path() -> PathBuf {
    if cfg!(target_os = "windows") {
        PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts")
    } else {
        PathBuf::from("/etc/hosts")
    }
}

/// Read the hosts file and return its contents.
pub fn read_hosts(path: &Path) -> Result<String> {
    Ok(std::fs::read_to_string(path)?)
}

/// Write content to the hosts file atomically (temp + rename).
pub fn write_hosts_atomic(path: &Path, content: &str) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot get parent directory"))?;
    let temp_path = dir.join("hosts.antra.tmp");

    std::fs::write(&temp_path, content)?;

    #[cfg(unix)]
    {
        std::fs::rename(&temp_path, path)?;
    }
    #[cfg(not(unix))]
    {
        // On Windows, rename may fail if target exists — remove first
        let _ = std::fs::remove_file(path);
        std::fs::rename(&temp_path, path)?;
    }

    Ok(())
}

/// Ensure the managed block markers exist in the hosts file.
/// Returns the content with markers present.
pub fn ensure_managed_block(content: &str) -> String {
    if content.contains(BEGIN_MARKER) && content.contains(END_MARKER) {
        content.to_string()
    } else {
        let mut new_content = content.trim_end().to_string();
        if !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push_str(BEGIN_MARKER);
        new_content.push('\n');
        new_content.push_str(END_MARKER);
        new_content.push('\n');
        new_content
    }
}

/// Extract just the managed block content (between markers).
pub fn extract_managed_block(content: &str) -> String {
    let begin = content.find(BEGIN_MARKER);
    let end = content.find(END_MARKER);

    match (begin, end) {
        (Some(b), Some(e)) => {
            let block_start = b + BEGIN_MARKER.len();
            let block_end = e;
            content[block_start..block_end].to_string()
        }
        _ => String::new(),
    }
}

/// Replace the managed block content in the hosts file.
pub fn replace_managed_block(content: &str, new_block: &str) -> String {
    let begin = content.find(BEGIN_MARKER);
    let end = content.find(END_MARKER);

    match (begin, end) {
        (Some(b), Some(e)) => {
            let marker_end = e + END_MARKER.len();
            let before = &content[..b];
            let after = &content[marker_end..];

            let mut result = String::new();
            result.push_str(before);
            result.push_str(BEGIN_MARKER);
            result.push('\n');
            result.push_str(new_block);
            result.push_str(END_MARKER);
            result.push_str(after);
            result
        }
        _ => {
            // No markers found — add them
            let mut result = content.trim_end().to_string();
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(BEGIN_MARKER);
            result.push('\n');
            result.push_str(new_block);
            result.push_str(END_MARKER);
            result.push('\n');
            result
        }
    }
}

/// Check if a domain already exists in the managed block.
pub fn domain_in_managed_block(content: &str, domain: &str) -> bool {
    let block = extract_managed_block(content);
    let entry = format!("127.0.0.1 {domain}");
    block.lines().any(|line| line.trim() == entry)
}

/// Add a domain to the managed block. Returns true if added, false if already present.
pub fn add_to_managed_block(content: &str, domain: &str) -> (String, bool) {
    if domain_in_managed_block(content, domain) {
        return (content.to_string(), false);
    }

    let block = extract_managed_block(content);
    let mut lines: Vec<&str> = block.lines().collect();

    // Remove empty lines at start/end of block
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }

    let entry = format!("127.0.0.1 {domain}");
    if !lines.is_empty() {
        lines.push("");
    }
    lines.push(&entry);
    lines.push("");

    let new_block = lines.join("\n");
    (replace_managed_block(content, &new_block), true)
}

/// Remove a domain from the managed block. Returns true if removed, false if not found.
pub fn remove_from_managed_block(content: &str, domain: &str) -> (String, bool) {
    let entry = format!("127.0.0.1 {domain}");
    let block = extract_managed_block(content);
    let lines: Vec<&str> = block.lines().collect();

    let filtered: Vec<&str> = lines
        .iter()
        .filter(|line| line.trim() != entry.trim())
        .copied()
        .collect();

    let removed = filtered.len() < lines.len();
    let new_block = filtered.join("\n") + "\n";

    (replace_managed_block(content, &new_block), removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_managed_block() {
        let content = "127.0.0.1 localhost\n";
        let result = ensure_managed_block(content);
        assert!(result.contains(BEGIN_MARKER));
        assert!(result.contains(END_MARKER));
        assert!(result.contains("127.0.0.1 localhost"));
    }

    #[test]
    fn test_add_to_managed_block() {
        let content = "# BEGIN ANTRA MANAGED HOSTS\n# END ANTRA MANAGED HOSTS\n";
        let (result, added) = add_to_managed_block(content, "myapp.test");
        assert!(added);
        assert!(result.contains("127.0.0.1 myapp.test"));
    }

    #[test]
    fn test_add_duplicate() {
        let content =
            "# BEGIN ANTRA MANAGED HOSTS\n127.0.0.1 myapp.test\n# END ANTRA MANAGED HOSTS\n";
        let (_, added) = add_to_managed_block(content, "myapp.test");
        assert!(!added);
    }

    #[test]
    fn test_remove_from_managed_block() {
        let content =
            "# BEGIN ANTRA MANAGED HOSTS\n127.0.0.1 myapp.test\n# END ANTRA MANAGED HOSTS\n";
        let (result, removed) = remove_from_managed_block(content, "myapp.test");
        assert!(removed);
        assert!(!result.contains("myapp.test"));
    }

    #[test]
    fn test_domain_in_managed_block() {
        let content =
            "# BEGIN ANTRA MANAGED HOSTS\n127.0.0.1 myapp.test\n# END ANTRA MANAGED HOSTS\n";
        assert!(domain_in_managed_block(content, "myapp.test"));
        assert!(!domain_in_managed_block(content, "other.test"));
    }

    #[test]
    fn test_replace_preserves_existing_hosts() {
        let content = "127.0.0.1 localhost\n255.255.255.255 broadcasthost\n# BEGIN ANTRA MANAGED HOSTS\n# END ANTRA MANAGED HOSTS\n";
        let (result, _) = add_to_managed_block(content, "myapp.test");
        assert!(result.contains("127.0.0.1 localhost"));
        assert!(result.contains("255.255.255.255 broadcasthost"));
        assert!(result.contains("127.0.0.1 myapp.test"));
    }
}
