//! Reauthenticated object-artifact manifest fields and canonical wire axes.

use crate::tests::*;
use omega_optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, OptimizationSelectionIdentity,
    OptimizedObjectArtifactIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity,
};
use omega_target::{Architecture, ObjectFormat};
use psi_terminal::SemanticFingerprint;

use super::fixture::staged_object_artifact;

type ManifestMutation = fn(&mut OptimizedObjectArtifactManifest);

#[test]
fn every_representable_artifact_manifest_field_rejects_after_reauthentication() {
    let mut staged = staged_object_artifact();
    let baseline = staged.manifest().record().clone();
    // Stage, vocabulary, and unavailable-data values are singleton in memory;
    // their closed tags are covered by the wire matrix below.
    let mutations: [(&str, ManifestMutation); 16] = [
        ("artifact", |record| {
            record.artifact =
                OptimizedObjectArtifactIdentity::from_canonical_bytes(b"other artifact")
        }),
        ("psi_artifact", |record| record.psi_artifact = [0xc1; 32]),
        ("psi.program_fingerprint", |record| {
            record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xc2; 32])
        }),
        ("selections", |record| {
            record.selections = OptimizationSelectionIdentity::from_bytes([0xc3; 32])
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
            record.semantic_entry = MachineId::new(99_921).unwrap()
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
        *staged.manifest_mut().record_mut() = baseline.clone();
        let record = staged.manifest_mut().record_mut();
        mutate(record);
        record.identity = record.recomputed_identity();
        assert_eq!(
            validate_optimized_object_artifact(&staged),
            Err(OptimizedObjectArtifactError::ManifestMismatch),
            "reauthenticated {field} mutation must fail independent replay",
        );
    }

    *staged.manifest_mut().record_mut() = baseline;
    staged.manifest_mut().record_mut().identity =
        omega_optimization_core::OptimizedObjectArtifactManifestIdentity::from_bytes([0xc4; 32]);
    assert_eq!(
        validate_optimized_object_artifact(&staged),
        Err(OptimizedObjectArtifactError::ManifestMismatch),
    );
}

#[test]
fn artifact_manifest_wire_rejects_every_closed_axis_and_envelope_mutation() {
    let staged = staged_object_artifact();
    let encoded = staged.manifest().record().encode();
    assert_eq!(encoded.len(), 333, "object artifact manifest V1 is pinned");

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&wrong_magic),
        Err(OptimizedObjectArtifactManifestDecodeError::WrongMagic),
    );

    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&99_u32.to_le_bytes());
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&wrong_version),
        Err(OptimizedObjectArtifactManifestDecodeError::UnsupportedVersion(99)),
    );

    let mut wrong_identity = encoded.clone();
    wrong_identity[12] ^= 1;
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&wrong_identity),
        Err(OptimizedObjectArtifactManifestDecodeError::IdentityMismatch),
    );

    let mut unknown_stage = encoded.clone();
    unknown_stage[44] = 99;
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&unknown_stage),
        Err(OptimizedObjectArtifactManifestDecodeError::UnknownStage(99)),
    );

    let mut unknown_vocabulary = encoded.clone();
    unknown_vocabulary[109..111].copy_from_slice(&59_u16.to_le_bytes());
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&unknown_vocabulary),
        Err(OptimizedObjectArtifactManifestDecodeError::Artifact(
            OptimizedObjectArtifactRecordDecodeError::UnknownVocabulary(59),
        )),
    );

    let mut unknown_architecture = encoded.clone();
    unknown_architecture[175] = 99;
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&unknown_architecture),
        Err(OptimizedObjectArtifactManifestDecodeError::Artifact(
            OptimizedObjectArtifactRecordDecodeError::UnknownArchitecture(99),
        )),
    );

    let mut unknown_object_format = encoded.clone();
    unknown_object_format[176] = 99;
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&unknown_object_format),
        Err(OptimizedObjectArtifactManifestDecodeError::Artifact(
            OptimizedObjectArtifactRecordDecodeError::UnknownObjectFormat(99),
        )),
    );

    let mut invalid_machine = encoded.clone();
    invalid_machine[193..201].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&invalid_machine),
        Err(OptimizedObjectArtifactManifestDecodeError::Artifact(
            OptimizedObjectArtifactRecordDecodeError::InvalidMachine,
        )),
    );

    for offset in 329..333 {
        let mut unknown_unavailable = encoded.clone();
        unknown_unavailable[offset] = 99;
        assert_eq!(
            OptimizedObjectArtifactManifest::decode(&unknown_unavailable),
            Err(OptimizedObjectArtifactManifestDecodeError::UnknownUnavailableStatus),
            "unavailable field at wire offset {offset} must fail closed",
        );
    }

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&trailing),
        Err(OptimizedObjectArtifactManifestDecodeError::TrailingBytes),
    );
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&encoded[..encoded.len() - 1]),
        Err(OptimizedObjectArtifactManifestDecodeError::Artifact(
            OptimizedObjectArtifactRecordDecodeError::Truncated,
        )),
    );
}
