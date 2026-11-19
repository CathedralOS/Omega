#![forbid(unsafe_code)]

/// Fingerprint of this compiler's sources, toolchain, target, and build profile.
pub const fn compiler_identity() -> [u8; 32] {
    *include_bytes!(concat!(env!("OUT_DIR"), "/compiler-identity.bin"))
}
