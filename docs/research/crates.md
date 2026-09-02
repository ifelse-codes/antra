# Crate Research

## Dependencies

| Crate | Latest | License | MSRV | Purpose |
|-------|--------|---------|------|---------|
| tokio | 1.53.1 | MIT | 1.71 | Async runtime |
| hyper | 1.11.0 | MIT | 1.63 | HTTP server/client |
| hyper-util | 0.1.20 | MIT | ~1.63 | Server utilities |
| http-body-util | 0.1 | MIT | - | Body combinators |
| rustls | 0.23.43 | Apache-2.0/ISC/MIT | 1.71 | TLS |
| tokio-rustls | 0.26.4 | Apache-2.0/MIT | 1.71 | Async TLS |
| rcgen | 0.14.9 | MIT/Apache-2.0 | 1.88 | Cert generation |
| clap | 4.6.6 | MIT/Apache-2.0 | 1.74 | CLI parsing |
| tower | 0.5.2 | MIT | 1.49 | Middleware |
| tower-http | 0.7.0 | MIT | 1.65 | HTTP middleware |
| tracing | 0.1.44 | MIT | 1.65 | Structured logging |
| tracing-subscriber | 0.3 | MIT | - | Log formatting |
| serde | 1.0.228 | MIT/Apache-2.0 | 1.31 | Serialization |
| serde_json | 1 | MIT/Apache-2.0 | - | JSON |
| toml | 0.9.8 | MIT/Apache-2.0 | - | TOML config |
| os-truststore | 0.0.2 | TBD | - | Trust store mgmt |
| anyhow | 1 | MIT/Apache-2.0 | - | Error handling |
| dirs | 5 | MIT | - | Config dirs |
| colored | 2 | MIT | - | Terminal colors |
| nix | 0.29 | MIT | - | Unix APIs |

## MSRV Check

Our Rust version: 1.96.0
Highest MSRV requirement: rcgen 0.14.9 → Rust 1.88
✅ All crates compatible.

## Key Breaking Changes to Watch

### hyper 0.x → 1.x
- Body type is now a trait, use `http-body-util` for concrete types
- Service trait decoupled from Tower (use `hyper-util` for bridging)
- HTTP types from `http` crate (uppercase: `Method::GET`, `StatusCode::OK`)
- Features are opt-in: `client`, `server`, `http1`, `http2`, `full`

### rcgen 0.12 → 0.14
- API restructured: `CertificateParams` replaces builder patterns
- `KeyPair` renamed to `SigningKey`
- DER types now use `rustls-pki-types`

### clap 2.x/3.x → 4.x
- `App` renamed to `Command`
- Derive API is primary
- `ArgAction` enum for argument behavior

## License Notes

All core crates use permissive licenses (MIT, Apache-2.0, ISC). Safe for open-source and commercial use.

os-truststore license needs verification (0.0.2, may be early).

## Sources
- crates.io: https://crates.io/
- docs.rs: https://docs.rs/
- hyper migration guide: https://hyper.rs/guides/1/upgrading
