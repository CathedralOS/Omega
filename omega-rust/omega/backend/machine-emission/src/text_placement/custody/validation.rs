//! Independent replay for direct and fixed-frame text-section artifacts.

mod manifest_fields;

use crate::{
    validate_function_fragment_frame_application, validate_optimized_function_fragment_emission,
};

use super::{
    FunctionFragmentTextSectionSourceCustody as Custody, FunctionFragmentTextSectionStage as Stage,
    RelocationFreeTextSectionPlacementError, StagedFixedFrameTextSectionCustodyReceipt,
    StagedOptimizedFixedFrameTextSection, StagedOptimizedRelocationFreeTextSection,
    StagedRelocationFreeTextSectionCustodyReceipt,
    assembly::{fixed_frame_receipt, receipt},
};

pub fn validate_optimized_fixed_frame_text_section(
    staged: &StagedOptimizedFixedFrameTextSection,
) -> Result<StagedFixedFrameTextSectionCustodyReceipt, RelocationFreeTextSectionPlacementError> {
    validate_function_fragment_frame_application(&staged.source)
        .map_err(RelocationFreeTextSectionPlacementError::FrameSource)?;
    crate::validate_fragment_text_section(
        crate::TextPlacementInput::InternalCalls(staged.source.fragments()),
        &staged.text_section,
    )?;
    manifest_fields::check(
        staged.source.source().manifest().record(),
        Stage::ValidatedFixedFrameInternalCallTextSectionPlacementV1,
        Custody::FixedFrameApplicationV1 {
            application: staged.source.receipt().identity(),
        },
        &staged.text_section,
        staged.source.fragments(),
        staged.manifest.record(),
    )?;
    let expected_receipt =
        fixed_frame_receipt(&staged.source, &staged.manifest, &staged.text_section);
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
    crate::validate_fragment_text_section(
        super::placement::input(&staged.source)?,
        &staged.text_section,
    )?;
    manifest_fields::check(
        staged.source.manifest().record(),
        Stage::ValidatedRelocationFreeTextSectionPlacementV1,
        Custody::DirectFragmentEmissionV1,
        &staged.text_section,
        staged.source.fragments(),
        staged.manifest.record(),
    )?;
    let expected_receipt = receipt(&staged.manifest, &staged.text_section);
    if staged.custody != expected_receipt {
        return Err(RelocationFreeTextSectionPlacementError::ReceiptMismatch);
    }
    Ok(expected_receipt)
}
