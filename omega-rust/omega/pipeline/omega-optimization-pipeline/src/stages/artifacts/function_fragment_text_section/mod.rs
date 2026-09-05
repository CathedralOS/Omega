//! Optimizer module role: executable entrance. Relocation-free text placement from validated function fragments.
//!
//! This entrance owns the stage/validation join. Data and custody live in
//! `model` and `carriers`; canonical serialization lives in
//! `manifest_codec`; `placement` admits inputs for the machine-emission backend;
//! and `assembly` seals the resulting manifest and statistics.

mod assembly;
mod carriers;
mod error;
mod manifest_codec;
mod model;
mod placement;
mod validation;

pub use carriers::*;
pub use error::*;
pub use model::*;
pub use validation::*;

#[cfg(test)]
pub(crate) use placement::{place_fragments_for_test, place_structural_unit_fragments_for_test};

use crate::{
    StagedFunctionFragmentFrameApplication, StagedOptimizedFunctionFragmentEmission,
    validate_function_fragment_frame_application, validate_optimized_function_fragment_emission,
};

use assembly::{compute, compute_fixed_frame, fixed_frame_receipt, receipt};

pub fn stage_optimized_relocation_free_text_section(
    source: StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedOptimizedRelocationFreeTextSection, RelocationFreeTextSectionPlacementError> {
    validate_optimized_function_fragment_emission(&source)
        .map_err(RelocationFreeTextSectionPlacementError::Source)?;
    let (text_section, manifest) = compute(&source)?;
    let custody = receipt(&manifest, &text_section);
    let staged = StagedOptimizedRelocationFreeTextSection {
        source,
        text_section: std::sync::Arc::new(text_section),
        manifest,
        custody,
    };
    validate_optimized_relocation_free_text_section(&staged)?;
    Ok(staged)
}

/// Resolve every ordinary typed internal call after the exact target frame has
/// shifted function-relative coordinates, then publish a relocation-free text
/// section bound to that frame application.
pub fn stage_optimized_fixed_frame_text_section(
    source: StagedFunctionFragmentFrameApplication,
) -> Result<StagedOptimizedFixedFrameTextSection, RelocationFreeTextSectionPlacementError> {
    validate_function_fragment_frame_application(&source)
        .map_err(RelocationFreeTextSectionPlacementError::FrameSource)?;
    let (text_section, manifest) = compute_fixed_frame(&source)?;
    let custody = fixed_frame_receipt(&source, &manifest, &text_section);
    let staged = StagedOptimizedFixedFrameTextSection {
        source,
        text_section: std::sync::Arc::new(text_section),
        manifest,
        custody,
    };
    validate_optimized_fixed_frame_text_section(&staged)?;
    Ok(staged)
}
