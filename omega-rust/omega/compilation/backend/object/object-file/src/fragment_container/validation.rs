//! Admission of retained publication without reconstructing its output.
use super::{
    RelocationFreeObjectContainerError, RelocationFreeObjectContainerIdentity,
    StagedOptimizedRelocationFreeObjectContainer,
    StagedRelocationFreeObjectContainerCustodyReceipt, object_error,
    validate_optimized_fixed_frame_text_section,
};
use crate::fragment_container::reconstruction::{construct_manifest, receipt};
use crate::{decode_relocation_free_object, validate_relocation_free_object_from_text};

pub fn validate_optimized_relocation_free_object_container(
    staged: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<StagedRelocationFreeObjectContainerCustodyReceipt, RelocationFreeObjectContainerError> {
    validate_optimized_fixed_frame_text_section(&staged.source)
        .map_err(RelocationFreeObjectContainerError::Source)?;
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
    // `construct_manifest` owns which manifest fields the source, the object and
    // the container decide. Admission rebuilds that record and compares it
    // whole: the field correspondence is not restated here, and a future stage
    // or unavailable role reaches admission through the constructor's single
    // owner.
    let expected = construct_manifest(&staged.source, staged.object(), staged.container())?;
    if staged.manifest.record() == expected.record() {
        Ok(())
    } else {
        Err(RelocationFreeObjectContainerError::ManifestMismatch)
    }
}
