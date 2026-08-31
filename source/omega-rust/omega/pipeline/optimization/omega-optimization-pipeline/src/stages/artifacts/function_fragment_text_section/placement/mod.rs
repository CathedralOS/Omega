//! Optimizer module role: executable entrance. Fragment placement route map.
//!
//! This entrance admits the exact fragment source shape, then dispatches to
//! ordinary relocation-free placement or structural-Unit call resolution.

mod conversion;
mod relocation_free;
mod structural_unit;

use omega_object_file::RelocationFreeTextSectionPlacement;

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
        ) => relocation_free::place(fragments),
        (
            true,
            false,
            FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1
            | FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1,
            FunctionFragmentEmissionSourceKind::StructuralUnitV1,
        ) => structural_unit::place(source),
        _ => Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
    }
}

#[cfg(test)]
pub(crate) fn place_fragments_for_test(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    relocation_free::place(fragments)
}

#[cfg(test)]
pub(crate) fn place_structural_unit_fragments_for_test(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    structural_unit::place(source)
}

pub(super) use conversion::usize_to_u64;
