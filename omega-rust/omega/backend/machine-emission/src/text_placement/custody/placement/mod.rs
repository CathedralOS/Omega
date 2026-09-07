//! Optimizer module role: executable entrance. Fragment placement route map.
//!
//! This entrance admits the exact fragment source shape, then dispatches to
//! ordinary relocation-free placement or frame-applied call resolution.

use crate::{TextPlacementInput, place_fragment_text_section};
use machine_code::RelocationFreeTextSectionPlacement;

#[cfg(any(test, feature = "test-support"))]
use machine_code::FunctionFragmentEmissionPlan;

use crate::StagedOptimizedFunctionFragmentEmission;
use machine_code::{FunctionFragmentEmissionSourceKind, FunctionFragmentEmissionStage};

use super::RelocationFreeTextSectionPlacementError;

pub(super) fn place_fragments(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_fragment_text_section(input(source)?).map_err(Into::into)
}

pub(super) fn input(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<TextPlacementInput<'_>, RelocationFreeTextSectionPlacementError> {
    let fragments = source.fragments();
    let source_manifest = source.manifest().record();
    match (
        fragments.functions.is_empty(),
        source_manifest.stage,
        source_manifest.source_kind,
    ) {
        (
            false,
            FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
            FunctionFragmentEmissionSourceKind::SelectedLoweringV1
            | FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 { .. }
            | FunctionFragmentEmissionSourceKind::UnitBaselineV1,
        ) => Ok(TextPlacementInput::RelocationFree(fragments)),
        _ => Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
    }
}

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
