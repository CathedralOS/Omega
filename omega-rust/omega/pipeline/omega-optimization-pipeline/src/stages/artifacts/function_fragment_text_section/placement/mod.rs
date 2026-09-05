//! Optimizer module role: executable entrance. Fragment placement route map.
//!
//! This entrance admits the exact fragment source shape, then dispatches to
//! ordinary relocation-free placement or structural-Unit call resolution.

use omega_machine_code::RelocationFreeTextSectionPlacement;
use omega_machine_emission::{
    StructuralFragmentPlacementInputs, TextPlacementInput, place_fragment_text_section,
};

#[cfg(test)]
use omega_machine_code::FunctionFragmentEmissionPlan;

use crate::{
    FunctionFragmentEmissionSourceKind, FunctionFragmentEmissionStage,
    StagedOptimizedFunctionFragmentEmission,
};

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
        fragments.structural_unit_functions.is_empty(),
        source_manifest.stage,
        source_manifest.source_kind,
    ) {
        (
            false,
            true,
            FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
            FunctionFragmentEmissionSourceKind::X86Rel8V1
            | FunctionFragmentEmissionSourceKind::SelectedLoweringV1
            | FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 { .. }
            | FunctionFragmentEmissionSourceKind::AllocationRecoveryV1
            | FunctionFragmentEmissionSourceKind::UnitBaselineV1,
        ) => Ok(TextPlacementInput::RelocationFree(fragments)),
        (
            true,
            false,
            FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1
            | FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1,
            FunctionFragmentEmissionSourceKind::StructuralUnitV1,
        ) => {
            let current = source.source();
            if !current.encoding().rows().is_empty() {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            Ok(TextPlacementInput::Structural {
                fragments,
                facts: StructuralFragmentPlacementInputs {
                    program: current.program(),
                    structural_encoding: current.encoding().structural_unit_functions(),
                    exit: current.exit_contract().contract(),
                    physical: current.register_environment().physical(),
                    constraints: current.register_environment().constraints(),
                },
            })
        },
        _ => Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
    }
}

#[cfg(test)]
pub(crate) fn place_fragments_for_test(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_fragment_text_section(TextPlacementInput::RelocationFree(fragments)).map_err(Into::into)
}

#[cfg(test)]
pub(crate) fn place_structural_unit_fragments_for_test(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_fragments(source)
}

pub(super) fn usize_to_u64(value: usize) -> Result<u64, RelocationFreeTextSectionPlacementError> {
    value
        .try_into()
        .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)
}

pub(super) fn place_fixed_frame_fragments(
    source: &crate::StagedFunctionFragmentFrameApplication,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_fragment_text_section(TextPlacementInput::InternalCalls(source.fragments()))
        .map_err(Into::into)
}
