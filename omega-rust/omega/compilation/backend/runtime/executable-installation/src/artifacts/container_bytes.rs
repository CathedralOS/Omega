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


pub(crate) const OMEGA_EXECUTABLE_CONTAINER_MAGIC: [u8; 8] = *b"OMEGAXE!";

pub(crate) const OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES: u64 = 64;

pub(crate) const OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES: u64 = 32;

const RELOCATION_COUNT_BYTES: u64 = 8;

const RELOCATION_RECORD_BYTES: u64 = 32;

const ENTRY_RECORD_BYTES: u64 = 16;

const PLACEMENT_RECORD_BYTES: u64 = 64;

const SECTION_CODE: u16 = 1;

const SECTION_RELOCATIONS: u16 = 2;

const SECTION_CONTRACTS: u16 = 3;

const SECTION_FOOTPRINT: u16 = 4;

const SECTION_PLACEMENT: u16 = 5;

const SECTION_ENTRIES: u16 = 6;

const SECTION_INFORMATIONAL: u16 = 8;

const SECTION_AUTHORITY_COMMITMENTS: u16 = 9;

const AUTHORITY_COMMITMENT_BYTES: u64 = 128;

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
