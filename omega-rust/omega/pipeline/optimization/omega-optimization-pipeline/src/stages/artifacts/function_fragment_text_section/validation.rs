//! Independent replay for direct and fixed-frame text-section artifacts.

use crate::{
    validate_function_fragment_frame_application, validate_optimized_function_fragment_emission,
};

use super::{
    RelocationFreeTextSectionPlacementError, StagedFixedFrameTextSectionCustodyReceipt,
    StagedOptimizedFixedFrameTextSection, StagedOptimizedRelocationFreeTextSection,
    StagedRelocationFreeTextSectionCustodyReceipt,
    assembly::{compute, compute_fixed_frame, fixed_frame_receipt, receipt},
};

pub fn validate_optimized_fixed_frame_text_section(
    staged: &StagedOptimizedFixedFrameTextSection,
) -> Result<StagedFixedFrameTextSectionCustodyReceipt, RelocationFreeTextSectionPlacementError> {
    validate_function_fragment_frame_application(&staged.source)
        .map_err(RelocationFreeTextSectionPlacementError::FrameSource)?;
    let (expected_section, expected_manifest) = compute_fixed_frame(&staged.source)?;
    if staged.text_section.recomputed_identity() != staged.text_section.identity
        || staged.text_section.as_ref() != &expected_section
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    if staged.manifest != expected_manifest {
        return Err(RelocationFreeTextSectionPlacementError::ManifestMismatch);
    }
    let expected_receipt =
        fixed_frame_receipt(&staged.source, &expected_manifest, &expected_section);
    if staged.custody != expected_receipt {
        return Err(RelocationFreeTextSectionPlacementError::ReceiptMismatch);
    }
    Ok(expected_receipt)
}

pub fn validate_optimized_relocation_free_text_section(
    staged: &StagedOptimizedRelocationFreeTextSection,
) -> Result<StagedRelocationFreeTextSectionCustodyReceipt, RelocationFreeTextSectionPlacementError>
{
    validate_optimized_function_fragment_emission(&staged.source)
        .map_err(RelocationFreeTextSectionPlacementError::Source)?;
    let (expected_section, expected_manifest) = compute(&staged.source)?;
    if staged.text_section.recomputed_identity() != staged.text_section.identity
        || staged.text_section.as_ref() != &expected_section
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    if staged.manifest != expected_manifest {
        return Err(RelocationFreeTextSectionPlacementError::ManifestMismatch);
    }
    let expected_receipt = receipt(&expected_manifest, &expected_section);
    if staged.custody != expected_receipt {
        return Err(RelocationFreeTextSectionPlacementError::ReceiptMismatch);
    }
    Ok(expected_receipt)
}
