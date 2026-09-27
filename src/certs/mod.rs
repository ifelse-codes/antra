pub mod ca;
pub mod cache;
pub mod leaf;
pub mod store;
pub mod validate;

use time::{Duration, OffsetDateTime};

/// Validity window for the CA and every leaf it signs.
///
/// rcgen defaults to 1975→4096, which is outside Apple's ceiling of 825 days
/// for TLS server certificates (support.apple.com/en-us/103769). That limit
/// applies to custom roots as well — mkcert, the tool this project measures
/// itself against, bounds its certificates for the same reason. 800 days
/// keeps a margin under the cap and, unlike a calendar-arithmetic "2 years
/// 3 months", cannot drift over it in a leap year.
pub const VALIDITY_DAYS: i64 = 800;

/// `notBefore`/`notAfter` for a locally issued certificate.
///
/// Backdated one hour so a machine whose clock runs slightly behind ours does
/// not reject a certificate that was just created.
pub fn validity_window() -> (OffsetDateTime, OffsetDateTime) {
    let now = OffsetDateTime::now_utc();
    (
        now - Duration::hours(1),
        now + Duration::days(VALIDITY_DAYS),
    )
}

/// Short, stable fingerprint of a certificate, used to answer "is this the
/// same CA?" across processes (the daemon reports one, the CLI compares).
///
/// FNV-1a, not a cryptographic hash: it is a change detector, not an
/// integrity check, and it must produce the same value across Antra versions
/// and platforms. Adding a digest dependency for eight hex characters of
/// internal plumbing would be the wrong trade.
pub fn fingerprint(der: &[u8]) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x1000_0000_01b3;
    let mut hash = OFFSET;
    for byte in der {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}
