//! Reauthenticated logical-field mutations against canonical manifest replay.

use super::super::*;
use super::fixture::manifest_fixture;
use crate::optimized_semantic_wrapper_object::object::validate_manifest;
use omega_target::{Architecture, ObjectFormat};

type ManifestMutation = fn(&mut OptimizedProgramStorageSemanticWrapperObjectManifest);

#[test]
fn every_representable_manifest_field_rejects_after_reauthentication() {
    let (object, container, baseline) = manifest_fixture();
    let mutations: [(&str, ManifestMutation); 12] = [
        ("object", |record| {
            record.object =
                OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(
                    b"mutated-wrapper-object",
                )
        }),
        ("container", |record| {
            record.container =
                OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_canonical_bytes(
                    b"mutated-wrapper-container",
                )
        }),
        ("source_artifact", |record| {
            record.source_artifact =
                OptimizedObjectArtifactIdentity::from_canonical_bytes(b"mutated-artifact")
        }),
        ("source_artifact_manifest", |record| {
            record.source_artifact_manifest =
                OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(
                    b"mutated-artifact-manifest",
                )
        }),
        ("source_object", |record| {
            record.source_object =
                RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"mutated-source-object")
        }),
        ("source_object_container", |record| {
            record.source_object_container =
                RelocationFreeObjectContainerIdentity::from_canonical_bytes(
                    b"mutated-source-container",
                )
        }),
        ("source_signature", |record| {
            record.source_signature = [0xa7; 32]
        }),
        ("psi.program_fingerprint", |record| {
            record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xa9; 32])
        }),
        ("wrapper_symbol", |record| {
            record.wrapper_symbol = ObjectLocalSymbolId::new(19).unwrap()
        }),
        ("continuation_symbol", |record| {
            record.continuation_symbol = ObjectLocalSymbolId::new(20).unwrap()
        }),
        ("text_byte_count", |record| record.text_byte_count += 1),
        ("symbol_count", |record| record.symbol_count += 1),
    ];

    for (field, mutate) in mutations {
        let mut record = baseline.clone();
        mutate(&mut record);
        record.identity = record.recomputed_identity();
        assert_eq!(
            OptimizedProgramStorageSemanticWrapperObjectManifest::decode(&record.encode()),
            Ok(record.clone()),
            "reauthenticated {field} must retain a valid manifest envelope",
        );
        assert_eq!(
            validate_manifest(&object, &container, &record),
            Err(OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch),
            "canonical producer replay must reject reauthenticated {field}",
        );
    }
}

#[test]
fn every_representable_closed_shape_field_fails_before_authority() {
    let (object, container, baseline) = manifest_fixture();
    let mutations: [(&str, ManifestMutation); 5] = [
        ("target.architecture", |record| {
            record.target.architecture = Architecture::Aarch64
        }),
        ("target.object_format", |record| {
            record.target.object_format = ObjectFormat::Elf
        }),
        ("target.pointer_size", |record| {
            record.target.pointer_size = 4
        }),
        ("target.pointer_alignment", |record| {
            record.target.pointer_alignment = 4
        }),
        ("relocation_record_count", |record| {
            record.relocation_record_count = 1
        }),
    ];

    for (field, mutate) in mutations {
        let mut record = baseline.clone();
        mutate(&mut record);
        record.identity = record.recomputed_identity();
        assert_ne!(
            OptimizedProgramStorageSemanticWrapperObjectManifest::decode(&record.encode()),
            Ok(record.clone()),
            "closed manifest shape must not recreate reauthenticated {field}",
        );
        assert_eq!(
            validate_manifest(&object, &container, &record),
            Err(OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch),
        );
    }
}

#[test]
fn stale_manifest_identity_rejects_before_authority() {
    let (object, container, mut manifest) = manifest_fixture();
    manifest.identity =
        OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_canonical_bytes(
            b"stale-wrapper-manifest",
        );
    assert_eq!(
        validate_manifest(&object, &container, &manifest),
        Err(OptimizedProgramStorageSemanticWrapperObjectError::ManifestMismatch),
    );
}
