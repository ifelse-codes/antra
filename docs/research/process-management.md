# Process Management Research

## Spawn Pattern

```rust
use tokio::process::Command;

let child = Command::new("pnpm")
    .arg("dev")
    .env("PORT", "5173")
    .env("HOST", "127.0.0.1")
    .env("ANTRA_DOMAIN", "myapp.localhost")
    .env("ANTRA_URL", "https://myapp.localhost")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
```

## Environment Variables

| Variable | Value | Purpose |
|----------|-------|---------|
| `PORT` | Assigned port | Most frameworks respect this |
| `HOST` | `127.0.0.1` | Bind address |
| `ANTRA_DOMAIN` | `myapp.localhost` | The domain Antra mapped |
| `ANTRA_URL` | `https://myapp.localhost` | Full URL |

### Precedence
```
explicit CLI flags
    > project config
    > Antra defaults
    > inferred values
```

## Signal Forwarding

```rust
#[cfg(unix)]
fn forward_signal(child: &Child, signal: Signal) {
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::getpgid;

    if let Some(pid) = child.id() {
        let pgid = getpgid(Pid::from_raw(pid as i32));
        if let Ok(pgid) = pgid {
            let _ = killpg(pgid, signal);
        }
    }
}
```

## Port Detection Strategies

1. **Explicit `--port`** — User specifies the port
2. **PORT injection** — Set env var, most frameworks respect it
3. **Port 0** — OS assigns free port, query via `local_addr()`
4. **Stdout parsing** — Last resort, fragile (framework-specific patterns)

### Framework Port Behavior

| Framework | Respects PORT? | Default Port |
|-----------|---------------|--------------|
| Vite | ✅ (with `--host`) | 5173 |
| Next.js | ✅ | 3000 |
| Astro | ✅ | 4321 |
| Remix | ✅ | 5173 |
| React Router | ✅ | 5173 |
| SvelteKit | ✅ | 5173 |
| Nuxt | ✅ | 3000 |
| Expo | ✅ | 8081 |
| Rails | ✅ | 3000 |
| Django | ✅ | 8000 |
| Flask | ✅ | 5000 |
| Go | ❌ (flags) | varies |
| Rust (axum) | ❌ (flags) | varies |

## Cleanup on Exit

```rust
async fn cleanup(child: &mut Child, registry: &RouteRegistry, domain: &str) {
    // 1. Forward SIGTERM to child process group
    #[cfg(unix)]
    forward_signal(child, Signal::SIGTERM);

    // 2. Wait up to 5 seconds for graceful exit
    let timeout = tokio::time::timeout(
        Duration::from_secs(5),
        child.wait()
    ).await;

    // 3. If still alive, SIGKILL
    if timeout.is_err() {
        child.kill().await.ok();
    }

    // 4. Remove route from registry
    registry.unregister(domain).ok();

    // 5. Clean up hosts entries if managed
    // (handled by resolver)

    // 6. Exit with child's exit code
}
```

## Orphan Prevention

- Child processes run in separate process group
- On Antra crash, OS kills process group (if set up correctly)
- `antra doctor` detects orphaned routes by checking PID liveness
- `antra clean` kills any remaining Antra-managed processes

## Sources
- tokio::process docs: https://docs.rs/tokio/
- nix crate: https://docs.rs/nix/
