//! Executable container byte tests.

use crate::artifacts::Artifact;
use crate::artifacts::ArtifactEntry;
use crate::artifacts::container::ArtifactRelocationKind;
use crate::artifacts::container::ContainerLimits;
use crate::artifacts::container::DecodedArtifactContainer;
use crate::artifacts::container::DecodedArtifactRelocation;
use crate::artifacts::container::OMEGA_EXECUTABLE_CONTAINER_V1_MARKER;
use crate::artifacts::container::OMEGA_EXECUTABLE_CONTAINER_V2_MARKER;
use crate::artifacts::container::normalized_proof_payload_digest;
use crate::artifacts::container::{
    ValidatedContainerAdmissionEvidence, admit_validated_container,
    non_authoritative_decoded_container_fingerprint,
};
use crate::artifacts::container_bytes::decoding::decode_executable_container;
use crate::artifacts::container_bytes::encoding::encode_record;
use crate::artifacts::container_bytes::record_layouts::entry_layout;
use crate::artifacts::container_bytes::record_layouts::header_layout;
use crate::artifacts::container_bytes::record_layouts::identity_layout;
use crate::artifacts::container_bytes::record_layouts::placement_layout;
use crate::artifacts::container_bytes::record_layouts::relocation_layout;
use crate::artifacts::container_bytes::record_layouts::section_layout;
use crate::artifacts::container_bytes::{
    ENTRY_RECORD_BYTES, OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES, OMEGA_EXECUTABLE_CONTAINER_MAGIC,
    OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES, PLACEMENT_RECORD_BYTES,
    RELOCATION_COUNT_BYTES, SECTION_CODE, SECTION_CONTRACTS, SECTION_ENTRIES, SECTION_FOOTPRINT,
    SECTION_INFORMATIONAL, SECTION_PLACEMENT, SECTION_PROOF, SECTION_RELOCATIONS,
    encode_executable_container, encode_executable_container_v1_compatibility,
    non_authoritative_informational_section_fingerprint,
};
use crate::authority_digests::AdmissionReceiptId;
use crate::authority_digests::ArtifactAuthorityCommitments;
use crate::authority_digests::ArtifactId;
use crate::authority_digests::EntrySetId;
use crate::authority_digests::MachineContractSetId;
use crate::authority_digests::MachineFootprintId;
use crate::authority_digests::NonAuthoritativeContainerFingerprint64;
use crate::authority_digests::PlacementPlanId;
use crate::authority_digests::RelocationSetId;
use layout_plans::ArtifactInstallationScopeId;
use layout_plans::EntryStubId;
use layout_plans::LayoutPlanReport;
use layout_plans::MachineRegimeId;
use layout_plans::PlacementConstraints;
use layout_plans::PlacementPhase;
use layout_plans::RelocationTarget;
use target::Architecture;

fn limits() -> ContainerLimits {
    ContainerLimits {
        max_total_bytes: 4096,
        max_sections: 16,
        max_section_bytes: 1024,
        max_relocations: 16,
    }
}

fn write_record(destination: &mut [u8], layout: LayoutPlanReport, values: &[(&str, u16, u64)]) {
    encode_record(destination, layout, values, "test record").expect("test record materializes");
}

fn canonical_bytes() -> Vec<u8> {
    let section_count = 7_u64;
    let directory_end = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES
        + section_count * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES;
    let code_offset = directory_end;
    let relocation_offset = code_offset + 64;
    let contracts_offset = relocation_offset + 40;
    let footprint_offset = contracts_offset + 8;
    let placement_offset = footprint_offset + 8;
    let entries_offset = placement_offset + PLACEMENT_RECORD_BYTES;
    let proof_offset = entries_offset + ENTRY_RECORD_BYTES;
    let total_length = proof_offset + 64;

    let contracts = MachineContractSetId::from_normalized_identity(3).unwrap();
    let footprint = MachineFootprintId::from_normalized_identity(4).unwrap();
    let placement = PlacementPlanId::from_normalized_identity(5).unwrap();
    let relocation_set = RelocationSetId::from_normalized_identity(6).unwrap();
    let entry_set = EntrySetId::from_normalized_identity(8).unwrap();
    let entry = EntryStubId::from_normalized_identity(9).unwrap();
    let proof = vec![0xa5; 64];
    let mut decoded = DecodedArtifactContainer {
        format_marker: OMEGA_EXECUTABLE_CONTAINER_V1_MARKER,
        total_length,
        artifact: ArtifactId::from_normalized_identity(1).unwrap(),
        content_fingerprint: NonAuthoritativeContainerFingerprint64::from_compatibility_value(2)
            .unwrap(),
        architecture: Architecture::X86_64,
        code_length: 64,
        code: vec![0x90; 64],
        contracts,
        declared_footprint: footprint,
        placement_plan: placement,
        placement_constraints: PlacementConstraints::unconstrained(PlacementPhase::Load),
        entry_set,
        entries: vec![ArtifactEntry::from_canonical_decode(entry, 16)],
        relocation_set,
        relocations: vec![DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::X86Relative32,
            destination_offset: 32,
            target: RelocationTarget::Entry(entry),
            addend: -4,
        }],
        proof_payload: normalized_proof_payload_digest(&proof),
        proof,
        authority_commitments: None,
        sections: Vec::new(),
    };
    decoded.content_fingerprint =
        non_authoritative_decoded_container_fingerprint(&decoded).unwrap();

    let mut bytes = vec![0_u8; total_length as usize];
    write_record(
        &mut bytes[..OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize],
        header_layout(),
        &[
            (
                "magic",
                64,
                u64::from_le_bytes(OMEGA_EXECUTABLE_CONTAINER_MAGIC),
            ),
            (
                "format_marker",
                16,
                u64::from(OMEGA_EXECUTABLE_CONTAINER_V1_MARKER),
            ),
            ("header_bytes", 16, OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES),
            ("architecture", 8, 2),
            ("reserved0", 8, 0),
            ("section_count", 16, section_count),
            (
                "directory_offset",
                64,
                OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES,
            ),
            ("total_length", 64, total_length),
            ("artifact", 64, 1),
            (
                "content",
                64,
                decoded.content_fingerprint.compatibility_value(),
            ),
            ("reserved1", 64, 0),
            ("reserved2", 64, 0),
        ],
    );
    let sections = [
        (SECTION_CODE, 1, 0, code_offset, 64),
        (
            SECTION_RELOCATIONS,
            1,
            relocation_set.normalized_identity(),
            relocation_offset,
            40,
        ),
        (
            SECTION_CONTRACTS,
            1,
            contracts.normalized_identity(),
            contracts_offset,
            8,
        ),
        (
            SECTION_FOOTPRINT,
            1,
            footprint.normalized_identity(),
            footprint_offset,
            8,
        ),
        (
            SECTION_PLACEMENT,
            1,
            placement.normalized_identity(),
            placement_offset,
            PLACEMENT_RECORD_BYTES,
        ),
        (
            SECTION_ENTRIES,
            1,
            entry_set.normalized_identity(),
            entries_offset,
            ENTRY_RECORD_BYTES,
        ),
        (SECTION_PROOF, 1, 0, proof_offset, 64),
    ];
    for (index, (kind, flags, identity, offset, length)) in sections.iter().enumerate() {
        let start = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize
            + index * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize;
        let end = start + OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize;
        write_record(
            &mut bytes[start..end],
            section_layout(),
            &[
                ("kind", 16, u64::from(*kind)),
                ("flags", 16, *flags),
                ("reserved", 32, 0),
                ("identity", 64, *identity),
                ("offset", 64, *offset),
                ("length", 64, *length),
            ],
        );
    }
    bytes[code_offset as usize..relocation_offset as usize].fill(0x90);
    write_record(
        &mut bytes[relocation_offset as usize..relocation_offset as usize + 8],
        identity_layout(),
        &[("identity", 64, 1)],
    );
    write_record(
        &mut bytes[relocation_offset as usize + 8..contracts_offset as usize],
        relocation_layout(),
        &[
            ("kind", 16, 2),
            ("target_kind", 16, 1),
            ("reserved", 32, 0),
            ("destination", 64, 32),
            ("target", 64, 9),
            ("addend", 64, (-4_i64) as u64),
        ],
    );
    write_record(
        &mut bytes[contracts_offset as usize..footprint_offset as usize],
        identity_layout(),
        &[("identity", 64, contracts.normalized_identity())],
    );
    write_record(
        &mut bytes[footprint_offset as usize..placement_offset as usize],
        identity_layout(),
        &[("identity", 64, footprint.normalized_identity())],
    );
    write_record(
        &mut bytes[placement_offset as usize..entries_offset as usize],
        placement_layout(),
        &[
            ("plan", 64, placement.normalized_identity()),
            ("range_present", 8, 0),
            ("phase", 8, 2),
            ("regime_present", 8, 0),
            ("scope_present", 8, 0),
            ("reserved0", 32, 0),
            ("range_start", 64, 0),
            ("range_end", 64, 0),
            ("alignment", 64, 1),
            ("regime", 64, 0),
            ("scope", 64, 0),
            ("reserved1", 64, 0),
        ],
    );
    write_record(
        &mut bytes[entries_offset as usize..proof_offset as usize],
        entry_layout(),
        &[("identity", 64, 9), ("offset", 64, 16)],
    );
    bytes[proof_offset as usize..].fill(0xa5);
    bytes
}

fn strong_artifact() -> Artifact {
    let legacy = decode_executable_container(&canonical_bytes(), limits())
        .expect("legacy compatibility candidate");
    let source = legacy.artifact();
    let regime = MachineRegimeId::from_normalized_identity(10).expect("machine regime");
    let scope =
        ArtifactInstallationScopeId::from_normalized_identity(11).expect("installation scope");
    let placement_constraints =
        PlacementConstraints::new(None, 1, PlacementPhase::Load, Some(regime), Some(scope))
            .expect("strong placement constraints");
    let commitments = ArtifactAuthorityCommitments::from_canonical_evidence(
        source.0.contracts,
        b"canonical imported contract set",
        source.0.declared_footprint,
        b"canonical declared footprint",
        Some((regime, b"canonical machine regime")),
        Some((scope, b"canonical installation scope")),
    );
    Artifact::from_canonical_decode(
        source.0.identity,
        source.0.architecture,
        source.0.code.clone(),
        source.0.contracts,
        source.0.declared_footprint,
        source.0.placement_plan,
        placement_constraints,
        source.0.entry_set,
        source.0.entries.clone(),
        source.0.relocation_set,
        source.0.relocations.clone(),
        commitments,
    )
    .expect("strong artifact")
}

fn add_optional_section(mut bytes: Vec<u8>, kind: u16, payload: &[u8]) -> Vec<u8> {
    let old_section_count = 7_usize;
    let inserted_directory_offset = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize
        + old_section_count * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize;
    bytes.splice(
        inserted_directory_offset..inserted_directory_offset,
        std::iter::repeat_n(0, OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize),
    );
    for index in 0..old_section_count {
        let record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize
            + index * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize;
        let offset = u64::from_le_bytes(bytes[record + 16..record + 24].try_into().unwrap());
        bytes[record + 16..record + 24].copy_from_slice(
            &(offset + OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES).to_le_bytes(),
        );
    }
    let payload_offset = bytes.len() as u64;
    bytes.extend_from_slice(payload);
    bytes[14..16].copy_from_slice(&8_u16.to_le_bytes());
    let total_length = bytes.len() as u64;
    bytes[24..32].copy_from_slice(&total_length.to_le_bytes());
    let identity =
        non_authoritative_informational_section_fingerprint(kind, payload).compatibility_value();
    write_record(
        &mut bytes[inserted_directory_offset
            ..inserted_directory_offset + OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize],
        section_layout(),
        &[
            ("kind", 16, u64::from(kind)),
            ("flags", 16, 0),
            ("reserved", 32, 0),
            ("identity", 64, identity),
            ("offset", 64, payload_offset),
            ("length", 64, payload.len() as u64),
        ],
    );
    bytes
}

#[test]
fn canonical_bytes_decode_through_layouts_into_validated_candidate() {
    let bytes = canonical_bytes();
    let decoded = decode_executable_container(&bytes, limits()).expect("canonical decode");
    assert_eq!(decoded.artifact().code(), vec![0x90; 64]);
    assert_eq!(decoded.artifact().entries()[0].code_offset(), 16);
    assert_eq!(decoded.relocations()[0].addend, -4);
    assert_eq!(decoded.proof(), vec![0xa5; 64]);
}

#[test]
fn stale_container_marker_bytes_reject() {
    let mut stale = canonical_bytes();
    stale[8..10].copy_from_slice(b"NO");
    let error = decode_executable_container(&stale, limits()).expect_err("stale marker");
    assert!(
        error
            .0
            .contains("unsupported Omega executable container marker")
    );
}

#[test]
fn canonical_encoder_round_trips_the_exact_validated_artifact_and_proof() {
    let canonical = canonical_bytes();
    let source = decode_executable_container(&canonical, limits()).expect("source");
    let encoded =
        encode_executable_container_v1_compatibility(source.artifact(), source.proof(), limits())
            .expect("encode");
    assert_eq!(
        encoded, canonical,
        "container-v1 wire bytes must remain stable"
    );
    assert_eq!(
        u64::from_le_bytes(encoded[40..48].try_into().unwrap()),
        source
            .artifact()
            .non_authoritative_container_fingerprint()
            .compatibility_value()
    );
    let decoded = decode_executable_container(&encoded, limits()).expect("round trip");
    assert_eq!(decoded.artifact(), source.artifact());
    assert_eq!(decoded.proof(), source.proof());
    assert_eq!(decoded.proof_payload(), source.proof_payload());
}

#[test]
fn container_v2_round_trips_strong_authority_commitments() {
    let artifact = strong_artifact();
    let proof = vec![0xa5; 64];
    let encoded = encode_executable_container(&artifact, &proof, limits()).expect("encode v2");
    assert_eq!(
        u16::from_le_bytes(encoded[8..10].try_into().unwrap()),
        OMEGA_EXECUTABLE_CONTAINER_V2_MARKER,
    );
    assert_eq!(u16::from_le_bytes(encoded[14..16].try_into().unwrap()), 8);

    let decoded = decode_executable_container(&encoded, limits()).expect("decode v2");
    assert_eq!(decoded.artifact(), &artifact);
    assert_eq!(
        decoded.artifact().authority_commitments(),
        artifact.authority_commitments(),
    );
}

#[test]
fn compact_equal_authority_digest_substitutions_cannot_replay_admission() {
    let artifact = strong_artifact();
    let proof = vec![0xa5; 64];
    let encoded = encode_executable_container(&artifact, &proof, limits()).expect("encode v2");
    let original = decode_executable_container(&encoded, limits()).expect("original v2");
    let authority_record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize + 7 * 32;
    let authority_offset = u64::from_le_bytes(
        encoded[authority_record + 16..authority_record + 24]
            .try_into()
            .unwrap(),
    ) as usize;

    for digest_offset in [0, 32, 64, 96] {
        let mut substituted = encoded.clone();
        substituted[authority_offset + digest_offset] ^= 1;
        let substituted =
            decode_executable_container(&substituted, limits()).expect("substituted v2");
        assert_eq!(
            substituted.artifact().0.contracts,
            original.artifact().0.contracts,
            "the adversary preserves the compact contract report identity",
        );
        assert_eq!(
            substituted.artifact().0.declared_footprint,
            original.artifact().0.declared_footprint,
            "the adversary preserves the compact footprint report identity",
        );
        assert_eq!(
            substituted.artifact().0.placement_constraints,
            original.artifact().0.placement_constraints,
            "the adversary preserves compact regime/scope report identities",
        );
        assert_ne!(
            substituted.artifact().content(),
            original.artifact().content(),
            "each strong authority commitment participates in executable content identity",
        );

        let evidence = ValidatedContainerAdmissionEvidence::from_validator(
            AdmissionReceiptId::from_normalized_identity(70).unwrap(),
            &original,
            true,
        );
        let error = admit_validated_container(&substituted, evidence)
            .expect_err("strong authority substitution must not replay admission");
        assert!(error.0.contains("different validated container"));
    }
}

#[test]
fn container_v1_decodes_for_compatibility_but_cannot_be_admitted() {
    let legacy = decode_executable_container(&canonical_bytes(), limits()).expect("legacy decode");
    assert!(legacy.artifact().authority_commitments().is_none());
    let evidence = ValidatedContainerAdmissionEvidence::from_validator(
        AdmissionReceiptId::from_normalized_identity(70).unwrap(),
        &legacy,
        true,
    );
    let error = admit_validated_container(&legacy, evidence)
        .expect_err("v1 compact-only authority must not admit");
    assert!(error.0.contains("container-v1 compatibility"));
}

#[test]
fn optional_section_trace_identity_is_derived_from_exact_opaque_bytes() {
    let known = add_optional_section(canonical_bytes(), SECTION_INFORMATIONAL, b"debug");
    let decoded = decode_executable_container(&known, limits()).expect("known information");
    assert_eq!(
        decoded.informational_sections()[0].compatibility_value(),
        non_authoritative_informational_section_fingerprint(SECTION_INFORMATIONAL, b"debug")
            .compatibility_value()
    );

    let unknown = add_optional_section(canonical_bytes(), 99, b"future");
    let decoded = decode_executable_container(&unknown, limits()).expect("unknown information");
    assert_eq!(
        decoded.unknown_informational_sections(),
        &[
            non_authoritative_informational_section_fingerprint(99, b"future")
                .compatibility_value()
        ]
    );

    let mut substituted = known;
    *substituted.last_mut().expect("payload byte") ^= 1;
    let error = decode_executable_container(&substituted, limits()).expect_err("identity replay");
    assert!(error.0.contains("does not match its exact opaque bytes"));
}

#[test]
fn truncation_directory_overlap_and_bad_reserved_bits_reject() {
    let mut truncated = canonical_bytes();
    truncated.pop();
    let error = decode_executable_container(&truncated, limits()).expect_err("truncation");
    assert!(error.0.contains("declares"));

    let mut prefix_overlap = canonical_bytes();
    let first_section = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize;
    prefix_overlap[first_section + 16..first_section + 24].copy_from_slice(&64_u64.to_le_bytes());
    let error = decode_executable_container(&prefix_overlap, limits()).expect_err("prefix overlap");
    assert!(error.0.contains("header/directory"));

    let mut reserved = canonical_bytes();
    reserved[13] = 1;
    let error = decode_executable_container(&reserved, limits()).expect_err("reserved");
    assert!(error.0.contains("reserved0"));
}

#[test]
fn payload_gaps_and_unreferenced_trailing_bytes_reject() {
    let mut gap = canonical_bytes();
    let code_record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize;
    let code_offset =
        u64::from_le_bytes(gap[code_record + 16..code_record + 24].try_into().unwrap());
    gap[code_record + 16..code_record + 24].copy_from_slice(&(code_offset + 1).to_le_bytes());
    gap[code_record + 24..code_record + 32].copy_from_slice(&63_u64.to_le_bytes());
    let error = decode_executable_container(&gap, limits()).expect_err("payload gap");
    assert!(error.0.contains("leaves a gap"));

    let mut trailing = canonical_bytes();
    let proof_record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize + 6 * 32;
    trailing[proof_record + 24..proof_record + 32].copy_from_slice(&63_u64.to_le_bytes());
    let error =
        decode_executable_container(&trailing, limits()).expect_err("unreferenced trailing");
    assert!(error.0.contains("unreferenced bytes"));
}

#[test]
fn malformed_counts_unknown_required_and_identity_drift_reject() {
    let mut count = canonical_bytes();
    let relocation_section_offset = 64 + 32;
    let relocation_payload = u64::from_le_bytes(
        count[relocation_section_offset + 16..relocation_section_offset + 24]
            .try_into()
            .unwrap(),
    ) as usize;
    count[relocation_payload..relocation_payload + 8].copy_from_slice(&2_u64.to_le_bytes());
    let error = decode_executable_container(&count, limits()).expect_err("count mismatch");
    assert!(error.0.contains("does not match 2"));

    let mut unknown = canonical_bytes();
    let code_record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize;
    unknown[code_record..code_record + 2].copy_from_slice(&99_u16.to_le_bytes());
    unknown[code_record + 8..code_record + 16].copy_from_slice(&99_u64.to_le_bytes());
    let error = decode_executable_container(&unknown, limits()).expect_err("unknown required");
    assert!(error.0.contains("unknown required"));

    let mut identity = canonical_bytes();
    let contracts_record = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize + 2 * 32;
    identity[contracts_record + 8..contracts_record + 16].copy_from_slice(&99_u64.to_le_bytes());
    let error = decode_executable_container(&identity, limits()).expect_err("identity drift");
    assert!(error.0.contains("payload identity"));
}

#[test]
fn semantic_byte_drift_reaches_normalized_content_check() {
    let mut bytes = canonical_bytes();
    let directory_end = OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES
        + 7 * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES;
    bytes[directory_end as usize] ^= 1;
    let error = decode_executable_container(&bytes, limits()).expect_err("content drift");
    assert!(error.0.contains("content fingerprint"));
}

/// Mutable copy of every `Artifact` field so each representable axis can be
/// substituted once while `Artifact::from_canonical_decode` honestly recomputes
/// the containing content digest and container fingerprint.
struct ArtifactSeed {
    identity: ArtifactId,
    architecture: Architecture,
    code: Vec<u8>,
    contracts: MachineContractSetId,
    declared_footprint: MachineFootprintId,
    placement_plan: PlacementPlanId,
    placement_constraints: PlacementConstraints,
    entry_set: EntrySetId,
    entries: Vec<ArtifactEntry>,
    relocation_set: RelocationSetId,
    relocations: Vec<DecodedArtifactRelocation>,
    authority_commitments: ArtifactAuthorityCommitments,
}

impl ArtifactSeed {
    fn from(artifact: &Artifact) -> Self {
        Self {
            identity: artifact.0.identity,
            architecture: artifact.0.architecture,
            code: artifact.0.code.clone(),
            contracts: artifact.0.contracts,
            declared_footprint: artifact.0.declared_footprint,
            placement_plan: artifact.0.placement_plan,
            placement_constraints: artifact.0.placement_constraints,
            entry_set: artifact.0.entry_set,
            entries: artifact.0.entries.clone(),
            relocation_set: artifact.0.relocation_set,
            relocations: artifact.0.relocations.clone(),
            authority_commitments: artifact
                .0
                .authority_commitments
                .expect("strong fixture carries commitments"),
        }
    }

    /// Honestly recompute the strong commitments for the seed's current report
    /// coordinates, matching the canonical evidence the fixture provider held.
    fn recommit(&mut self) {
        self.authority_commitments = ArtifactAuthorityCommitments::from_canonical_evidence(
            self.contracts,
            b"canonical imported contract set",
            self.declared_footprint,
            b"canonical declared footprint",
            self.placement_constraints
                .machine_regime()
                .map(|regime| (regime, b"canonical machine regime".as_slice())),
            self.placement_constraints
                .installation_scope()
                .map(|scope| (scope, b"canonical installation scope".as_slice())),
        );
    }

    fn into_artifact(self) -> Artifact {
        Artifact::from_canonical_decode(
            self.identity,
            self.architecture,
            self.code,
            self.contracts,
            self.declared_footprint,
            self.placement_plan,
            self.placement_constraints,
            self.entry_set,
            self.entries,
            self.relocation_set,
            self.relocations,
            self.authority_commitments,
        )
        .expect("mutated artifact must remain canonically constructible")
    }
}

fn section_record(index: usize) -> usize {
    OMEGA_EXECUTABLE_CONTAINER_HEADER_BYTES as usize
        + index * OMEGA_EXECUTABLE_CONTAINER_SECTION_RECORD_BYTES as usize
}

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

#[test]
fn executable_container_wire_rejects_every_one_field_substitution() {
    let artifact = strong_artifact();
    let proof = vec![0xa5; 64];
    let encoded = encode_executable_container(&artifact, &proof, limits()).expect("encode v2");
    let authentic = decode_executable_container(&encoded, limits()).expect("authentic v2");

    // Every representable artifact field: substitute once, let
    // `Artifact::from_canonical_decode` honestly recompute the containing
    // content digest and container fingerprint, canonically encode and decode
    // the container, then confirm independent admission replay against the
    // original evidence still rejects the substitution.
    let mutated = |mutate: &mut dyn FnMut(&mut ArtifactSeed)| -> Artifact {
        let mut seed = ArtifactSeed::from(&artifact);
        mutate(&mut seed);
        let mutated = seed.into_artifact();
        assert_ne!(
            mutated, artifact,
            "the substitution must change the artifact"
        );
        let mutated_bytes = encode_executable_container(&mutated, &proof, limits())
            .expect("mutated artifact canonically encodes");
        assert_ne!(mutated_bytes, encoded, "the wire bytes must change");
        let substituted = decode_executable_container(&mutated_bytes, limits())
            .expect("mutated container canonically decodes");
        assert_eq!(
            substituted.artifact(),
            &mutated,
            "canonical decode must preserve the mutated artifact",
        );
        let evidence = ValidatedContainerAdmissionEvidence::from_validator(
            AdmissionReceiptId::from_normalized_identity(70).unwrap(),
            &authentic,
            true,
        );
        let error = admit_validated_container(&substituted, evidence)
            .expect_err("independent replay must reject the substitution");
        assert!(
            error.0.contains("different validated container"),
            "unexpected replay rejection: {}",
            error.0
        );
        mutated
    };

    // The claimed normalized identity rides outside the content digest and is
    // bound only by the admission evidence.
    let substituted = mutated(&mut |seed| {
        seed.identity = ArtifactId::from_normalized_identity(2).expect("artifact identity");
    });
    assert_eq!(
        substituted.content(),
        artifact.content(),
        "the claimed artifact identity rides outside the content digest",
    );

    mutated(&mut |seed| {
        seed.architecture = Architecture::Aarch64;
        seed.relocations[0].kind = ArtifactRelocationKind::Absolute64;
    });
    mutated(&mut |seed| seed.code[0] ^= 1);
    mutated(&mut |seed| {
        seed.code.truncate(32);
        seed.relocations[0].destination_offset = 8;
    });
    mutated(&mut |seed| {
        seed.contracts = MachineContractSetId::from_normalized_identity(13).expect("contracts");
        seed.recommit();
    });
    mutated(&mut |seed| {
        seed.declared_footprint =
            MachineFootprintId::from_normalized_identity(14).expect("footprint");
        seed.recommit();
    });
    mutated(&mut |seed| {
        seed.placement_plan = PlacementPlanId::from_normalized_identity(15).expect("plan");
    });
    mutated(&mut |seed| {
        seed.placement_constraints = PlacementConstraints::new(
            seed.placement_constraints.permitted_range(),
            8,
            seed.placement_constraints.phase(),
            seed.placement_constraints.machine_regime(),
            seed.placement_constraints.installation_scope(),
        )
        .expect("alignment");
    });
    mutated(&mut |seed| {
        seed.placement_constraints = PlacementConstraints::new(
            seed.placement_constraints.permitted_range(),
            seed.placement_constraints.alignment(),
            PlacementPhase::PostHandoff,
            seed.placement_constraints.machine_regime(),
            seed.placement_constraints.installation_scope(),
        )
        .expect("phase");
    });
    mutated(&mut |seed| {
        seed.placement_constraints = PlacementConstraints::new(
            Some(
                layout_plans::PlacementAddressRange::new(0x1000, 0x2000).expect("permitted range"),
            ),
            seed.placement_constraints.alignment(),
            seed.placement_constraints.phase(),
            seed.placement_constraints.machine_regime(),
            seed.placement_constraints.installation_scope(),
        )
        .expect("range");
    });
    mutated(&mut |seed| {
        seed.placement_constraints = PlacementConstraints::new(
            seed.placement_constraints.permitted_range(),
            seed.placement_constraints.alignment(),
            seed.placement_constraints.phase(),
            Some(MachineRegimeId::from_normalized_identity(12).expect("regime")),
            seed.placement_constraints.installation_scope(),
        )
        .expect("regime");
        seed.recommit();
    });
    mutated(&mut |seed| {
        seed.placement_constraints = PlacementConstraints::new(
            seed.placement_constraints.permitted_range(),
            seed.placement_constraints.alignment(),
            seed.placement_constraints.phase(),
            seed.placement_constraints.machine_regime(),
            Some(ArtifactInstallationScopeId::from_normalized_identity(13).expect("scope")),
        )
        .expect("scope");
        seed.recommit();
    });
    mutated(&mut |seed| {
        seed.placement_constraints = PlacementConstraints::new(
            seed.placement_constraints.permitted_range(),
            seed.placement_constraints.alignment(),
            seed.placement_constraints.phase(),
            None,
            seed.placement_constraints.installation_scope(),
        )
        .expect("dropped regime");
        seed.recommit();
    });
    mutated(&mut |seed| {
        seed.entry_set = EntrySetId::from_normalized_identity(18).expect("entry set");
    });
    mutated(&mut |seed| {
        seed.entries[0] = ArtifactEntry::from_canonical_decode(
            EntryStubId::from_normalized_identity(10).expect("entry"),
            seed.entries[0].code_offset(),
        );
    });
    mutated(&mut |seed| {
        seed.entries[0] = ArtifactEntry::from_canonical_decode(seed.entries[0].identity(), 24);
    });
    mutated(&mut |seed| {
        seed.entries.push(ArtifactEntry::from_canonical_decode(
            EntryStubId::from_normalized_identity(10).expect("entry"),
            24,
        ));
    });
    mutated(&mut |seed| {
        seed.relocation_set = RelocationSetId::from_normalized_identity(16).expect("set");
    });
    mutated(&mut |seed| {
        seed.relocations[0].kind = ArtifactRelocationKind::Absolute64;
    });
    mutated(&mut |seed| {
        seed.relocations[0].destination_offset = 40;
    });
    mutated(&mut |seed| {
        seed.relocations[0].target = RelocationTarget::Data(
            layout_plans::DataSymbolId::from_normalized_identity(12).expect("data symbol"),
        );
    });
    mutated(&mut |seed| seed.relocations[0].addend = 7);
    mutated(&mut |seed| seed.relocations.clear());
    mutated(&mut |seed| {
        seed.authority_commitments = ArtifactAuthorityCommitments::from_canonical_evidence(
            seed.contracts,
            b"forged imported contract set",
            seed.declared_footprint,
            b"canonical declared footprint",
            seed.placement_constraints
                .machine_regime()
                .map(|regime| (regime, b"canonical machine regime".as_slice())),
            seed.placement_constraints
                .installation_scope()
                .map(|scope| (scope, b"canonical installation scope".as_slice())),
        );
    });

    // Substitutions the canonical artifact construction or encoder cannot
    // carry at all reject before any container exists.
    let rejects_construction = |mutate: &mut dyn FnMut(&mut ArtifactSeed), needle: &str| {
        let mut seed = ArtifactSeed::from(&artifact);
        mutate(&mut seed);
        let error = Artifact::from_canonical_decode(
            seed.identity,
            seed.architecture,
            seed.code,
            seed.contracts,
            seed.declared_footprint,
            seed.placement_plan,
            seed.placement_constraints,
            seed.entry_set,
            seed.entries,
            seed.relocation_set,
            seed.relocations,
            seed.authority_commitments,
        )
        .expect_err("non-representable substitution must reject construction");
        assert!(
            error.0.contains(needle),
            "unexpected construction rejection: {}",
            error.0
        );
    };
    rejects_construction(&mut |seed| seed.code.clear(), "cannot have empty content");
    rejects_construction(
        &mut |seed| seed.entries.clear(),
        "at least one selected entry",
    );
    rejects_construction(
        &mut |seed| seed.entries.push(seed.entries[0]),
        "must be unique",
    );
    rejects_construction(
        &mut |seed| {
            seed.entries[0] = ArtifactEntry::from_canonical_decode(seed.entries[0].identity(), 64)
        },
        "lies outside",
    );
    rejects_construction(
        &mut |seed| {
            seed.contracts = MachineContractSetId::from_normalized_identity(13).expect("id");
        },
        "compact report coordinates",
    );
    rejects_construction(
        &mut |seed| {
            seed.declared_footprint = MachineFootprintId::from_normalized_identity(14).expect("id");
        },
        "compact report coordinates",
    );
    rejects_construction(
        &mut |seed| {
            seed.placement_constraints = PlacementConstraints::new(
                seed.placement_constraints.permitted_range(),
                seed.placement_constraints.alignment(),
                seed.placement_constraints.phase(),
                None,
                seed.placement_constraints.installation_scope(),
            )
            .expect("dropped regime");
        },
        "compact report coordinates",
    );

    let mut too_many = ArtifactSeed::from(&artifact);
    too_many.code = vec![0x90; 128];
    too_many.relocations = (0..17)
        .map(|index| DecodedArtifactRelocation {
            kind: ArtifactRelocationKind::X86Relative32,
            destination_offset: index * 4,
            target: RelocationTarget::Entry(seed_entry(&artifact)),
            addend: 0,
        })
        .collect();
    let too_many = too_many.into_artifact();
    let error = encode_executable_container(&too_many, &proof, limits())
        .expect_err("the relocation bound must reject at encoding");
    assert!(error.0.contains("exceeding configured bound"));

    let mut oversized = ArtifactSeed::from(&artifact);
    oversized.code = vec![0x90; 2048];
    oversized.entries[0] =
        ArtifactEntry::from_canonical_decode(oversized.entries[0].identity(), 2000);
    oversized.relocations[0].destination_offset = 1992;
    let oversized = oversized.into_artifact();
    let error = encode_executable_container(&oversized, &proof, limits())
        .expect_err("the section bound must reject at encoding");
    assert!(error.0.contains("is empty or exceeds configured bound"));

    let error = encode_executable_container(&artifact, &[], limits())
        .expect_err("an empty proof section must reject at encoding");
    assert!(error.0.contains("proof section cannot be empty"));
    let error = encode_executable_container_v1_compatibility(&artifact, &proof, limits())
        .expect_err("v1 encoding cannot discard strong commitments");
    assert!(
        error
            .0
            .contains("cannot discard strong authority commitments")
    );

    // Every wire field the canonical encoder derives must reject a raw
    // substitution at decode: the codec is hostile-input, so no byte axis may
    // be carried silently.
    let rejects_decode = |mutated: &[u8], needle: &str| {
        let error = decode_executable_container(mutated, limits())
            .expect_err("wire substitution must reject");
        assert!(
            error.0.contains(needle),
            "unexpected decode rejection for `{needle}`: {}",
            error.0
        );
    };

    // Header fields.
    let mut changed = encoded.clone();
    changed[0] ^= 1;
    rejects_decode(&changed, "invalid magic");

    let mut changed = encoded.clone();
    write_u16(&mut changed, 8, 0x9999);
    rejects_decode(&changed, "unsupported Omega executable container marker");

    let mut changed = encoded.clone();
    write_u16(&mut changed, 10, 32);
    rejects_decode(&changed, "is not canonical 64");

    let mut changed = encoded.clone();
    changed[12] = 9;
    rejects_decode(&changed, "unknown executable-container architecture 9");

    let mut changed = encoded.clone();
    changed[13] = 1;
    rejects_decode(&changed, "reserved0");

    let mut changed = encoded.clone();
    write_u16(&mut changed, 14, 0);
    rejects_decode(&changed, "has 0 sections");

    let mut changed = encoded.clone();
    write_u16(&mut changed, 14, 17);
    rejects_decode(&changed, "has 17 sections");

    let mut changed = encoded.clone();
    write_u64(&mut changed, 16, 128);
    rejects_decode(&changed, "is not canonical");

    let mut changed = encoded.clone();
    write_u64(&mut changed, 24, encoded.len() as u64 + 1);
    rejects_decode(&changed, "declares");

    let mut changed = encoded.clone();
    write_u64(&mut changed, 32, 0);
    rejects_decode(&changed, "normalized artifact identity cannot be zero");

    let mut changed = encoded.clone();
    write_u64(&mut changed, 40, 0);
    rejects_decode(&changed, "fingerprint cannot be zero");

    let mut changed = encoded.clone();
    write_u64(&mut changed, 48, 1);
    rejects_decode(&changed, "reserved1");

    let mut changed = encoded.clone();
    write_u64(&mut changed, 56, 1);
    rejects_decode(&changed, "reserved2");

    // Section-record fields: flags, reserved, and the per-kind identity rule.
    let code_record = section_record(0);
    let mut changed = encoded.clone();
    write_u16(&mut changed, code_record + 2, 0);
    rejects_decode(&changed, "must be required");

    let mut changed = encoded.clone();
    write_u16(&mut changed, code_record + 2, 3);
    rejects_decode(&changed, "unknown flags");

    let mut changed = encoded.clone();
    write_u32(&mut changed, code_record + 4, 1);
    rejects_decode(&changed, "must be zero");

    let mut changed = encoded.clone();
    write_u64(&mut changed, code_record + 8, 1);
    rejects_decode(&changed, "must use zero wire identity");

    let relocation_record = section_record(1);
    let mut changed = encoded.clone();
    write_u64(&mut changed, relocation_record + 8, 0);
    rejects_decode(&changed, "requires a nonzero normalized identity");

    let mut changed = encoded.clone();
    write_u64(&mut changed, relocation_record + 8, 99);
    rejects_decode(&changed, "content fingerprint");

    let contracts_record = section_record(2);
    let mut changed = encoded.clone();
    write_u64(&mut changed, contracts_record + 8, 99);
    rejects_decode(&changed, "payload identity");

    let placement_record = section_record(4);
    let mut changed = encoded.clone();
    write_u64(&mut changed, placement_record + 8, 99);
    rejects_decode(&changed, "payload identity");

    let proof_record = section_record(6);
    let mut changed = encoded.clone();
    write_u64(&mut changed, proof_record + 8, 1);
    rejects_decode(&changed, "must use zero wire identity");

    let authority_record = section_record(7);
    let mut changed = encoded.clone();
    write_u16(&mut changed, authority_record + 2, 0);
    rejects_decode(
        &changed,
        "container-v2 authority commitments must be required",
    );

    let mut changed = encoded.clone();
    write_u64(&mut changed, authority_record + 8, 1);
    rejects_decode(&changed, "must use zero wire identity");

    // A v2 container remarkered as v1 still cannot smuggle its authority
    // section through the compatibility reader.
    let mut changed = encoded.clone();
    write_u16(&mut changed, 8, OMEGA_EXECUTABLE_CONTAINER_V1_MARKER);
    rejects_decode(
        &changed,
        "container-v1 cannot carry a v2 authority-commitment section",
    );

    // A code-payload byte substitution decodes into different semantics, so
    // the canonical content-fingerprint join rejects it before artifact
    // construction ever runs.
    let mut changed = encoded.clone();
    changed[section_offset(&encoded, 0)] ^= 1;
    rejects_decode(&changed, "content fingerprint");

    // Placement-record fields.
    let placement_offset = section_offset(&encoded, 4);
    let mut changed = encoded.clone();
    write_u64(&mut changed, placement_offset, 99);
    rejects_decode(&changed, "payload identity");

    let mut changed = encoded.clone();
    changed[placement_offset + 8] = 2;
    rejects_decode(&changed, "placement range presence flag 2 is not boolean");

    let mut changed = encoded.clone();
    changed[placement_offset + 9] = 9;
    rejects_decode(&changed, "unknown placement phase 9");

    let mut changed = encoded.clone();
    changed[placement_offset + 10] = 0;
    rejects_decode(&changed, "machine regime identity must be zero when absent");

    let mut changed = encoded.clone();
    changed[placement_offset + 10] = 2;
    rejects_decode(&changed, "machine regime presence flag 2 is not boolean");

    let mut changed = encoded.clone();
    write_u64(&mut changed, placement_offset + 40, 0);
    rejects_decode(
        &changed,
        "machine regime identity cannot be zero when present",
    );

    let mut changed = encoded.clone();
    changed[placement_offset + 11] = 0;
    rejects_decode(
        &changed,
        "installation scope identity must be zero when absent",
    );

    let mut changed = encoded.clone();
    changed[placement_offset + 11] = 2;
    rejects_decode(
        &changed,
        "installation scope presence flag 2 is not boolean",
    );

    let mut changed = encoded.clone();
    write_u64(&mut changed, placement_offset + 48, 0);
    rejects_decode(
        &changed,
        "installation scope identity cannot be zero when present",
    );

    let mut changed = encoded.clone();
    write_u64(&mut changed, placement_offset + 16, 1);
    rejects_decode(&changed, "placement range values must be zero when absent");

    let mut changed = encoded.clone();
    changed[placement_offset + 8] = 1;
    write_u64(&mut changed, placement_offset + 16, 0x2000);
    write_u64(&mut changed, placement_offset + 24, 0x1000);
    rejects_decode(&changed, "placement range is invalid");

    let mut changed = encoded.clone();
    write_u64(&mut changed, placement_offset + 32, 0);
    rejects_decode(&changed, "placement alignment must be nonzero");

    let mut changed = encoded.clone();
    write_u64(&mut changed, placement_offset + 32, 8);
    rejects_decode(&changed, "content fingerprint");

    let mut changed = encoded.clone();
    write_u32(&mut changed, placement_offset + 12, 1);
    rejects_decode(&changed, "placement reserved field must be zero");

    let mut changed = encoded.clone();
    write_u64(&mut changed, placement_offset + 56, 1);
    rejects_decode(&changed, "placement reserved field must be zero");

    // Relocation count and record fields.
    let relocation_offset = section_offset(&encoded, 1);
    let mut changed = encoded.clone();
    write_u64(&mut changed, relocation_offset, 0);
    rejects_decode(&changed, "does not match 0 canonical records");

    let mut changed = encoded.clone();
    write_u64(&mut changed, relocation_offset, 17);
    rejects_decode(&changed, "exceeding configured bound");

    let relocation_payload = relocation_offset + RELOCATION_COUNT_BYTES as usize;
    let mut changed = encoded.clone();
    write_u16(&mut changed, relocation_payload, 9);
    rejects_decode(&changed, "unknown artifact relocation kind 9");

    let mut changed = encoded.clone();
    write_u16(&mut changed, relocation_payload + 2, 9);
    rejects_decode(&changed, "unknown artifact relocation target kind 9");

    let mut changed = encoded.clone();
    write_u32(&mut changed, relocation_payload + 4, 1);
    rejects_decode(&changed, "relocation reserved");

    let mut changed = encoded.clone();
    write_u64(&mut changed, relocation_payload + 16, 0);
    rejects_decode(&changed, "normalized entry stub identity cannot be zero");

    let mut changed = encoded.clone();
    write_u64(&mut changed, relocation_payload + 24, 1);
    rejects_decode(&changed, "content fingerprint");

    // Entry-record fields: a raw payload substitution decodes into different
    // semantics, so the canonical content-fingerprint join rejects it before
    // artifact construction ever runs.
    let entry_offset = section_offset(&encoded, 5);
    let mut changed = encoded.clone();
    write_u64(&mut changed, entry_offset, 0);
    rejects_decode(&changed, "normalized entry stub identity cannot be zero");

    let mut changed = encoded.clone();
    write_u64(&mut changed, entry_offset + 8, 24);
    rejects_decode(&changed, "content fingerprint");

    // The authority-commitment section binds four strong digests: a zero
    // digest is not representable, and a short section cannot keep canonical
    // tiling without also retiring the declared length.
    let authority_offset = section_offset(&encoded, 7);
    let mut changed = encoded.clone();
    changed[authority_offset..authority_offset + 32].fill(0);
    rejects_decode(&changed, "authority commitments cannot be zero");

    let mut changed = encoded.clone();
    changed.truncate(authority_offset + 64);
    write_u64(&mut changed, authority_record + 24, 64);
    let total_length = changed.len() as u64;
    write_u64(&mut changed, 24, total_length);
    rejects_decode(&changed, "must contain exactly 128 bytes");
}

fn seed_entry(artifact: &Artifact) -> EntryStubId {
    artifact.0.entries[0].identity()
}
