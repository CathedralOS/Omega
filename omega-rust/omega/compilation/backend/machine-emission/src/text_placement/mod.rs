//! Text placement: the fourth machine-emission stage.
//!
//! [`stage_optimized_fixed_frame_text_section`] places one frame
//! application's framed fragments into one dense relocation-free text section
//! with every ordinary internal call resolved and binds the section manifest
//! and custody receipt (`assembly`).
//! [`validate_optimized_fixed_frame_text_section`] (`validation`) replays the
//! whole join for retained-artifact readers and tests; the stage does not
//! replay what it just produced. `carriers` holds the retained stage and
//! receipt.
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

use crate::frame_application::StagedFunctionFragmentFrameApplication;
use assembly::{compute_fixed_frame, fixed_frame_receipt};

/// Resolve every ordinary typed internal call after the exact target frame has
/// shifted function-relative coordinates, then publish a relocation-free text
/// section bound to that frame application.
pub fn stage_optimized_fixed_frame_text_section(
    source: StagedFunctionFragmentFrameApplication,
) -> Result<StagedOptimizedFixedFrameTextSection, RelocationFreeTextSectionPlacementError> {
    let (text_section, manifest) = compute_fixed_frame(&source)?;
    let custody = fixed_frame_receipt(&source, &manifest, &text_section);
    Ok(StagedOptimizedFixedFrameTextSection {
        source,
        text_section: std::sync::Arc::new(text_section),
        manifest,
        custody,
    })
}
