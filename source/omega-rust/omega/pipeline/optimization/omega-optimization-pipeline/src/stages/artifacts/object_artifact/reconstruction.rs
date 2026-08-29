//! Independent semantic/proof-to-object reconstruction.

use super::*;

pub(super) fn validate_terminal_join(
    terminal: &psi_terminal_codec::CanonicalTerminalArtifact,
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<(), OptimizedObjectArtifactError> {
    terminal
        .validate()
        .map_err(|_| OptimizedObjectArtifactError::InvalidTerminalArtifact)?;
    let terminal_manifest = terminal.manifest();
    if terminal_manifest.installation().is_some() {
        return Err(OptimizedObjectArtifactError::InstallationAlreadyPresent);
    }
    let module = psi_terminal_codec::decode_module(terminal.semantic_bytes())
        .map_err(|_| OptimizedObjectArtifactError::InvalidTerminalArtifact)?;
    let proof = psi_terminal_codec::decode_proof_bundle(terminal.proof_bytes())
        .map_err(|_| OptimizedObjectArtifactError::InvalidTerminalArtifact)?;
    let input = source.verified_input();
    if module != *input.context().module() || terminal_manifest.semantic() != source.object().psi {
        return Err(OptimizedObjectArtifactError::SemanticMismatch);
    }
    if proof != *input.context().proof_bundle()
        || terminal_manifest.proof() != input.context().proof_bundle_fingerprint()
    {
        return Err(OptimizedObjectArtifactError::ProofMismatch);
    }
    if module.entry != source.object().semantic_entry {
        return Err(OptimizedObjectArtifactError::EntryMismatch);
    }
    let encoded_object_manifest = source.manifest().record().encode();
    if FunctionFragmentObjectContainerManifest::decode(&encoded_object_manifest)
        .map_err(|_| OptimizedObjectArtifactError::InvalidObjectManifest)?
        != *source.manifest().record()
    {
        return Err(OptimizedObjectArtifactError::InvalidObjectManifest);
    }
    Ok(())
}

pub(super) fn construct_artifact(
    terminal: &psi_terminal_codec::CanonicalTerminalArtifact,
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<OptimizedObjectArtifactRecord, OptimizedObjectArtifactError> {
    let terminal_manifest = terminal.manifest();
    let emission = source.source().source();
    let relative = emission.function_relative_manifest().record();
    let object_manifest = source.manifest().record();
    let mut record = OptimizedObjectArtifactRecord {
        identity: OptimizedObjectArtifactIdentity::from_canonical_bytes(b"pending"),
        psi_artifact: *terminal_manifest.identity().as_bytes(),
        psi: terminal_manifest.semantic(),
        obligation_ledger: *terminal_manifest.obligations().as_bytes(),
        proof_bundle: *terminal_manifest.proof().as_bytes(),
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

pub(super) fn replay_artifact(
    terminal: &psi_terminal_codec::CanonicalTerminalArtifact,
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<OptimizedObjectArtifactRecord, OptimizedObjectArtifactError> {
    let canonical = terminal.manifest();
    let text_stage = source.source();
    let fragment_stage = text_stage.source();
    let realization = fragment_stage.function_relative_manifest().record();
    let object = source.object();
    let mut record = OptimizedObjectArtifactRecord {
        identity: OptimizedObjectArtifactIdentity::from_canonical_bytes(b"replay"),
        psi_artifact: *canonical.identity().as_bytes(),
        psi: object.psi,
        obligation_ledger: *canonical.obligations().as_bytes(),
        proof_bundle: *canonical.proof().as_bytes(),
        debug_section: canonical.debug().map(|value| *value.as_bytes()),
        selections: text_stage.manifest().record().selections,
        target: text_stage.text_section().target,
        semantic_entry: text_stage.text_section().semantic_entry,
        pre_physical_manifest: realization.pre_physical_manifest,
        post_allocation_manifest: fragment_stage.manifest().record().post_allocation_manifest,
        function_relative_manifest: fragment_stage.manifest().record().source_realization,
        function_fragment_manifest: text_stage.manifest().record().source_fragment_manifest,
        text_section_manifest: source.manifest().record().source_text_section_manifest,
        object_container_manifest: source.manifest().record().identity,
        object: source.container().object,
        object_container: RelocationFreeObjectContainerIdentity::from_canonical_bytes(
            &source.container().bytes,
        ),
        statistics: OptimizedObjectArtifactStatistics {
            text_bytes: object.text_section.byte_count,
            object_container_bytes: u64::try_from(source.container().bytes.len())
                .map_err(|_| OptimizedObjectArtifactError::LengthOverflow)?,
            function_symbols: u64::try_from(object.symbols.len())
                .map_err(|_| OptimizedObjectArtifactError::LengthOverflow)?,
            relocation_records: object.relocation_record_count,
        },
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
