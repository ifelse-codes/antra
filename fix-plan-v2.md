# Antra Fix Plan v2

> Generated from UX test on 2026-09-03
> Use this as a prompt to fix all identified issues

---

## Fix Priority Order

### P0 — Critical (Breaks core functionality)

#### 1. Daemon silently dies after `proxy start`

**Symptom:** `antra proxy start` prints "Daemon started (PID: XXXX)" then the process immediately exits. `antra proxy status` shows "Daemon is not running" right after.

**Root cause:** The daemon process forks but the child crashes. No error is surfaced to the user.

**Fix requirements:**
- After forking, wait and verify the child process is actually running before printing success
- If the child dies within N seconds, report the actual error (port binding failure, etc.)
- Add a health check loop: start → wait 2s → verify PID alive → report real status
- Consider logging to `~/Library/Application Support/antra/daemon.log` for debugging

**Test case:**
```bash
antra proxy start
sleep 3
antra proxy status  # Must show "Daemon is running", not "not running"
ps aux | grep antra | grep -v grep  # Must show a live process
```

---

#### 2. Ports 80/443 binding fails silently

**Symptom:** Daemon starts but doesn't actually listen on the configured ports. `doctor` warns about ports in use, but `proxy start` still prints success.

**Fix requirements:**
- After binding, verify the socket is actually listening (not just attempted)
- If binding fails, print a clear error: "Cannot bind to port 443 — try `antra proxy start --port 8443` or run with sudo"
- Add a `--privileged` flag or auto-detect if ports need elevation
- Consider using ports 8443/8080 as fallback when 443/80 fail, with a warning

**Test case:**
```bash
# Without sudo, should either:
# a) Work on 8443/8080 with a warning, OR
# b) Fail with a clear message suggesting --port 8443
antra proxy start
lsof -p $(cat ~/Library/Application\ Support/antra/daemon.pid) -iTCP -sTCP:LISTEN
# Must show actual TCP listeners
```

---

### P1 — High (Breaks user workflow)

#### 3. Auto-port detection in `antra run` doesn't work

**Symptom:** `antra run --domain test.localhost -- python3 -m http.server 9877` assigns a random port (51759) instead of detecting 9877. Results in 503 errors.

**Fix requirements:**
- Parse the command arguments to detect `python3 -m http.server PORT` patterns
- For other common servers (vite, next, etc.), detect port from `--port` flags in the command
- If detection fails, prompt the user: "What port does your server listen on?"
- Add `--port` as a required flag if detection fails, not a silent fallback

**Test case:**
```bash
antra run --domain test.localhost -- python3 -m http.server 9877
# Route should show port 9877, not a random port
curl -k https://test.localhost:8443/  # Should return content, not 503
```

---

#### 4. `antra trust` needs sudo but doesn't suggest it

**Symptom:** `antra trust` fails with `SecCertificateAddToKeychain: Write permissions error`. No suggestion to use sudo.

**Fix requirements:**
- Detect permission error and suggest: "Try: sudo antra trust" or "Run antra trust with admin privileges"
- Alternatively, use `security add-trusted-cert` with user-level keychain as fallback
- Consider adding `--user-level` flag that installs to login keychain (no sudo needed)

**Test case:**
```bash
antra trust
# Should either:
# a) Suggest "sudo antra trust" on failure, OR
# b) Install to user keychain without sudo
```

---

### P2 — Medium (UX issues)

#### 5. `remove` silently succeeds on non-existent routes

**Symptom:** `antra remove nonexistent.localhost` prints "Route removed: nonexistent.localhost" even though no such route existed.

**Fix requirements:**
- Check if route exists before attempting removal
- If route doesn't exist, print: "No route found for nonexistent.localhost"
- Exit with non-zero status code for scripting

**Test case:**
```bash
antra remove nonexistent.localhost
# Should print warning, not fake success
# Exit code should be non-zero
```

---

#### 6. `doctor` count is misleading

**Symptom:** `doctor` says "1 issue(s) found" but also shows warnings (⚠). The count only includes ✗, not ⚠.

**Fix requirements:**
- Count both errors (✗) and warnings (⚠) in the summary
- Or change wording: "1 error(s), 2 warning(s) found"
- Consider separate exit codes: 0=clean, 1=warnings, 2=errors

**Test case:**
```bash
antra doctor
# Summary should accurately reflect all issues
```

---

### P3 — Low (Nice to have)

#### 7. No `--verbose` or `--debug` flag

**Request:** Add debugging output for troubleshooting.

**Fix requirements:**
- Add `--verbose` flag to all commands
- Show: daemon connection attempts, port binding, route registration, cert generation
- Log to stderr so it doesn't break piped output

---

#### 8. No `antra.toml` project config tested

**Request:** The website promises `antra.toml` for team setup. This was not tested.

**Fix requirements:**
- Ensure `antra.toml` documentation is clear
- Add example `antra.toml` to repo
- Test `antra dev` command that reads from `antra.toml`

---

## Implementation Checklist

```
[x] Fix daemon lifecycle (P0-1)
    [x] Add health check after fork
    [x] Log errors to daemon.log
    [x] Verify PID alive before reporting success

[x] Fix port binding (P0-2)
    [x] Verify socket is actually listening
    [x] Print clear error on bind failure
    [x] Add auto-fallback to 8443/8080

[x] Fix auto-port detection (P1-3)
    [x] Parse common server patterns
    [x] Prompt user if detection fails
    [x] Make --port required when detection fails

[x] Fix trust error messaging (P1-4)
    [x] Detect permission error
    [x] Suggest sudo or user-level install
    [x] Add --user-level flag

[x] Fix remove on non-existent route (P2-5)
    [x] Check route existence before removal
    [x] Print warning for missing routes
    [x] Non-zero exit code

[x] Fix doctor summary count (P2-6)
    [x] Count warnings and errors separately
    [x] Update summary text

[x] Add --verbose flag (P3-7)
    [x] Add flag to all commands
    [x] Log detailed operation info

[x] Document antra.toml (P3-8)
    [x] Add example to repo
    [x] Test antra dev command
```

---

## Expected Outcome

After these fixes:

| Metric | Before | After (Target) |
|--------|--------|----------------|
| Setup ease | 4/10 | 8/10 |
| Promise delivery | 5/10 | 9/10 |
| "One command" | Partial | Yes |
| "Real HTTPS" | No | Yes |
| "No ports" | No | Yes |
| "No certificate warnings" | No | Yes |

**Critical success:** If `antra proxy start` says "started", it MUST be actually running. This alone moves the score from 4 → 7.
