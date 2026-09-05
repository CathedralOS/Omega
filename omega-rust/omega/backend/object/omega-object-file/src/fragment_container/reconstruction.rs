//! Bind object publication claims and the source custody receipt.
use super::*;

pub(super) fn construct_manifest(
    source: &StagedOptimizedObjectTextSectionSource,
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> Result<ValidatedFunctionFragmentObjectContainerManifest, RelocationFreeObjectContainerError> {
    let unavailable = FunctionFragmentObjectContainerUnavailableData::Unavailable;
    let statistics = crate::relocation_free_object_statistics(object, container)
        .map_err(|_| RelocationFreeObjectContainerError::LengthOverflow)?;
    let mut record = FunctionFragmentObjectContainerManifest {
        identity: FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"pending"),
        stage: FunctionFragmentObjectContainerStage::ValidatedRelocationFreeObjectContainerV1,
        source_text_section_manifest: source.manifest().record().identity,
        text_section: source.text_section().identity,
        psi: object.psi,
        fuel_schedule: object.fuel_schedule,
        selections: object.selections,
        selected: object.selected,
        target: object.target,
        semantic_entry: object.semantic_entry,
        semantic_entry_symbol: object.semantic_entry_symbol,
        symbol_policy: object.symbol_policy,
        object: object.identity,
        object_container: container.identity,
        relocation_requirements: object.relocation_requirements,
        statistics,
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionFragmentObjectContainerManifest {
        record: std::sync::Arc::new(record),
    })
}

pub(super) fn receipt(
    manifest: &ValidatedFunctionFragmentObjectContainerManifest,
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> StagedRelocationFreeObjectContainerCustodyReceipt {
    StagedRelocationFreeObjectContainerCustodyReceipt {
        source_text_section_manifest: manifest.record.source_text_section_manifest,
        text_section: object.source_text_section,
        object: object.identity,
        object_container: container.identity,
        manifest: manifest.record.identity,
    }
}
