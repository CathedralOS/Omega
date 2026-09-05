//! Reauthenticated ordinary-callable manifest field mutations.

use crate::tests::*;
use object_file::ObjectLocalSymbolId;
use optimization_core::{
    OptimizationSelectionIdentity, OptimizedObjectArtifactIdentity,
    OptimizedObjectArtifactManifestIdentity, OptimizedTerminalOrdinaryCallableEntryIdentity,
};
use target::{Architecture, ObjectFormat};
use terminal_psi::SemanticFingerprint;

use super::fixture::staged_callable;

type ManifestMutation = fn(&mut OptimizedOrdinaryCallableEntryManifest);

#[test]
fn every_representable_ordinary_callable_manifest_field_rejects_after_reauthentication() {
    let mut staged = staged_callable();
    let baseline = staged.manifest().record().clone();
    // Stage, vocabulary, disposition, and unavailable-data values are singleton
    // in memory; their closed tags are covered by the wire matrix.
    let mutations: [(&str, ManifestMutation); 14] = [
        ("entry", |record| {
            record.entry =
                OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(b"other entry")
        }),
        ("source_artifact", |record| {
            record.source_artifact =
                OptimizedObjectArtifactIdentity::from_canonical_bytes(b"other source artifact")
        }),
        ("source_manifest", |record| {
            record.source_manifest = OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(
                b"other source manifest",
            )
        }),
        ("psi.program_fingerprint", |record| {
            record.psi.program_fingerprint = SemanticFingerprint::from_bytes([0xd1; 32])
        }),
        ("selections", |record| {
            record.selections = OptimizationSelectionIdentity::from_bytes([0xd2; 32])
        }),
        ("target.architecture", |record| {
            record.target.architecture = Architecture::Aarch64
        }),
        ("target.object_format", |record| {
            record.target.object_format = ObjectFormat::Coff
        }),
        ("target.pointer_size", |record| {
            record.target.pointer_size += 1
        }),
        ("target.pointer_alignment", |record| {
            record.target.pointer_alignment += 1
        }),
        ("semantic_entry", |record| {
            record.semantic_entry = MachineId::new(99_901).unwrap()
        }),
        ("semantic_entry_symbol", |record| {
            record.semantic_entry_symbol = ObjectLocalSymbolId::new(99_902).unwrap()
        }),
        ("exit_contract", |record| {
            record.exit_contract = WholeFunctionExitContractIdentity::from_bytes([0xd3; 32])
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

    *staged.manifest_mut().record_mut() = baseline;
    staged.manifest_mut().record_mut().identity =
        optimization_core::OptimizedOrdinaryCallableEntryManifestIdentity::from_bytes([0xd4; 32]);
    assert_eq!(
        validate_optimized_ordinary_callable_entry(&staged),
        Err(OptimizedOrdinaryCallableEntryError::ManifestMismatch),
    );
}
