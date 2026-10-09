//! Independent semantic/proof-to-object reconstruction.

use super::{
      OptimizedObjectArtifactCustodyReceipt,
    OptimizedObjectArtifactError, OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifest,
    OptimizedObjectArtifactManifestIdentity, OptimizedObjectArtifactRecord,
    OptimizedObjectArtifactStage, OptimizedObjectArtifactStatistics,
    OptimizedObjectArtifactUnavailableData,  
    StagedOptimizedRelocationFreeObjectContainer, ValidatedOptimizedObjectArtifactManifest};

pub(super) fn construct_artifact(
    terminal: &terminal_codec::CanonicalTerminalArtifact,
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<OptimizedObjectArtifactRecord, OptimizedObjectArtifactError> {
    let terminal_manifest = terminal.manifest();
    let emission = source.source().source().source();
    let relative = emission.function_relative_manifest().record();
    let object_manifest = source.manifest().record();
    let mut record = OptimizedObjectArtifactRecord {
        identity: OptimizedObjectArtifactIdentity::from_canonical_bytes(b"pending"),
        psi_artifact: *terminal_manifest.identity().as_bytes(),
        psi: terminal_manifest.semantic(),
        debug_section: terminal_manifest.debug().map(|value| *value.as_bytes()),
        selections: source.object().selections,
        target: source.object().target,
        semantic_entry: source.object().semantic_entry,
        pre_physical_manifest: relative.pre_physical_manifest,
        post_allocation_manifest: relative.post_allocation_manifest,
        function_relative_manifest: relative.identity,
        function_fragment_manifest: emission.manifest().record().identity,
        text_section_manifest: source.source().manifest().record().identity,
        object_container_manifest: object_manifest.identity,
        object: source.object().identity,
        object_container: source.container().identity,
        statistics: artifact_statistics(source)?,
    };
    record.identity = record.recomputed_identity();
    Ok(record)
}

pub(super) fn artifact_statistics(
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<OptimizedObjectArtifactStatistics, OptimizedObjectArtifactError> {
    Ok(OptimizedObjectArtifactStatistics {
        text_bytes: source.object().text_section.byte_count,
        object_container_bytes: u64::try_from(source.container().bytes.len())
            .map_err(|_| OptimizedObjectArtifactError::LengthOverflow)?,
        function_symbols: u64::try_from(source.object().symbols.len())
            .map_err(|_| OptimizedObjectArtifactError::LengthOverflow)?,
        relocation_records: source.object().relocation_record_count,
    })
}

pub(super) fn construct_manifest(
    artifact: &OptimizedObjectArtifactRecord,
) -> ValidatedOptimizedObjectArtifactManifest {
    let unavailable = OptimizedObjectArtifactUnavailableData::Unavailable;
    let mut record = OptimizedObjectArtifactManifest {
        identity: OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(b"pending"),
        stage: OptimizedObjectArtifactStage::ValidatedOptimizedObjectArtifactV1,
        artifact: artifact.identity,
        psi_artifact: artifact.psi_artifact,
        psi: artifact.psi,
        selections: artifact.selections,
        target: artifact.target,
        semantic_entry: artifact.semantic_entry,
        object_container_manifest: artifact.object_container_manifest,
        object: artifact.object,
        object_container: artifact.object_container,
        statistics: artifact.statistics,
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    ValidatedOptimizedObjectArtifactManifest { record }
}

pub(super) fn receipt(
    artifact: &OptimizedObjectArtifactRecord,
    manifest: &ValidatedOptimizedObjectArtifactManifest,
) -> OptimizedObjectArtifactCustodyReceipt {
    OptimizedObjectArtifactCustodyReceipt {
        psi_artifact: artifact.psi_artifact,
        object_container_manifest: artifact.object_container_manifest,
        object: artifact.object,
        object_container: artifact.object_container,
        artifact: artifact.identity,
        manifest: manifest.record.identity,
    }
}
