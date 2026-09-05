use crate::FunctionFragmentReplayInputs;
use crate::{
    validate_allocation_recovery_function_relative_realization,
    validate_fixed_frame_function_relative_realization,
    validate_function_relative_layout_optimization_realization_custody,
    validate_optimized_structural_unit_function_relative_realization,
    validate_optimized_unit_function_relative_realization,
    validate_post_allocation_machine_function_relative_realization_custody,
    validate_selected_lowering_function_relative_realization_custody,
};
use machine_code::FunctionFragmentEmissionPlan;
use optimization_core::OptimizationSelections;
use target::Architecture;

use super::error::FunctionFragmentEmissionError;
use super::model::{
    StagedFunctionFragmentEmissionCustodyReceipt, ValidatedFunctionFragmentEmissionManifest,
};
use super::source::StagedOptimizedFunctionFragmentEmissionSource;

pub(super) fn validate_source(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<(), FunctionFragmentEmissionError> {
    match source.replay() {
        FunctionFragmentReplayInputs::X86Rel8Direct(realization) => {
            validate_function_relative_layout_optimization_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
            if realization.layout().target().architecture != Architecture::X86_64 {
                return Err(FunctionFragmentEmissionError::SourceKindMismatch);
            }
        }
        FunctionFragmentReplayInputs::SelectedLowering(realization) => {
            validate_selected_lowering_function_relative_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
        }
        FunctionFragmentReplayInputs::PostAllocationMachine(realization) => {
            validate_post_allocation_machine_function_relative_realization_custody(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
        }
        FunctionFragmentReplayInputs::AllocationRecovery(realization) => {
            validate_allocation_recovery_function_relative_realization(realization).map_err(
                |error| FunctionFragmentEmissionError::AllocationRecoverySource(Box::new(error)),
            )?;
        }
        FunctionFragmentReplayInputs::UnitBaseline(realization) => {
            validate_optimized_unit_function_relative_realization(realization)
                .map_err(FunctionFragmentEmissionError::UnitSource)?;
        }
        FunctionFragmentReplayInputs::StructuralUnit(realization) => {
            validate_optimized_structural_unit_function_relative_realization(realization)
                .map_err(FunctionFragmentEmissionError::StructuralUnitSource)?;
        }
        FunctionFragmentReplayInputs::FixedFrame(realization) => {
            validate_fixed_frame_function_relative_realization(realization)
                .map_err(FunctionFragmentEmissionError::Source)?;
        }
    }
    source.validate_current()?;
    let expected_allocation_recovery = match source.replay() {
        FunctionFragmentReplayInputs::AllocationRecovery(realization) => realization
            .allocation()
            .current()
            .selections()
            .for_phase(optimization_core::OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        FunctionFragmentReplayInputs::PostAllocationMachine(realization) => realization
            .allocation()
            .current()
            .selections()
            .for_phase(optimization_core::OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        _ => OptimizationSelections::default().identity(),
    };
    let source_manifest = source.function_relative_manifest().record();
    if source_manifest.allocation_recovery_selections != expected_allocation_recovery
        || matches!(
            source.replay(),
            FunctionFragmentReplayInputs::AllocationRecovery(_)
        ) && source_manifest.selections != expected_allocation_recovery
    {
        return Err(FunctionFragmentEmissionError::RootMismatch);
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
