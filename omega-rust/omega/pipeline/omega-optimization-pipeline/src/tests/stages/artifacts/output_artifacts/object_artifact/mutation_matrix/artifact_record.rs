//! Reauthenticated object-artifact record fields and canonical wire axes.

use crate::tests::*;
use omega_optimization_core::{
    FunctionFragmentEmissionManifestIdentity, FunctionFragmentObjectContainerManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
};
use omega_target::{Architecture, ObjectFormat};
use psi_terminal::SemanticFingerprint;

use super::fixture::staged_object_artifact;

type RecordMutation = fn(&mut OptimizedObjectArtifactRecord);

#[test]
fn every_representable_artifact_record_field_rejects_after_reauthentication() {
    let mut staged = staged_object_artifact();
    let baseline = staged.artifact().clone();
    // Vocabulary is singleton in memory; its closed marker is covered below.
    let mutations: [(&str, RecordMutation); 23] = [
        ("psi_artifact", |record| record.psi_artifact = [0xb1; 32]),
        ("psi.program_fingerprint", |record| {
            record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xb2; 32])
        }),
        ("obligation_ledger", |record| {
            record.obligation_ledger = [0xb3; 32]
        }),
        ("proof_bundle", |record| record.proof_bundle = [0xb4; 32]),
        ("debug_section", |record| {
            record.debug_section = match record.debug_section {
                None => Some([0xb5; 32]),
                Some(_) => None,
            }
        }),
        ("selections", |record| {
            record.selections = OptimizationSelectionIdentity::from_bytes([0xb6; 32])
        }),
        ("target.architecture", |record| {
            record.target.architecture = Architecture::X86_64
        }),
        ("target.object_format", |record| {
            record.target.object_format = ObjectFormat::MachO
        }),
        ("target.pointer_size", |record| {
            record.target.pointer_size += 1
        }),
        ("target.pointer_alignment", |record| {
            record.target.pointer_alignment += 1
        }),
        ("semantic_entry", |record| {
            record.semantic_entry = MachineId::new(99_911).unwrap()
        }),
        ("pre_physical_manifest", |record| {
            record.pre_physical_manifest =
                PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"other prephysical")
        }),
        ("post_allocation_manifest", |record| {
            record.post_allocation_manifest =
                PostAllocationOptimizationManifestIdentity::from_canonical_bytes(
                    b"other post-allocation",
                )
        }),
        ("function_relative_manifest", |record| {
            record.function_relative_manifest =
                FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
                    b"other function-relative",
                )
        }),
        ("function_fragment_manifest", |record| {
            record.function_fragment_manifest =
                FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(
                    b"other function fragment",
                )
        }),
        ("text_section_manifest", |record| {
            record.text_section_manifest =
                FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(
                    b"other text section",
                )
        }),
        ("object_container_manifest", |record| {
            record.object_container_manifest =
                FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(
                    b"other object manifest",
                )
        }),
        ("object", |record| {
            record.object = RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"other object")
        }),
        ("object_container", |record| {
            record.object_container =
                RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"other container")
        }),
        ("statistics.text_bytes", |record| {
            record.statistics.text_bytes += 1
        }),
        ("statistics.object_container_bytes", |record| {
            record.statistics.object_container_bytes += 1
        }),
        ("statistics.function_symbols", |record| {
            record.statistics.function_symbols += 1
        }),
        ("statistics.relocation_records", |record| {
            record.statistics.relocation_records += 1
        }),
    ];

    for (field, mutate) in mutations {
        *staged.artifact_mut() = baseline.clone();
        let record = staged.artifact_mut();
        mutate(record);
        record.identity = record.recomputed_identity();
        assert_eq!(
            validate_optimized_object_artifact(&staged),
            Err(OptimizedObjectArtifactError::ArtifactMismatch),
            "reauthenticated {field} mutation must fail independent replay",
        );
    }

    *staged.artifact_mut() = baseline;
    staged.artifact_mut().identity =
        omega_optimization_core::OptimizedObjectArtifactIdentity::from_bytes([0xb7; 32]);
    assert_eq!(
        validate_optimized_object_artifact(&staged),
        Err(OptimizedObjectArtifactError::ArtifactMismatch),
    );
}

#[test]
fn artifact_record_wire_rejects_every_closed_axis_and_envelope_mutation() {
    let staged = staged_object_artifact();
    let encoded = staged.artifact().encode();
    assert_eq!(
        encoded.len(),
        521,
        "debug-free object artifact V1 is pinned"
    );

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&wrong_magic),
        Err(OptimizedObjectArtifactRecordDecodeError::WrongMagic),
    );

    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&99_u32.to_le_bytes());
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&wrong_version),
        Err(OptimizedObjectArtifactRecordDecodeError::UnsupportedVersion(99)),
    );

    let mut wrong_identity = encoded.clone();
    wrong_identity[12] ^= 1;
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&wrong_identity),
        Err(OptimizedObjectArtifactRecordDecodeError::IdentityMismatch),
    );

    let mut unknown_vocabulary = encoded.clone();
    unknown_vocabulary[76..78].copy_from_slice(&59_u16.to_le_bytes());
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&unknown_vocabulary),
        Err(OptimizedObjectArtifactRecordDecodeError::UnknownVocabulary(
            59
        )),
    );

    let mut unknown_debug_tag = encoded.clone();
    unknown_debug_tag[174] = 99;
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&unknown_debug_tag),
        Err(OptimizedObjectArtifactRecordDecodeError::UnknownOptionalTag(99)),
    );

    let mut unknown_architecture = encoded.clone();
    unknown_architecture[207] = 99;
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&unknown_architecture),
        Err(OptimizedObjectArtifactRecordDecodeError::UnknownArchitecture(99)),
    );

    let mut unknown_object_format = encoded.clone();
    unknown_object_format[208] = 99;
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&unknown_object_format),
        Err(OptimizedObjectArtifactRecordDecodeError::UnknownObjectFormat(99)),
    );

    let mut invalid_machine = encoded.clone();
    invalid_machine[225..233].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&invalid_machine),
        Err(OptimizedObjectArtifactRecordDecodeError::InvalidMachine),
    );

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&trailing),
        Err(OptimizedObjectArtifactRecordDecodeError::TrailingBytes),
    );
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&encoded[..encoded.len() - 1]),
        Err(OptimizedObjectArtifactRecordDecodeError::Truncated),
    );
}
