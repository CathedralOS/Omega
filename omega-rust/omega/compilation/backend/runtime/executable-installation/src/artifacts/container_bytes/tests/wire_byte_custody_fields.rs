//! Substitutable-field inventory for the raw wire-byte half of the
//! executable-container wire custody matrix driven by `tests.rs`'s
//! `executable_container_wire_rejects_every_one_field_substitution`.
//!
//! The declared lanes are the fields the canonical encoder derives into the
//! container bytes: the header, each section-directory record's flags,
//! reserved bits, and per-kind identity rule, and the placement,
//! relocation, entry, and authority-commitment payload records. The codec
//! is hostile-input, so no byte axis may be carried silently: every lane
//! writes one raw field, or the smallest joined set of fields one
//! substitution needs, and `decode_executable_container` must refuse it
//! with the named fragment. A payload byte that decodes into different
//! semantics reaches the canonical content-fingerprint join instead of a
//! field check.
//!
//! Named axes that are deliberately not lanes: section payload offsets and
//! lengths, directory gaps, trailing bytes, informational sections, and a
//! version-1 relocation count are exercised by the focused decode tests
//! beside this matrix; the artifact-level substitutions the encoder carries
//! honestly are the artifact wire inventory.

use super::artifact_wire_custody_fields::ContainerWireRejection;
use crate::artifacts::container::OMEGA_EXECUTABLE_CONTAINER_V1_MARKER;
use crate::artifacts::container_bytes::{
    OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES, OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES,
    RELOCATION_COUNT_BYTES,
};
use optimization_core::MutationOutcome;

optimization_core::custody_field_inventory! {
    /// One substitutable wire field of a canonically encoded version-2
    /// executable container; checked by `decode_executable_container` in
    /// `executable_container_wire_rejects_every_one_field_substitution`.
    /// Several fields carry more than one lane when distinct substituted
    /// values reach distinct rejections (a zero section count and one above
    /// the bound, an absent flag and a non-boolean flag).
    pub enum ContainerWireByteFieldForTest {
        // Header fields.
        HeaderMagic,
        HeaderFormatMarker,
        HeaderBytes,
        HeaderArchitecture,
        HeaderReserved0,
        HeaderSectionCountZero,
        HeaderSectionCountAboveBound,
        HeaderDirectoryOffset,
        HeaderTotalLength,
        HeaderArtifactIdentity,
        HeaderContentFingerprint,
        HeaderReserved1,
        HeaderReserved2,
        // A version-2 container remarkered as version 1 still cannot
        // smuggle its authority section through the compatibility reader.
        HeaderFormatMarkerV1,
        // Section-record fields: flags, reserved, and the per-kind identity
        // rule.
        CodeRecordOptional,
        CodeRecordUnknownFlags,
        CodeRecordReserved,
        CodeRecordIdentity,
        RelocationRecordIdentityZero,
        RelocationRecordIdentity,
        ContractsRecordIdentity,
        PlacementRecordIdentity,
        ProofRecordIdentity,
        AuthorityRecordOptional,
        AuthorityRecordIdentity,
        // Payload fields.
        CodePayloadByte,
        PlacementPlan,
        PlacementRangePresence,
        PlacementPhase,
        PlacementRegimeAbsent,
        PlacementRegimePresence,
        PlacementRegimeIdentity,
        PlacementScopeAbsent,
        PlacementScopePresence,
        PlacementScopeIdentity,
        PlacementRangeValuesWhenAbsent,
        PlacementRangeInverted,
        PlacementAlignmentZero,
        PlacementAlignment,
        PlacementReserved0,
        PlacementReserved1,
        RelocationCountZero,
        RelocationCountAboveBound,
        RelocationKind,
        RelocationTargetKind,
        RelocationReserved,
        RelocationTargetIdentity,
        RelocationAddend,
        EntryIdentity,
        EntryCodeOffset,
        AuthorityDigestZero,
        // A short authority section keeps canonical tiling only by also
        // retiring the declared lengths.
        AuthoritySectionShortened,
    }
}

/// Byte offset of section-directory record `index`.
fn section_record(index: usize) -> usize {
    OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize
        + index * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize
}

/// Payload offset directory record `index` declares.
fn section_offset(bytes: &[u8], index: usize) -> usize {
    let record = section_record(index);
    u64::from_le_bytes(bytes[record + 16..record + 24].try_into().unwrap()) as usize
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

/// The fixture's canonical directory order.
const CODE: usize = 0;
const RELOCATIONS: usize = 1;
const CONTRACTS: usize = 2;
const PLACEMENT: usize = 4;
const ENTRIES: usize = 5;
const PROOF: usize = 6;
const AUTHORITY: usize = 7;

/// The family's substitution hook: write exactly the declared lane into the
/// canonically encoded `bytes`. Wire lanes take fixed non-canonical values;
/// the foreign `_donor` encoding only proves the driver's custody view
/// distinguishes authentic containers.
pub fn substitute_container_wire_byte_for_test(
    bytes: &mut Vec<u8>,
    field: ContainerWireByteFieldForTest,
    _donor: &Vec<u8>,
) {
    use ContainerWireByteFieldForTest as Lane;
    let placement = section_offset(bytes, PLACEMENT);
    let relocation = section_offset(bytes, RELOCATIONS) + RELOCATION_COUNT_BYTES as usize;
    match field {
        Lane::HeaderMagic => bytes[0] ^= 1,
        Lane::HeaderFormatMarker => write_u16(bytes, 8, 0x9999),
        Lane::HeaderBytes => write_u16(bytes, 10, 32),
        Lane::HeaderArchitecture => bytes[12] = 9,
        Lane::HeaderReserved0 => bytes[13] = 1,
        Lane::HeaderSectionCountZero => write_u16(bytes, 14, 0),
        Lane::HeaderSectionCountAboveBound => write_u16(bytes, 14, 17),
        Lane::HeaderDirectoryOffset => write_u64(bytes, 16, 128),
        Lane::HeaderTotalLength => {
            let beyond = bytes.len() as u64 + 1;
            write_u64(bytes, 24, beyond);
        }
        Lane::HeaderArtifactIdentity => write_u64(bytes, 32, 0),
        Lane::HeaderContentFingerprint => write_u64(bytes, 40, 0),
        Lane::HeaderReserved1 => write_u64(bytes, 48, 1),
        Lane::HeaderReserved2 => write_u64(bytes, 56, 1),
        Lane::HeaderFormatMarkerV1 => write_u16(bytes, 8, OMEGA_EXECUTABLE_CONTAINER_V1_MARKER),
        Lane::CodeRecordOptional => write_u16(bytes, section_record(CODE) + 2, 0),
        Lane::CodeRecordUnknownFlags => write_u16(bytes, section_record(CODE) + 2, 3),
        Lane::CodeRecordReserved => write_u32(bytes, section_record(CODE) + 4, 1),
        Lane::CodeRecordIdentity => write_u64(bytes, section_record(CODE) + 8, 1),
        Lane::RelocationRecordIdentityZero => {
            write_u64(bytes, section_record(RELOCATIONS) + 8, 0);
        }
        Lane::RelocationRecordIdentity => write_u64(bytes, section_record(RELOCATIONS) + 8, 99),
        Lane::ContractsRecordIdentity => write_u64(bytes, section_record(CONTRACTS) + 8, 99),
        Lane::PlacementRecordIdentity => write_u64(bytes, section_record(PLACEMENT) + 8, 99),
        Lane::ProofRecordIdentity => write_u64(bytes, section_record(PROOF) + 8, 1),
        Lane::AuthorityRecordOptional => write_u16(bytes, section_record(AUTHORITY) + 2, 0),
        Lane::AuthorityRecordIdentity => write_u64(bytes, section_record(AUTHORITY) + 8, 1),
        Lane::CodePayloadByte => {
            let code = section_offset(bytes, CODE);
            bytes[code] ^= 1;
        }
        Lane::PlacementPlan => write_u64(bytes, placement, 99),
        Lane::PlacementRangePresence => bytes[placement + 8] = 2,
        Lane::PlacementPhase => bytes[placement + 9] = 9,
        Lane::PlacementRegimeAbsent => bytes[placement + 10] = 0,
        Lane::PlacementRegimePresence => bytes[placement + 10] = 2,
        Lane::PlacementRegimeIdentity => write_u64(bytes, placement + 40, 0),
        Lane::PlacementScopeAbsent => bytes[placement + 11] = 0,
        Lane::PlacementScopePresence => bytes[placement + 11] = 2,
        Lane::PlacementScopeIdentity => write_u64(bytes, placement + 48, 0),
        Lane::PlacementRangeValuesWhenAbsent => write_u64(bytes, placement + 16, 1),
        Lane::PlacementRangeInverted => {
            bytes[placement + 8] = 1;
            write_u64(bytes, placement + 16, 0x2000);
            write_u64(bytes, placement + 24, 0x1000);
        }
        Lane::PlacementAlignmentZero => write_u64(bytes, placement + 32, 0),
        Lane::PlacementAlignment => write_u64(bytes, placement + 32, 8),
        Lane::PlacementReserved0 => write_u32(bytes, placement + 12, 1),
        Lane::PlacementReserved1 => write_u64(bytes, placement + 56, 1),
        Lane::RelocationCountZero => {
            write_u64(bytes, relocation - RELOCATION_COUNT_BYTES as usize, 0);
        }
        Lane::RelocationCountAboveBound => {
            write_u64(bytes, relocation - RELOCATION_COUNT_BYTES as usize, 17);
        }
        Lane::RelocationKind => write_u16(bytes, relocation, 9),
        Lane::RelocationTargetKind => write_u16(bytes, relocation + 2, 9),
        Lane::RelocationReserved => write_u32(bytes, relocation + 4, 1),
        Lane::RelocationTargetIdentity => write_u64(bytes, relocation + 16, 0),
        Lane::RelocationAddend => write_u64(bytes, relocation + 24, 1),
        Lane::EntryIdentity => {
            let entry = section_offset(bytes, ENTRIES);
            write_u64(bytes, entry, 0);
        }
        Lane::EntryCodeOffset => {
            let entry = section_offset(bytes, ENTRIES);
            write_u64(bytes, entry + 8, 24);
        }
        Lane::AuthorityDigestZero => {
            let authority = section_offset(bytes, AUTHORITY);
            bytes[authority..authority + 32].fill(0);
        }
        Lane::AuthoritySectionShortened => {
            let authority = section_offset(bytes, AUTHORITY);
            bytes.truncate(authority + 64);
            write_u64(bytes, section_record(AUTHORITY) + 24, 64);
            let total_length = bytes.len() as u64;
            write_u64(bytes, 24, total_length);
        }
    }
}

/// The exact decode rejection each declared lane must produce.
pub fn container_wire_byte_custody_outcome(
    field: ContainerWireByteFieldForTest,
) -> MutationOutcome<ContainerWireRejection> {
    use ContainerWireByteFieldForTest as Lane;
    let fragment = match field {
        Lane::HeaderMagic => "invalid magic",
        Lane::HeaderFormatMarker => "unsupported Omega executable container marker",
        Lane::HeaderBytes => "header length 32 is not canonical 64",
        Lane::HeaderArchitecture => "unknown executable-container architecture 9",
        Lane::HeaderReserved0 => "container header reserved0",
        Lane::HeaderSectionCountZero => "has 0 sections",
        Lane::HeaderSectionCountAboveBound => "has 17 sections",
        Lane::HeaderDirectoryOffset => "directory offset 128 is not canonical",
        Lane::HeaderTotalLength => "declares",
        Lane::HeaderArtifactIdentity => "normalized artifact identity cannot be zero",
        Lane::HeaderContentFingerprint => "fingerprint cannot be zero",
        Lane::HeaderReserved1 => "container header reserved1",
        Lane::HeaderReserved2 => "container header reserved2",
        Lane::HeaderFormatMarkerV1 => "container-v1 cannot carry a v2 authority-commitment section",
        Lane::CodeRecordOptional => "must be required",
        Lane::CodeRecordUnknownFlags => "unknown flags",
        Lane::CodeRecordReserved => "section record reserved must be zero",
        Lane::CodeRecordIdentity | Lane::ProofRecordIdentity | Lane::AuthorityRecordIdentity => {
            "must use zero wire identity"
        }
        Lane::RelocationRecordIdentityZero => "requires a nonzero normalized identity",
        Lane::ContractsRecordIdentity | Lane::PlacementRecordIdentity | Lane::PlacementPlan => {
            "payload identity"
        }
        Lane::AuthorityRecordOptional => "container-v2 authority commitments must be required",
        Lane::RelocationRecordIdentity
        | Lane::CodePayloadByte
        | Lane::PlacementAlignment
        | Lane::RelocationAddend
        | Lane::EntryCodeOffset => "content fingerprint",
        Lane::PlacementRangePresence => "placement range presence flag 2 is not boolean",
        Lane::PlacementPhase => "unknown placement phase 9",
        Lane::PlacementRegimeAbsent => "machine regime identity must be zero when absent",
        Lane::PlacementRegimePresence => "machine regime presence flag 2 is not boolean",
        Lane::PlacementRegimeIdentity => "machine regime identity cannot be zero when present",
        Lane::PlacementScopeAbsent => "installation scope identity must be zero when absent",
        Lane::PlacementScopePresence => "installation scope presence flag 2 is not boolean",
        Lane::PlacementScopeIdentity => "installation scope identity cannot be zero when present",
        Lane::PlacementRangeValuesWhenAbsent => "placement range values must be zero when absent",
        Lane::PlacementRangeInverted => "placement range is invalid",
        Lane::PlacementAlignmentZero => "placement alignment must be nonzero",
        Lane::PlacementReserved0 | Lane::PlacementReserved1 => {
            "placement reserved field must be zero"
        }
        Lane::RelocationCountZero => "does not match 0 canonical records",
        Lane::RelocationCountAboveBound => "exceeding configured bound",
        Lane::RelocationKind => "unknown artifact relocation kind 9",
        Lane::RelocationTargetKind => "unknown artifact relocation target kind 9",
        Lane::RelocationReserved => "relocation reserved",
        Lane::RelocationTargetIdentity | Lane::EntryIdentity => {
            "normalized entry stub identity cannot be zero"
        }
        Lane::AuthorityDigestZero => "authority commitments cannot be zero",
        Lane::AuthoritySectionShortened => "must contain exactly 128 bytes",
    };
    MutationOutcome::ExactError(ContainerWireRejection::Decode(fragment))
}

/// Every verdict the wire-byte family declares, for the checker's
/// classification.
pub fn container_wire_byte_vocabulary() -> impl Iterator<Item = ContainerWireRejection> {
    ContainerWireByteFieldForTest::INVENTORY
        .iter()
        .map(|&field| match container_wire_byte_custody_outcome(field) {
            MutationOutcome::ExactError(verdict) => verdict,
            MutationOutcome::RebuiltCustodyDiffers => {
                unreachable!("container wire lanes declare exact rejections")
            }
        })
}
