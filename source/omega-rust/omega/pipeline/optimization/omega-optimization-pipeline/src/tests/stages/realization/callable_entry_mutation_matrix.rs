//! Reauthenticated ordinary-callable manifest and receipt mutation coverage.

use crate::tests::*;

use super::callable_entry::staged_callable_object_artifact;

type ManifestMutation = fn(&mut OptimizedOrdinaryCallableEntryManifest);

#[test]
fn ordinary_callable_manifest_rejects_every_representable_field_mutation() {
    let mut staged = stage_validated_optimized_ordinary_callable_entry(
        staged_callable_object_artifact(NativeTarget::linux_x64(), false),
    )
    .unwrap();
    let baseline = staged.manifest().record().clone();
    let mutations: [(&str, ManifestMutation); 11] = [
        ("entry", |record| {
            record.entry = omega_optimization_core::OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(b"other entry")
        }),
        ("source_artifact", |record| {
            record.source_artifact =
                omega_optimization_core::OptimizedObjectArtifactIdentity::from_canonical_bytes(
                    b"other source artifact",
                )
        }),
        ("source_manifest", |record| {
            record.source_manifest = omega_optimization_core::OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(b"other source manifest")
        }),
        ("psi", |record| {
            record.psi.program_fingerprint =
                psi_terminal::SemanticFingerprint::from_bytes([0x5a; 32])
        }),
        ("selections", |record| {
            record.selections =
                omega_optimization_core::OptimizationSelectionIdentity::from_bytes([0x6b; 32])
        }),
        ("target", |record| {
            record.target = NativeTarget::linux_arm64()
        }),
        ("semantic_entry", |record| {
            record.semantic_entry = MachineId::new(99_901).unwrap()
        }),
        ("semantic_entry_symbol", |record| {
            record.semantic_entry_symbol =
                omega_object_file::ObjectLocalSymbolId::new(99_902).unwrap()
        }),
        ("exit_contract", |record| {
            record.exit_contract = WholeFunctionExitContractIdentity::from_bytes([0x6c; 32])
        }),
        ("parameter_count", |record| record.parameter_count += 1),
        ("return_count", |record| record.return_count += 1),
    ];

    for (field, mutate) in mutations {
        *staged.manifest_mut().record_mut() = baseline.clone();
        let record = staged.manifest_mut().record_mut();
        mutate(record);
        record.identity = record.recomputed_identity();
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged),
            Err(OptimizedOrdinaryCallableEntryError::ManifestMismatch),
            "reauthenticated {field} mutation must fail independent replay",
        );
    }

    *staged.manifest_mut().record_mut() = baseline.clone();
    staged.manifest_mut().record_mut().identity =
        omega_optimization_core::OptimizedOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(
            b"other manifest identity",
        );
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&staged),
        Err(OptimizedOrdinaryCallableEntryError::ManifestMismatch),
    );
}

#[test]
fn ordinary_callable_manifest_rejects_closed_enum_and_unavailable_wire_mutations() {
    let staged = stage_validated_optimized_ordinary_callable_entry(
        staged_callable_object_artifact(NativeTarget::linux_x64(), false),
    )
    .unwrap();
    let encoded = staged.manifest().record().encode();

    let mut unknown_stage = encoded.clone();
    unknown_stage[44] = 7;
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&unknown_stage),
        Err(OptimizedOrdinaryCallableEntryManifestDecodeError::UnknownStage(7)),
    );

    let disposition_offset = encoded.len() - 6;
    let mut unknown_disposition = encoded.clone();
    unknown_disposition[disposition_offset] = 7;
    assert_eq!(
        OptimizedOrdinaryCallableEntryManifest::decode(&unknown_disposition),
        Err(OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
            OptimizedOrdinaryCallableEntryDecodeError::UnknownDisposition(7),
        )),
    );

    for offset in encoded.len() - 5..encoded.len() {
        let mut unavailable = encoded.clone();
        unavailable[offset] = 7;
        assert_eq!(
            OptimizedOrdinaryCallableEntryManifest::decode(&unavailable),
            Err(OptimizedOrdinaryCallableEntryManifestDecodeError::UnknownUnavailableStatus),
            "unavailable field at wire offset {offset} must fail closed",
        );
    }
}

#[test]
fn ordinary_callable_receipt_rejects_each_custody_root_mutation() {
    for mutate in [
        StagedValidatedOptimizedOrdinaryCallableEntry::corrupt_custody_source_artifact_for_test,
        StagedValidatedOptimizedOrdinaryCallableEntry::corrupt_custody_entry_for_test,
        StagedValidatedOptimizedOrdinaryCallableEntry::corrupt_custody_for_test,
    ] {
        let mut staged = stage_validated_optimized_ordinary_callable_entry(
            staged_callable_object_artifact(NativeTarget::linux_x64(), false),
        )
        .unwrap();
        mutate(&mut staged);
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&staged),
            Err(OptimizedOrdinaryCallableEntryError::ReceiptMismatch),
        );
    }
}
