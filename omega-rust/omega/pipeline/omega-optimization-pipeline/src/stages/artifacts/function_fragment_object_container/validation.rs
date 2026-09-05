//! Admission of retained publication without reconstructing its output.
use super::*;
use omega_object_file::{
    decode_relocation_free_object, relocation_free_object_statistics,
    validate_relocation_free_object_from_text,
};

pub fn validate_optimized_relocation_free_object_container(
    staged: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<StagedRelocationFreeObjectContainerCustodyReceipt, RelocationFreeObjectContainerError> {
    staged.source.validate()?;
    validate_relocation_free_object_from_text(
        staged.source.text_section(),
        staged.source.manifest().record().selections,
        &staged.object,
    )
    .map_err(object_error)?;
    if staged.container.object != staged.object.identity
        || staged.container.identity
            != RelocationFreeObjectContainerIdentity::from_canonical_bytes(&staged.container.bytes)
    {
        return Err(RelocationFreeObjectContainerError::ContainerMismatch);
    }
    // Exact tags, UTF-8, fixed-width fields, canonical rows and exact end make
    // the decoded representation unique. Replay need not invoke the encoder.
    let decoded = decode_relocation_free_object(&staged.container.bytes)
        .map_err(RelocationFreeObjectContainerError::InvalidContainer)?;
    if decoded != *staged.object {
        return Err(RelocationFreeObjectContainerError::ContainerMismatch);
    }
    validate_manifest(staged)?;
    let expected = receipt(&staged.manifest, &staged.object, &staged.container);
    if staged.custody != expected {
        return Err(RelocationFreeObjectContainerError::ReceiptMismatch);
    }
    Ok(expected)
}

fn validate_manifest(
    staged: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<(), RelocationFreeObjectContainerError> {
    let record = staged.manifest.record();
    let object = staged.object();
    let statistics = relocation_free_object_statistics(object, staged.container())
        .map_err(|_| RelocationFreeObjectContainerError::LengthOverflow)?;
    if record.identity != record.recomputed_identity()
        || record.source_text_section_manifest != staged.source.manifest().record().identity
        || record.text_section != staged.source.text_section().identity
        || record.psi != object.psi
        || record.fuel_schedule != object.fuel_schedule
        || record.selections != object.selections
        || record.selected != object.selected
        || record.target != object.target
        || record.semantic_entry != object.semantic_entry
        || record.semantic_entry_symbol != object.semantic_entry_symbol
        || record.symbol_policy != object.symbol_policy
        || record.object != object.identity
        || record.object_container != staged.container.identity
        || record.relocation_requirements != object.relocation_requirements
        || record.statistics != statistics
    {
        return Err(RelocationFreeObjectContainerError::ManifestMismatch);
    }
    // Exhaustive matches force future role additions to receive admission rules.
    match record.stage {
        FunctionFragmentObjectContainerStage::ValidatedRelocationFreeObjectContainerV1 => {}
    }
    for unavailable in [
        record.external_entry_bridge,
        record.executable_image,
        record.installation,
        record.publication,
    ] {
        match unavailable {
            FunctionFragmentObjectContainerUnavailableData::Unavailable => {}
        }
    }
    Ok(())
}
