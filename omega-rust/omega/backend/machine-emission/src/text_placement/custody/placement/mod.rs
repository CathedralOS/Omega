//! Optimizer module role: executable entrance. Frame-applied fragment placement.
//!
//! The current frame application supplies the exact instruction and call coordinates.
//! Placement resolves those calls; independent validation checks the resulting text.

use crate::{TextPlacementInput, place_fragment_text_section};
use machine_code::RelocationFreeTextSectionPlacement;

#[cfg(any(test, feature = "test-support"))]
use machine_code::FunctionFragmentEmissionPlan;

use super::RelocationFreeTextSectionPlacementError;

#[cfg(any(test, feature = "test-support"))]
pub fn place_fragments_for_test(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_fragment_text_section(TextPlacementInput::RelocationFree(fragments)).map_err(Into::into)
}

pub(super) fn place_fixed_frame_fragments(
    source: &crate::StagedFunctionFragmentFrameApplication,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_fragment_text_section(TextPlacementInput::InternalCalls(source.fragments()))
        .map_err(Into::into)
}
