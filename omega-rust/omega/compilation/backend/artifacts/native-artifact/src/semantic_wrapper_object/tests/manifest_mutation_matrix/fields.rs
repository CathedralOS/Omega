//! Reauthenticated logical-field mutations against canonical manifest replay.
//!
//! The representable legs run through the shared
//! `run_one_field_substitution_matrix` driver over the declared
//! `ManifestFieldForTest` inventory: each leg substitutes one manifest field,
//! recomputes the manifest identity honestly, must keep a valid canonical
//! envelope, and is rejected by `validate_manifest` — the producer replay
//! the owning stage runs — as `ManifestMismatch`. The closed-shape fields,
//! which the manifest codec refuses to recreate, and the stale-identity case
//! stay authored below.
use super::super::{
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectRecordError,
};
use super::fixture::{foreign_manifest, manifest_fixture};
use crate::semantic_wrapper_object::manifest::validate_manifest;
use object_file::ObjectLocalSymbolId;
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, OptimizedObjectArtifactIdentity,
    OptimizedObjectArtifactManifestIdentity,
    OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity,
    OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
    run_one_field_substitution_matrix,
};
use target::{Architecture, ObjectFormat};
use terminal_psi::SemanticFingerprint;

type ManifestMutation = fn(&mut OptimizedProgramStorageSemanticWrapperObjectManifest);

optimization_core::custody_field_inventory! {
    /// One independently representable field of the wrapper object manifest;
    /// checked by `validate_manifest`, which re-derives the manifest from the
    /// plan/container pair and compares the retained record.
    pub(crate) enum ManifestFieldForTest {
        Object,
        Container,
        SourceArtifact,
        SourceArtifactManifest,
        SourceObject,
        SourceObjectContainer,
        SourceSignature,
        PsiProgramFingerprint,
        WrapperSymbol,
        ContinuationSymbol,
        TextByteCount,
        SymbolCount,
    }
}

/// Substitute exactly one field with a fixed non-canonical alternate and
/// re-authenticate the manifest identity, so only producer replay can
/// expose the substitution.
fn substitute_manifest_for_test(
    record: &mut OptimizedProgramStorageSemanticWrapperObjectManifest,
    field: ManifestFieldForTest,
    _donor: &OptimizedProgramStorageSemanticWrapperObjectManifest,
) {
    match field {
        ManifestFieldForTest::Object => {
            record.object =
                OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(
                    b"mutated-wrapper-object",
                )
        }
        ManifestFieldForTest::Container => {
            record.container =
                OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_canonical_bytes(
                    b"mutated-wrapper-container",
                )
        }
        ManifestFieldForTest::SourceArtifact => {
            record.source_artifact =
                OptimizedObjectArtifactIdentity::from_canonical_bytes(b"mutated-artifact")
        }
        ManifestFieldForTest::SourceArtifactManifest => {
            record.source_artifact_manifest =
                OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(
                    b"mutated-artifact-manifest",
                )
        }
        ManifestFieldForTest::SourceObject => {
            record.source_object =
                RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"mutated-source-object")
        }
        ManifestFieldForTest::SourceObjectContainer => {
            record.source_object_container =
                RelocationFreeObjectContainerIdentity::from_canonical_bytes(
                    b"mutated-source-container",
                )
        }
        ManifestFieldForTest::SourceSignature => record.source_signature = [0xa7; 32],
        ManifestFieldForTest::PsiProgramFingerprint => {
            record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xa9; 32])
        }
        ManifestFieldForTest::WrapperSymbol => {
            record.wrapper_symbol = ObjectLocalSymbolId::new(19).unwrap()
        }
        ManifestFieldForTest::ContinuationSymbol => {
            record.continuation_symbol = ObjectLocalSymbolId::new(20).unwrap()
        }
        ManifestFieldForTest::TextByteCount => record.text_byte_count += 1,
        ManifestFieldForTest::SymbolCount => record.symbol_count += 1,
    }
    record.identity = record.recomputed_identity();
}

#[test]
fn every_representable_manifest_field_rejects_after_reauthentication() {
    let (object, container, _) = manifest_fixture();
    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "wrapper object manifest",
        fields: ManifestFieldForTest::INVENTORY,
        honest: &|| manifest_fixture().2,
        donor: foreign_manifest(),
        // The retained identity: honest re-authentication must move it.
        custody: &|record| record.identity,
        substitute: &substitute_manifest_for_test,
        check: &|record| validate_manifest(&object, &container, record).map(|()| record.identity),
        outcome: &|_| {
            MutationOutcome::ExactError(
                OptimizedProgramStorageSemanticWrapperObjectRecordError::ManifestMismatch,
            )
        },
        // Replay must reject a record the codec still accepts: the
        // substitution is a representable envelope, not a wire refusal.
        joined_replay: Some(&|record, field| {
            assert_eq!(
                OptimizedProgramStorageSemanticWrapperObjectManifest::decode(&record.encode()),
                Ok(record.clone()),
                "reauthenticated {field:?} must retain a valid manifest envelope",
            );
        }),
    });
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
            Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::ManifestMismatch),
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
        Err(OptimizedProgramStorageSemanticWrapperObjectRecordError::ManifestMismatch),
    );
}
