# ANTRA — MASTER ENGINEERING PROMPT

## Mission

You are the lead engineer, systems architect, Rust developer, networking engineer, security engineer, QA engineer, and release engineer responsible for building **Antra**.

**Antra** is a native, cross-platform, Rust-based developer CLI and local networking layer that turns ordinary local development servers such as:

```text
localhost:3000
localhost:5173
localhost:8080
localhost:4321
```

into stable, configurable local domains such as:

```text
https://myapp.localhost
https://api.myapp.localhost
https://myapp.test
https://api.myapp.com
```

while the actual applications continue running locally.

The fundamental concept is:

```text
                         BROWSER
                            │
                            │ https://yapp.com
                            ▼
                    ┌────────────────┐
                    │ Local Resolver │
                    │                │
                    │ yapp.com        │
                    │      ↓         │
                    │  127.0.0.1     │
                    └───────┬────────┘
                            │
                            ▼
                    ┌────────────────┐
                    │     ANTRA      │
                    │ Local Proxy    │
                    │      :443      │
                    └───────┬────────┘
                            │
                            ▼
                       localhost:5173
                            │
                            ▼
                         YOUR APP
```

Antra should make this experience feel native and effortless.

---

# 1. PRODUCT VISION

The user should be able to install Antra once and then stop thinking about localhost ports.

Instead of:

```bash
pnpm dev
```

then manually remembering:

```text
http://localhost:5173
```

the developer should be able to run something conceptually like:

```bash
antra run --domain myapp.localhost -- pnpm dev
```

and receive:

```text
ANTRA

✓ Proxy ready
✓ Domain registered
✓ HTTPS ready

  https://myapp.localhost

  → localhost:5173
```

The application remains completely local.

Antra is the layer between:

```text
DOMAIN
   ↓
LOCAL RESOLUTION
   ↓
ANTRA PROXY
   ↓
LOCAL PROCESS
```

---

# 2. IMPORTANT TERMINOLOGY

The product name is:

**Antra**

Do NOT rename it to Antara.

Use:

```text
antra
```

for:

- binary
- CLI commands
- Cargo package
- documentation
- configuration
- project references

The name should always be written:

**Antra**

---

# 3. CORE DESIGN PRINCIPLES

The implementation must follow these principles.

## 3.1 Native first

Antra is primarily a Rust application.

It must NOT depend on:

- Node.js
- npm
- pnpm
- Bun
- Python
- Ruby
- Java
- Docker

to operate its core functionality.

Node/npm/pnpm/Bun are merely applications Antra may execute.

---

## 3.2 Language agnostic

Antra must be able to proxy applications regardless of runtime.

Examples:

```bash
antra run --domain api.example.com -- python app.py
```

```bash
antra run --domain api.example.com -- cargo run
```

```bash
antra run --domain api.example.com -- go run main.go
```

```bash
antra run --domain app.example.com -- pnpm dev
```

```bash
antra run --domain app.example.com -- npm run dev
```

```bash
antra run --domain app.example.com -- bun dev
```

The core system must never assume the application is Node-based.

---

## 3.3 Zero-config should be the default

The common case should require almost no configuration.

The ideal workflow is:

```bash
antra run --domain myapp.localhost -- pnpm dev
```

Eventually, project configuration should allow:

```bash
antra dev
```

to infer the configured domain.

---

## 3.4 Explicit configuration over magic

Antra may provide intelligent defaults, but must never make dangerous assumptions.

In particular:

- Never silently take over arbitrary real domains.
- Never silently modify DNS configuration without telling the user.
- Never silently install a trusted root CA.
- Never overwrite unrelated `/etc/hosts` entries.
- Never destroy unrelated certificates.
- Never kill unrelated processes.

All privileged/system changes must be explicit, reversible, and observable.

---

# 4. PRIMARY USE CASES

Antra must support these workflows.

## Use case A — localhost replacement

```bash
antra run --domain myapp.localhost -- pnpm dev
```

Result:

```text
https://myapp.localhost
```

→ local development server.

---

## Use case B — API

```bash
antra run --domain api.myapp.localhost -- cargo run
```

Result:

```text
https://api.myapp.localhost
```

---

## Use case C — multiple applications

Example:

```text
frontend.myapp.localhost → localhost:3000
api.myapp.localhost      → localhost:8080
admin.myapp.localhost    → localhost:4173
```

All applications run simultaneously.

Antra must route requests according to hostname.

---

## Use case D — custom local domain

Support:

```text
myapp.test
myapp.localhost
myapp.internal
```

where technically and legally appropriate.

---

## Use case E — real-looking local development domain

The system should eventually support explicit user-owned/custom domains such as:

```text
myapp.com
api.myapp.com
admin.myapp.com
```

mapped locally.

Example:

```bash
antra run --domain myapp.com -- pnpm dev
```

The architecture must distinguish this from `.localhost` and `.test`.

Do not assume arbitrary public domains can simply be redirected without configuring local name resolution.

---

# 5. DOMAIN RESOLUTION ARCHITECTURE

Research this area before implementing.

The implementation must support a clean abstraction:

```rust
trait DomainResolver {
    fn register(&self, domain: &str) -> Result<()>;
    fn unregister(&self, domain: &str) -> Result<()>;
    fn status(&self, domain: &str) -> Result<ResolutionStatus>;
}
```

Potential implementations include:

### Strategy A

`.localhost`

Use the operating system/browser's reserved localhost behavior where possible.

Avoid unnecessary `/etc/hosts` modification.

### Strategy B

`.test`

Use appropriate local resolution.

### Strategy C

Custom domains

Potentially manage:

```text
/etc/hosts
```

or another local resolver mechanism.

The agent must research the correct cross-platform mechanism before choosing the implementation.

Platforms:

- macOS
- Linux
- Windows

Do not assume Unix behavior works on Windows.

---

# 6. IMPORTANT SECURITY RULE

Custom domains are powerful.

For example:

```text
google.com
github.com
bank.com
```

must NOT casually become local routes.

Implement protections against dangerous accidental domain overrides.

The user should explicitly confirm risky custom domains.

Consider rejecting or warning for:

- major public domains
- domains not obviously intended for local development
- domains with existing DNS ownership
- system/security-sensitive hostnames

The exact policy should be researched and documented.

---

# 7. REVERSE PROXY

Antra needs a native reverse proxy.

Preferred Rust ecosystem:

- Tokio
- Hyper
- Hyper-util
- Tower
- Rustls

But do not blindly use these exact versions.

Before implementation:

1. Check current stable Rust.
2. Check current compatible versions.
3. Verify APIs.
4. Choose maintained dependencies.
5. Record decisions in architecture documentation.

The proxy must support:

```text
HTTP
HTTPS
HTTP/1.1
HTTP/2 where appropriate
WebSockets
Upgrade requests
streaming bodies
large request/response bodies
```

---

# 8. ROUTING

Core routing model:

```text
DOMAIN → LOCAL TARGET
```

Example:

```text
myapp.localhost → 127.0.0.1:5173
api.myapp.localhost → 127.0.0.1:8080
```

Internally maintain a route registry.

Conceptually:

```rust
struct Route {
    domain: String,
    host: IpAddr,
    port: u16,
    pid: Option<u32>,
    protocol: Protocol,
    created_at: DateTime,
}
```

The proxy receives:

```http
Host: api.myapp.localhost
```

and resolves:

```text
api.myapp.localhost
        ↓
127.0.0.1:8080
```

---

# 9. HTTP PROXY BEHAVIOR

Port 80:

```text
HTTP → HTTPS
```

Example:

```text
http://myapp.localhost/foo
```

redirects to:

```text
https://myapp.localhost/foo
```

Preserve:

- host
- path
- query string

Do not create redirect loops.

---

# 10. HTTPS

HTTPS should be a first-class Antra feature.

The goal is:

```text
https://myapp.localhost
```

without browser certificate warnings after setup.

Antra should use a local development Root CA.

Potential architecture:

```text
Antra Root CA
      │
      ├── myapp.localhost
      ├── api.myapp.localhost
      ├── myapp.test
      └── custom-domain
```

Use modern Rust TLS libraries.

Potentially:

- rustls
- rcgen

But verify current APIs and security recommendations before implementation.

---

# 11. ROOT CA MANAGEMENT

Antra should create a dedicated CA.

Example conceptual location:

```text
~/.config/antra/
```

with appropriate OS-specific configuration directories.

Do NOT hard-code Unix paths for every platform.

Use platform-appropriate directories.

Example:

```text
ca.pem
ca-key.pem
config.toml
routes.json
```

The private key must have appropriate filesystem permissions.

Never print the private key.

Never commit it.

Never expose it through logs.

---

# 12. TRUST MANAGEMENT

Implement:

```bash
antra trust
```

and:

```bash
antra trust --status
```

Potentially:

```bash
antra trust --remove
```

Support:

- macOS
- Linux
- Windows

But first research each platform's recommended trust mechanism.

Do not assume:

```bash
security
certutil
update-ca-certificates
```

are universally available.

Detect the environment.

Provide clear errors.

Example:

```text
Antra needs permission to install its local development CA.

This changes your system certificate trust store.

Continue? [y/N]
```

Never perform this silently.

---

# 13. DYNAMIC CERTIFICATES / SNI

The proxy should eventually dynamically issue certificates for requested domains.

Conceptually:

```text
TLS ClientHello
       ↓
SNI = api.myapp.com
       ↓
Antra
       ↓
lookup certificate cache
       ↓
if missing:
create leaf certificate
       ↓
signed by Antra Root CA
       ↓
TLS connection
```

Use a certificate cache.

Do not regenerate certificates unnecessarily.

Certificate generation must be safe under concurrency.

---

# 14. PROCESS RUNNER

The CLI must be able to run arbitrary commands.

Example:

```bash
antra run --domain api.myapp.com -- cargo run
```

The runner should:

1. Parse the command.
2. Validate the domain.
3. Ensure proxy is running.
4. Determine target port.
5. Register the route.
6. Spawn child process.
7. Monitor it.
8. Forward signals.
9. Detect termination.
10. Remove the route.
11. Clean up temporary state.

---

# 15. PORT MANAGEMENT

There are two possible models.

## Model A — application specifies the port

Example:

```bash
antra run --domain app.localhost --port 5173 -- pnpm dev
```

Antra routes:

```text
app.localhost → 5173
```

## Model B — Antra allocates the port

Example:

```bash
antra run --domain app.localhost -- pnpm dev
```

Antra selects an available port.

If the framework accepts:

```text
PORT
```

inject:

```text
PORT=<allocated-port>
```

Also potentially:

```text
HOST=127.0.0.1
ANTRA_URL=https://app.localhost
```

However:

**Do not assume every framework respects PORT.**

Research common frameworks:

- Vite
- Next.js
- Astro
- Remix
- React Router
- Nuxt
- SvelteKit
- Expo
- Rails
- Django
- Flask
- FastAPI
- Go
- Rust

Build framework-specific behavior only when justified.

Avoid turning Antra into a framework-specific launcher.

---

# 16. PORT DETECTION

If the child process chooses its own port, Antra should eventually be able to detect it.

Possible strategies:

1. Explicit `--port`.
2. Inject `PORT`.
3. Parse known startup output.
4. Inspect listening sockets.
5. Allow manual port configuration.

Do not rely exclusively on stdout parsing.

The final architecture should be robust against:

- buffered stdout
- localized output
- frameworks changing log formats
- applications writing logs to stderr
- applications not printing a URL

---

# 17. ENVIRONMENT INJECTION

Potential variables:

```text
PORT
HOST
ANTRA_URL
ANTRA_DOMAIN
ANTRA_PORT
```

Only inject variables that are useful and documented.

Do not overwrite user-provided environment variables unexpectedly.

Define precedence clearly:

```text
explicit CLI
    >
project config
    >
Antra defaults
    >
inferred values
```

Document this.

---

# 18. WEBSOCKET / HMR SUPPORT

This is mandatory for modern frontend development.

Test against:

- Vite
- Next.js
- Turbopack
- Astro

Support HTTP upgrade correctly.

Do not implement WebSocket support as an afterthought.

Test:

```text
Browser
   ↓
Antra HTTPS
   ↓
WebSocket
   ↓
localhost application
```

HMR must continue working.

---

# 19. FORWARDED HEADERS

The proxy should correctly handle headers such as:

```text
X-Forwarded-For
X-Forwarded-Proto
X-Forwarded-Host
```

Avoid blindly trusting incoming spoofed forwarding headers.

Document the trust model.

---

# 20. DAEMON ARCHITECTURE

Antra should eventually have a background daemon.

Conceptually:

```text
                 ANTRA CLI
                     │
                     │ IPC
                     ▼
              ANTRA DAEMON
                     │
          ┌──────────┼──────────┐
          ▼          ▼          ▼
       routes       TLS        proxy
```

The daemon owns:

- HTTP listener
- HTTPS listener
- route registry
- certificate cache
- domain registrations
- lifecycle state

The CLI communicates with it.

---

# 21. IPC

Use platform-appropriate IPC.

Potential:

### macOS/Linux

Unix domain socket.

### Windows

Named pipe.

Do not assume Unix sockets work identically on Windows.

Define a versioned IPC protocol.

Example:

```json
{
  "version": 1,
  "command": "register_route",
  "domain": "api.myapp.localhost",
  "port": 8080
}
```

Responses should be structured.

Handle:

- daemon unavailable
- stale socket
- incompatible version
- malformed messages
- permission problems

---

# 22. DAEMON LIFECYCLE

Commands should eventually include:

```bash
antra proxy start
antra proxy stop
antra proxy status
```

The normal workflow should auto-start the daemon when appropriate.

Example:

```bash
antra run --domain app.localhost -- pnpm dev
```

If the daemon isn't running:

```text
Antra proxy is not running.
Starting it...
✓ Proxy started.
```

Do not require the developer to manually start infrastructure every time.

---

# 23. ROUTE MANAGEMENT CLI

Implement:

```bash
antra list
```

Example:

```text
ACTIVE ROUTES

DOMAIN                       TARGET              PID
──────────────────────────────────────────────────────
app.localhost                127.0.0.1:5173      8211
api.localhost                127.0.0.1:8080      8232
admin.localhost              127.0.0.1:4173      8240
```

Additional commands:

```bash
antra remove <domain>
antra stop <domain>
antra open <domain>
```

Only implement commands when they have a clear lifecycle meaning.

---

# 24. STATIC ALIASES

Eventually support applications Antra does not launch itself.

Example:

```bash
antra alias api.myapp.localhost 8080
```

Then:

```text
api.myapp.localhost
        ↓
localhost:8080
```

This is especially useful for:

- Docker
- manually started servers
- external development tools

Removing the alias:

```bash
antra alias remove api.myapp.localhost
```

---

# 25. PROJECT CONFIGURATION

Support an optional project-level configuration file.

Potential:

```text
antra.toml
```

Example:

```toml
domain = "myapp.localhost"

[server]
command = "pnpm"
args = ["dev"]
port = 5173
```

Potentially:

```toml
[server]
command = "cargo"
args = ["run"]

[domain]
name = "api.myapp.localhost"
```

Do not over-design this initially.

The CLI must work without the config file.

---

# 26. PACKAGE MANAGER INTEGRATION

Antra must NOT require npm.

However, JavaScript developers should be able to integrate it easily.

Example:

```json
{
  "scripts": {
    "dev": "antra run --domain myapp.localhost -- vite"
  }
}
```

Or eventually:

```json
{
  "scripts": {
    "dev": "antra"
  }
}
```

with:

```toml
domain = "myapp.localhost"
command = "pnpm"
args = ["vite"]
```

Also support:

```bash
pnpm dev
npm run dev
bun dev
```

where appropriate.

The Rust binary remains the actual engine.

---

# 27. CLI DESIGN

Use Clap or a similarly mature Rust CLI parser.

Initial command structure:

```text
antra
├── run
├── dev
├── alias
├── list
├── open
├── remove
├── trust
├── doctor
├── proxy
│   ├── start
│   ├── stop
│   └── status
└── clean
```

Potential shorthand:

```bash
antra myapp.localhost -- pnpm dev
```

may eventually be supported.

But don't sacrifice clarity for clever syntax.

---

# 28. DOCTOR COMMAND

Implement:

```bash
antra doctor
```

It should diagnose:

```text
✓ Rust binary
✓ Antra configuration
✓ Root CA
✓ Root CA trust
✓ Local resolver
✓ Port 80
✓ Port 443
✓ Proxy daemon
✓ IPC
✓ Route registry
```

Example failure:

```text
✗ Port 443 is already in use

Process:
  PID 1823
  nginx

Suggested action:
  antra proxy start --port 8443
```

Do not simply say "something went wrong."

Provide actionable diagnostics.

---

# 29. CLEANUP

Implement safe cleanup.

Example:

```bash
antra clean
```

Potentially removes:

- Antra routes
- Antra-managed hosts entries
- Antra CA trust
- Antra configuration

BUT:

Never delete unrelated user configuration.

Before destructive cleanup:

```text
This will remove Antra's local CA and managed routes.

Continue? [y/N]
```

---

# 30. `/etc/hosts` MANAGEMENT

If hosts-file management is used, it must be extremely careful.

Use a clearly identifiable block:

```text
# BEGIN ANTRA MANAGED HOSTS
127.0.0.1 yapp.com
127.0.0.1 api.yapp.com
# END ANTRA MANAGED HOSTS
```

Requirements:

- Never overwrite unrelated entries.
- Preserve formatting outside the managed block.
- Be atomic where possible.
- Handle duplicate registrations.
- Handle concurrent modifications.
- Recover gracefully if another process changes the file.
- Remove only Antra-owned entries.

Do not assume `/etc/hosts` is the best solution for every platform.

Research alternatives first.

---

# 31. CUSTOM DOMAIN SAFETY

Custom domains create a potential security problem.

For example:

```text
antra run --domain github.com ...
```

could make a developer accidentally route GitHub locally.

Design safeguards.

At minimum:

```text
localhost
.test
.localhost
```

should be considered safe development namespaces.

Arbitrary public domains should require explicit opt-in.

Potential syntax:

```bash
antra run --domain myapp.com --allow-custom-domain -- pnpm dev
```

The exact UX should be researched and refined.

---

# 32. BROWSER EXTENSION — OPTIONAL PHASE

Do NOT build the browser extension before the core CLI works.

Eventually Antra may support environments where modifying system DNS/hosts is undesirable.

A browser extension could route selected domains to:

```text
127.0.0.1:443
```

through browser proxy/PAC capabilities.

Possible architecture:

```text
Browser
   ↓
Extension
   ↓
PAC
   ↓
127.0.0.1:443
   ↓
Antra
```

Potentially use Native Messaging to communicate with the Antra daemon.

But this is explicitly a later phase.

Do not allow the browser extension to complicate the MVP.

---

# 33. CROSS-PLATFORM SUPPORT

Target:

```text
macOS
Linux
Windows
```

The architecture must separate platform-specific code.

Example:

```text
src/platform/
├── mod.rs
├── macos.rs
├── linux.rs
└── windows.rs
```

or an equivalent clean abstraction.

Platform-specific responsibilities include:

- hosts/resolver configuration
- CA trust
- process groups
- signals
- IPC
- config directories
- service/daemon startup

Do not litter the entire codebase with random `cfg` blocks.

---

# 34. PROJECT STRUCTURE

Use a clean Rust workspace/module structure.

A possible starting point:

```text
antra/
├── Cargo.toml
├── README.md
├── LICENSE
├── CONTRIBUTING.md
├── CHANGELOG.md
├── docs/
│   ├── architecture.md
│   ├── networking.md
│   ├── security.md
│   ├── development.md
│   └── troubleshooting.md
│
├── src/
│   ├── main.rs
│   ├── cli/
│   ├── config/
│   ├── daemon/
│   ├── proxy/
│   ├── routing/
│   ├── process/
│   ├── certs/
│   ├── resolver/
│   ├── ipc/
│   ├── platform/
│   └── util/
│
└── tests/
    ├── routing/
    ├── proxy/
    ├── certificates/
    ├── process/
    └── integration/
```

Modify this structure if research indicates a better architecture.

Do not create modules merely for the sake of having many files.

---

# 35. TECHNOLOGY SELECTION

Potential technology:

```text
Rust
Tokio
Hyper
Tower
Rustls
RCGen
Serde
TOML
Clap
Tracing
```

But:

**Research current versions before implementing.**

Do not blindly use versions from an old specification.

For every major dependency, evaluate:

- maintenance status
- current stable release
- API quality
- security history
- platform support
- license
- ecosystem compatibility

Prefer mature, actively maintained dependencies.

---

# 36. LICENSE

Research the licenses of all dependencies.

The project should use a permissive license suitable for open-source distribution and potential commercial use.

If using or adapting code from existing projects such as Portless, comply fully with their license requirements.

Do not copy code without checking its license.

Use existing projects as architectural references where appropriate.

---

# 37. PORTLESS RESEARCH

Portless is an important reference implementation.

Study it.

Determine:

- what problem it solves
- how its proxy works
- how it handles ports
- how it handles domain resolution
- how it handles HTTPS
- how it handles process lifecycle
- what limitations it has
- what its Rust implementation does differently
- which ideas Antra should adopt
- which ideas Antra should intentionally avoid

Do NOT simply clone Portless.

Antra's purpose is to develop a clean independent architecture.

Create:

```text
docs/research/portless.md
```

with findings.

---

# 38. COMPETITOR RESEARCH

Before finalizing the architecture, research:

- Portless
- Caddy
- Traefik
- nginx
- local reverse proxy tools
- mkcert
- local DNS tools
- dnsmasq
- browser PAC approaches
- hosts-file approaches
- similar Rust projects

For each, determine:

```text
What does it solve?
How does it solve it?
What should Antra learn?
What should Antra avoid?
```

Do not add features simply because competitors have them.

---

# 39. MVP DEFINITION

The first release must be intentionally small.

### MVP MUST support

```text
✓ Rust CLI
✓ arbitrary child commands
✓ domain → local port
✓ HTTP reverse proxy
✓ HTTPS reverse proxy
✓ local CA
✓ safe certificate generation
✓ WebSockets
✓ HMR
✓ route registry
✓ process lifecycle
✓ macOS support
✓ Linux support if reasonably achievable
✓ useful diagnostics
```

### MVP SHOULD NOT require

```text
✗ browser extension
✗ native messaging
✗ complex GUI
✗ cloud service
✗ external DNS provider
✗ account system
✗ telemetry
✗ framework-specific integrations
```

---

# 40. DEVELOPMENT PHASES

Do not attempt to implement the entire system in one pass.

Work in phases.

## Phase 0 — Research

Before coding:

1. Study Portless.
2. Study local domain resolution.
3. Study HTTPS local CA approaches.
4. Study Hyper/Tokio/Rustls current APIs.
5. Study macOS/Linux/Windows behavior.
6. Study process management.
7. Study WebSocket proxying.
8. Identify security risks.

Write:

```text
docs/architecture.md
docs/research/
docs/security.md
```

---

## Phase 1 — Minimal HTTP proxy

Build:

```text
localhost:9000
        ↑
     Antra
        ↑
myapp.localhost
```

No HTTPS initially if HTTPS blocks progress.

Prove:

```text
domain → route → port → HTTP response
```

---

## Phase 2 — CLI process runner

Implement:

```bash
antra run --domain myapp.localhost -- <command>
```

The command starts.

Antra registers it.

Ctrl+C cleans it up.

---

## Phase 3 — WebSockets/HMR

Test against a real Vite application.

Must support HMR.

---

## Phase 4 — HTTPS

Add:

```text
Root CA
   ↓
leaf certificates
   ↓
SNI
   ↓
TLS proxy
```

Test in real browsers.

---

## Phase 5 — Domain resolution

Implement the safest appropriate local resolver strategy.

Test:

```text
myapp.localhost
myapp.test
custom domains
```

---

## Phase 6 — Daemon + IPC

Move long-lived proxy infrastructure into a daemon.

CLI communicates through IPC.

---

## Phase 7 — Multiple routes

Support many simultaneous applications.

---

## Phase 8 — Configuration

Add:

```text
antra.toml
```

and project-aware workflows.

---

## Phase 9 — Developer experience

Add:

```bash
antra list
antra open
antra doctor
antra clean
```

Improve terminal output.

---

## Phase 10 — Cross-platform hardening

Test:

```text
macOS
Linux
Windows
```

Fix platform-specific behavior.

---

## Phase 11 — Optional browser extension

Only after the CLI/proxy system is stable.

---

# 41. TESTING STRATEGY

Do not consider a feature complete because it compiles.

Testing must include:

## Unit tests

- domain validation
- route registry
- configuration parsing
- certificate generation
- resolver logic
- command parsing

## Integration tests

```text
start local server
       ↓
register route
       ↓
request domain
       ↓
receive response
```

## HTTPS tests

Verify:

- certificate chain
- hostname
- SNI
- CA trust
- renewal/cache behavior

## WebSocket tests

Verify:

```text
browser/client
    ↓
Antra
    ↓
WebSocket server
```

## Lifecycle tests

Verify:

```text
Ctrl+C
SIGTERM
child exits
parent exits
crash
```

No orphan processes.

No stale routes.

No stale hosts entries.

---

# 42. SECURITY TESTING

Test:

- malicious host headers
- invalid domains
- path traversal
- malformed HTTP
- oversized headers
- certificate abuse
- unauthorized route registration
- arbitrary domain hijacking
- concurrent route updates
- stale IPC sockets
- permission errors
- malicious child processes
- hosts-file corruption

Never assume localhost means security is irrelevant.

---

# 43. LOGGING

Use structured logging.

Potential levels:

```text
ERROR
WARN
INFO
DEBUG
TRACE
```

Normal output should remain developer-friendly.

Example:

```text
✓ Proxy running
✓ app.localhost → 127.0.0.1:5173
```

Debug mode:

```bash
antra --verbose run ...
```

should expose detailed routing information.

Never log:

- CA private keys
- sensitive environment variables
- credentials
- cookies
- authorization headers

---

# 44. ERROR UX

Errors must explain:

1. What happened.
2. Why it happened.
3. How to fix it.

Bad:

```text
Error: Permission denied
```

Good:

```text
Unable to update the system hosts file.

Antra needs administrator privileges to register:

    myapp.com → 127.0.0.1

Try:
    sudo antra ...

Or use a .localhost domain, which does not require hosts-file modification.
```

---

# 45. NO TELEMETRY BY DEFAULT

Antra should not send developer traffic or project information anywhere.

No cloud dependency for core functionality.

Everything should work offline.

---

# 46. PERFORMANCE

Antra should be lightweight.

Goals:

- fast startup
- low idle memory
- minimal CPU
- efficient streaming
- no unnecessary polling
- no unnecessary child processes

Do not prematurely optimize.

Measure before optimizing.

---

# 47. OBSERVABILITY

Provide:

```bash
antra list
antra doctor
antra proxy status
```

Eventually potentially:

```bash
antra logs
```

The user should always be able to understand:

```text
What domain exists?
Where does it point?
Which process owns it?
Is the proxy running?
Is HTTPS working?
Is DNS/hosts working?
```

---

# 48. DOCUMENTATION

Write documentation as the project develops.

README must quickly explain:

```text
What is Antra?
Why does it exist?
Installation
Quick start
Custom domains
HTTPS
Multiple apps
Troubleshooting
Architecture
Security
```

The first README example should be extremely simple:

```bash
antra run --domain myapp.localhost -- pnpm dev
```

then:

```text
https://myapp.localhost
```

---

# 49. INSTALLATION

Eventually support common installation methods.

Examples:

```bash
brew install antra
```

and downloadable binaries.

Potential future:

```bash
cargo install antra
```

Do not assume package manager availability.

Release binaries for:

```text
macOS ARM64
macOS x86_64
Linux x86_64
Linux ARM64
Windows x86_64
```

where practical.

---

# 50. VERSIONING

Use semantic versioning.

Initial:

```text
0.1.0
```

Do not claim production readiness prematurely.

Maintain:

```text
CHANGELOG.md
```

---

# 51. ARCHITECTURAL QUALITY BAR

Do not create a giant monolithic `main.rs`.

Separate:

```text
CLI
configuration
process manager
proxy
routing
certificates
resolver
IPC
platform
```

Use clear interfaces.

Prefer dependency inversion for platform-specific operations.

Avoid global mutable state unless carefully justified.

Use asynchronous code only where appropriate.

---

# 52. IMPORTANT IMPLEMENTATION RULE

When you encounter an architectural uncertainty:

**STOP AND RESEARCH.**

Do not invent APIs.

Do not assume operating-system behavior.

Do not assume a browser will resolve a domain a particular way.

Do not assume a framework accepts `PORT`.

Do not assume certificate trust works identically across platforms.

Do not assume an existing crate's API from memory.

Research current documentation and source code where necessary.

Record important decisions.

---

# 53. DO NOT OVERENGINEER

Antra is a developer utility, not a distributed networking platform.

Do NOT introduce:

- databases unless necessary
- microservices
- cloud infrastructure
- REST APIs unless useful
- unnecessary abstraction layers
- complex plugin systems
- telemetry infrastructure
- GUI
- authentication
- user accounts

Keep the binary small and understandable.

---

# 54. DEFINITION OF DONE FOR MVP

The MVP is complete when this works reliably:

```bash
antra run --domain myapp.localhost -- pnpm dev
```

The terminal displays:

```text
ANTRA

✓ Proxy ready
✓ HTTPS ready
✓ Route registered

  https://myapp.localhost

  → 127.0.0.1:5173
```

Opening:

```text
https://myapp.localhost
```

loads the application.

HMR works.

WebSockets work.

Ctrl+C terminates the application.

The route disappears.

No orphan process remains.

No unrelated system configuration is damaged.

And the same fundamental workflow can work with:

```bash
antra run --domain api.myapp.localhost -- cargo run
```

and:

```bash
antra run --domain api.myapp.localhost -- python app.py
```

without Antra caring what runtime the application uses.

---

# 55. THE MOST IMPORTANT PRODUCT PRINCIPLE

The user should feel that Antra is almost invisible.

They should think:

```text
"My app runs at myapp.com."
```

not:

```text
"I have a Rust proxy that forwards localhost port 5173 through a hosts entry with a generated certificate."
```

All infrastructure should disappear behind a simple developer experience.

The ideal mental model is:

```text
                     ANTRA

         YOUR DOMAIN → YOUR LOCAL APP

              yapp.com
                  ↓
               Antra
                  ↓
          localhost:5173
```

---

# 56. FIRST TASK

Do NOT immediately start writing the complete application.

Start by acting as a senior systems architect.

Perform the research required to validate:

1. `.localhost` behavior.
2. `.test` behavior.
3. Arbitrary custom domain resolution.
4. `/etc/hosts` limitations.
5. macOS resolver behavior.
6. Linux resolver behavior.
7. Windows resolver behavior.
8. Local CA trust.
9. Dynamic SNI certificates.
10. Hyper + Rustls architecture.
11. WebSocket proxying.
12. Child-process lifecycle management.
13. Unix domain sockets.
14. Windows named pipes.
15. Port detection.
16. Framework behavior.
17. Portless architecture and limitations.
18. Existing Rust implementations.

Then produce:

```text
docs/architecture.md
docs/research/domain-resolution.md
docs/research/https.md
docs/research/process-management.md
docs/research/portless.md
docs/security.md
docs/mvp.md
```

Only after this research is complete should implementation begin.

---

# 57. AUTONOMOUS ENGINEERING MODE

You are authorized to:

- inspect the repository
- create files
- modify files
- install Rust dependencies
- run tests
- run local servers
- create integration tests
- inspect network behavior
- research current documentation
- refactor code
- fix compilation errors
- improve architecture
- improve documentation

But maintain these constraints:

### Never

- delete unrelated user files
- overwrite unrelated configuration
- expose secrets
- install system certificates without explicit user confirmation
- modify system DNS silently
- modify arbitrary domains without explicit intent
- disable security checks merely to make tests pass
- hide errors
- claim a feature works without testing it

---

# 58. WORK ITERATIVELY

After each major phase:

1. Build.
2. Run unit tests.
3. Run integration tests.
4. Manually verify the workflow.
5. Inspect logs.
6. Review security implications.
7. Update documentation.
8. Commit a coherent change if Git is available.

Do not accumulate hundreds of untested changes.

---

# 59. FINAL ENGINEERING PHILOSOPHY

Build Antra as if it will eventually be used daily by thousands of developers.

It should be:

```text
FAST
SMALL
SAFE
PREDICTABLE
CROSS-PLATFORM
LANGUAGE-AGNOSTIC
OFFLINE-FIRST
OPEN-SOURCE FRIENDLY
```

Most importantly:

**Make the common case incredibly simple.**

The experience we are ultimately building is:

```text
                BEFORE ANTRA

        pnpm dev
           ↓
    localhost:5173
           ↓
    "Which port was that?"
           ↓
      copy/paste URL


                AFTER ANTRA

        antra dev
           ↓
     https://myapp.com
           ↓
            done
```

That simplicity is the product.

Build the infrastructure underneath it carefully, but never let the infrastructure become the user experience.