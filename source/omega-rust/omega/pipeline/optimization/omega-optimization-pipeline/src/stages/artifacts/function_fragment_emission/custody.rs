use crate::{
    validate_allocation_recovery_function_relative_realization,
    validate_fixed_frame_function_relative_realization,
    validate_function_relative_layout_optimization_realization_custody,
    validate_optimized_structural_unit_function_relative_realization,
    validate_optimized_unit_function_relative_realization,
    validate_post_allocation_machine_function_relative_realization_custody,
    validate_selected_lowering_function_relative_realization_custody,
};
use omega_machine_code::FunctionFragmentEmissionPlan;
use omega_target::Architecture;

use super::error::FunctionFragmentEmissionError;
use super::model::{
    StagedFunctionFragmentEmissionCustodyReceipt, ValidatedFunctionFragmentEmissionManifest,
};
use super::source::StagedOptimizedFunctionFragmentEmissionSource;

pub(super) fn validate_source(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<(), FunctionFragmentEmissionError> {
    match source {
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(realization) => {
            validate_function_relative_layout_optimization_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
            if realization.layout().target().architecture != Architecture::X86_64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        StagedOptimizedFunctionFragmentEmissionSource::SelectedLowering(realization) => {
            validate_selected_lowering_function_relative_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
        }
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(realization) => {
            validate_post_allocation_machine_function_relative_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
        }
        StagedOptimizedFunctionFragmentEmissionSource::AllocationRecovery(realization) => {
            validate_allocation_recovery_function_relative_realization(realization).map_err(
                |error| FunctionFragmentEmissionError::AllocationRecoverySource(Box::new(error)),
            )?;
        }
        StagedOptimizedFunctionFragmentEmissionSource::UnitBaseline(realization) => {
            validate_optimized_unit_function_relative_realization(realization)
                .map_err(FunctionFragmentEmissionError::UnitSource)?;
        }
        StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(realization) => {
            validate_optimized_structural_unit_function_relative_realization(realization)
                .map_err(FunctionFragmentEmissionError::StructuralUnitSource)?;
        }
        StagedOptimizedFunctionFragmentEmissionSource::FixedFrame(realization) => {
            validate_fixed_frame_function_relative_realization(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
        }
    }
    Ok(())
}
pub(super) fn receipt(
    manifest: &ValidatedFunctionFragmentEmissionManifest,
    fragments: &FunctionFragmentEmissionPlan,
) -> StagedFunctionFragmentEmissionCustodyReceipt {
    StagedFunctionFragmentEmissionCustodyReceipt {
        source_realization: manifest.record.source_realization,
        fragments: fragments.identity,
        manifest: manifest.record.identity,
    }
}
