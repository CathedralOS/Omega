//! Relocation-free text placement from validated function fragments.
//!
//! This entrance owns the stage/validation join. Data and custody live in
//! `model` and `carriers`; canonical serialization lives in
//! `manifest_codec`; `placement` resolves fragment spans and internal calls;
//! and `assembly` seals the resulting manifest and statistics.

mod assembly;
mod carriers;
mod error;
mod manifest_codec;
mod model;
mod placement;

pub use carriers::*;
pub use error::*;
pub use model::*;

#[cfg(test)]
pub(crate) use placement::{place_fragments_for_test, place_structural_unit_fragments_for_test};

use crate::{
    StagedOptimizedFunctionFragmentEmission, validate_optimized_function_fragment_emission,
};

use assembly::{compute, receipt};

pub fn stage_optimized_relocation_free_text_section(
    source: StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedOptimizedRelocationFreeTextSection, RelocationFreeTextSectionPlacementError> {
    validate_optimized_function_fragment_emission(&source)
        .map_err(RelocationFreeTextSectionPlacementError::Source)?;
    let (text_section, manifest) = compute(&source)?;
    let custody = receipt(&manifest, &text_section);
    let staged = StagedOptimizedRelocationFreeTextSection {
        source,
        text_section: Box::new(text_section),
        manifest,
        custody,
    };
    validate_optimized_relocation_free_text_section(&staged)?;
    Ok(staged)
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
