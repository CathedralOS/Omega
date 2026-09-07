//! Optimizer module role: executable entrance. Relocation-free text placement from validated function fragments.
//!
//! This entrance owns source admission and the publication join. Machine-code
//! owns raw records and codecs; machine-emission owns placement and counters.
//! `placement` supplies admitted inputs, `assembly` binds publication claims,
//! and `validation` independently checks them. `carriers` retains admission.

mod assembly;
mod carriers;
mod error;
mod placement;
mod validation;

pub use carriers::*;
pub use error::*;
pub use machine_code::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionStage,
    FunctionFragmentTextSectionStatistics, FunctionFragmentTextSectionUnavailableData,
};
pub use validation::*;

#[cfg(any(test, feature = "test-support"))]
pub use placement::place_fragments_for_test;

use crate::{StagedFunctionFragmentFrameApplication, validate_function_fragment_frame_application};

use assembly::{compute_fixed_frame, fixed_frame_receipt};

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
