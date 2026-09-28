//!
//! This file owns the container constants, the wire section and the
//! encode and decode entry points. `encoding.rs` writes one container
//! version, `decoding.rs` reads and validates a container,
//! `record_layouts.rs` carries the fixed record layouts and schemas and
//! `tests.rs` the container tests.

mod decoding;
mod encoding;
mod record_layouts;
#[cfg(test)]
mod tests;

pub use decoding::decode_executable_container;

use crate::artifacts::Artifact;
use crate::artifacts::container::ContainerLimits;
use crate::authority_digests::NonAuthoritativeInformationalFingerprint64;
use crate::installation::InstallationDiagnostic;

use crate::artifacts::container_bytes::encoding::encode_executable_container_version;

pub const OMEGA_EXECUTABLE_CONTAINER_MAGIC: [u8; 8] = *b"OMEGAXE!";

pub const OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES: u64 = 64;

pub const OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES: u64 = 32;

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

const SECTION_PROOF: u16 = 7;

const SECTION_INFORMATIONAL: u16 = 8;

const SECTION_AUTHORITY_COMMITMENTS: u16 = 9;

const AUTHORITY_COMMITMENT_BYTES: u64 = 128;

pub fn non_authoritative_informational_section_fingerprint(
    kind: u16,
    payload: &[u8],
) -> NonAuthoritativeInformationalFingerprint64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in b"omega.executable-container.information.v1"
        .iter()
        .copied()
        .chain(kind.to_le_bytes())
        .chain((payload.len() as u64).to_le_bytes())
        .chain(payload.iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
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

/// Emits the canonical Omega-native byte container for one compiler-produced
/// artifact candidate and exact proof payload.
///
/// The encoder publishes no optional informational sections and derives the
/// proof identity from the exact bytes. Before returning, it routes its own
/// output through the hostile-input decoder and semantic validator. An encoder
/// bug therefore fails closed instead of producing a container that a later
/// loader interprets differently.
pub fn encode_executable_container(
    artifact: &Artifact,
    proof: &[u8],
    limits: ContainerLimits,
) -> Result<Vec<u8>, InstallationDiagnostic> {
    encode_executable_container_version(artifact, proof, limits, true)
}

/// Re-encodes a decoded version-1 compatibility candidate byte-for-byte.
/// Version 1 has no strong imported-authority section and cannot be admitted.
pub fn encode_executable_container_v1_compatibility(
    artifact: &Artifact,
    proof: &[u8],
    limits: ContainerLimits,
) -> Result<Vec<u8>, InstallationDiagnostic> {
    encode_executable_container_version(artifact, proof, limits, false)
}
