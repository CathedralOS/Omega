//! Text placement: the fourth machine-emission stage.
//!
//! [`stage_optimized_fixed_frame_text_section`] admits one frame application
//! by replaying its custody, places the framed fragments into one dense
//! relocation-free text section with every ordinary internal call resolved,
//! binds the section manifest and custody receipt (`assembly`), and returns
//! only after [`validate_optimized_fixed_frame_text_section`] (`validation`)
//! replays the whole join. `carriers` holds the retained stage and receipt.
//!
//! `placement` is the placement algorithm and its independent checker over
//! current fragment data; source admission and publication stay at this level.
//! Machine-code owns the raw section records and codecs.

mod assembly;
mod carriers;
mod error;
pub(crate) mod placement;
mod validation;

pub use carriers::{
    StagedFixedFrameTextSectionCustodyReceipt, StagedOptimizedFixedFrameTextSection,
    ValidatedFunctionFragmentTextSectionManifest,
};
pub use error::{
    FunctionFragmentTextSectionManifestDecodeError, RelocationFreeTextSectionPlacementError,
};
pub use machine_code::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionStage,
    FunctionFragmentTextSectionStatistics, FunctionFragmentTextSectionUnavailableData,
};
pub use validation::validate_optimized_fixed_frame_text_section;

use crate::frame_application::{
    StagedFunctionFragmentFrameApplication, validate_function_fragment_frame_application,
};
use assembly::{compute_fixed_frame, fixed_frame_receipt};
#[cfg(any(test, feature = "test-support"))]
use machine_code::{FunctionFragmentEmissionPlan, RelocationFreeTextSectionPlacement};

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

/// Place fragments that carry no internal call directly, outside any stage
/// custody; a test fixture for placement order and replay, not an admission.
#[cfg(any(test, feature = "test-support"))]
pub fn place_fragments_for_test(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    placement::place_fragment_text_section(placement::TextPlacementInput::RelocationFree(fragments))
        .map_err(Into::into)
}
