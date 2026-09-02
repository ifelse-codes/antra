# Domain Resolution Research

## .localhost

### RFC Status
- **RFC 6761** (Section 6.3): Reserved special-use domain name
- Resolvers SHOULD return loopback address for `*.localhost` queries
- Browsers SHOULD treat `http://*.localhost` as a secure context

### Browser Behavior

| Browser | Resolves `*.localhost`? | Ignores /etc/hosts? | Secure Context? |
|---------|------------------------|---------------------|-----------------|
| Chrome/Chromium | ✅ Hardcoded to 127.0.0.1 | ✅ Yes | ✅ Yes (even HTTP) |
| Firefox (84+) | ✅ Hardcoded to 127.0.0.1 | ✅ Yes | ✅ Yes (even HTTP) |
| Safari (macOS < 26) | ❌ Needs /etc/hosts | N/A | ❌ Domain won't resolve |
| Safari (macOS 26+) | ✅ OS-level fix | ✅ Yes | ✅ Yes |
| curl (7.78+) | ⚠️ `localhost` only, subdomains via DNS | No | N/A |

### Key Gotchas
1. **Chrome ignores /etc/hosts for .localhost** — Custom entries like `192.168.88.88 myapp.localhost` are ignored
2. **Safari < macOS 26 needs hosts entries** — For Safari users on older macOS, must add `/etc/hosts` entries
3. **curl inconsistency** — `localhost` resolves, `foo.localhost` goes through DNS

### Antra Strategy
- **No-op**: Browsers handle `.localhost` natively
- **Document** Safari limitation for macOS < 26
- **Future**: Optional hosts file sync as Safari fallback

## .test

### RFC Status
- **RFC 2606**: IANA-reserved TLD, will never exist in global DNS
- Safe for testing, guaranteed no conflicts with real domains

### Resolution Methods

| Platform | Method | Wildcard? |
|----------|--------|-----------|
| macOS | `/etc/resolver/test` + dnsmasq | ✅ |
| macOS | `/etc/hosts` (per-domain) | ❌ |
| Linux | `systemd-resolved` drop-in + dnsmasq | ✅ |
| Linux | `/etc/hosts` (per-domain) | ❌ |
| Windows | `/etc/hosts` only | ❌ |

### Secure Context
- **No** — `.test` is NOT treated as secure by browsers
- HTTPS with trusted certificate is required for secure APIs

### Antra Strategy
- Manage `/etc/hosts` with managed block markers
- Require HTTPS for `.test` domains
- Phase 5 implementation

## Custom Domains

### Resolution
- Must modify `/etc/hosts` (or platform equivalent)
- No native browser/OS support

### Security
- Require `--allow-custom-domain` flag
- Reject known public domains (google.com, github.com, etc.)
- Warn for non-obvious dev domains

### Antra Strategy
- Managed hosts block with `BEGIN/END ANTRA MANAGED HOSTS` markers
- Atomic file operations (write temp, rename)
- Explicit opt-in required

## Cross-Platform Hosts File

| Platform | Path | Admin Required | Flush Command |
|----------|------|---------------|---------------|
| macOS | `/etc/hosts` | `sudo` | `sudo dscacheutil -flushcache && sudo killall -HUP mDNSResponder` |
| Linux | `/etc/hosts` | `sudo` (root) | `sudo resolvectl flush-caches` |
| Windows | `C:\Windows\System32\drivers\etc\hosts` | Administrator | `ipconfig /flushdns` |

## Sources
- RFC 6761: https://datatracker.ietf.org/doc/rfc6761/
- RFC 2606: https://www.rfc-editor.org/rfc/rfc2606.html
- W3C Secure Contexts: https://www.w3.org/TR/secure-contexts/
- Chrome .localhost behavior: Chromium issue 41175806
- Firefox .localhost: Bug 1220810
- Safari .localhost: WebKit Bug 160504 (fixed macOS 26)
