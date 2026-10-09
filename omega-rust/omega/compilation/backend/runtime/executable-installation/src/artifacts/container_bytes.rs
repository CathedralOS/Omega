//!
//! This file owns the container constants, the wire section and the
//! encode and decode entry points. `encoding.rs` writes one container
//! version, `decoding.rs` reads and validates a container,
//! `record_layouts.rs` carries the fixed record layouts and schemas and
//! `tests.rs` the container tests.

use crate::artifacts::Artifact;
use crate::artifacts::container::ContainerLimits;
use crate::authority_digests::NonAuthoritativeInformationalFingerprint64;
use crate::installation::InstallationDiagnostic;


pub(crate) fn non_authoritative_informational_section_fingerprint(
    kind: u16,
    payload: &[u8],
) -> NonAuthoritativeInformationalFingerprint64 {
    let mut hash = crate::fnv1a::Fnv1a::new();
    hash.bytes(b"omega.executable-container.information.v1");
    hash.u16(kind);
    hash.u64(payload.len() as u64);
    hash.bytes(payload);
    let hash = hash.finish();
    NonAuthoritativeInformationalFingerprint64::from_compatibility_value(if hash == 0 {
        1
    } else {
        hash
    })
    .expect("fixed FNV normalization replaces zero")
}

#[derive(Debug, Clone, Copy)]
struct WireSection {
    kind: u16,
    required: bool,
    identity: u64,
    offset: u64,
    length: u64,
}
