use crate::validate_fixed_frame_function_relative_realization;
use machine_code::FunctionFragmentEmissionPlan;

use super::error::FunctionFragmentEmissionError;
use super::model::{
    StagedFunctionFragmentEmissionCustodyReceipt, ValidatedFunctionFragmentEmissionManifest,
};
use super::source::StagedOptimizedFunctionFragmentEmissionSource;

pub(super) fn validate_source(
    source: &StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<(), FunctionFragmentEmissionError> {
    validate_fixed_frame_function_relative_realization(source.replay().fixed_frame())
        .map_err(FunctionFragmentEmissionError::Source)?;
    source.validate_current()?;
    let expected_allocation_recovery = source
        .optimized_target()
        .optimized()
        .selections()
        .for_phase(optimization_core::OptimizationExecutionPhase::AllocationRecovery)
        .identity();
    if source
        .function_relative_manifest()
        .record()
        .allocation_recovery_selections
        != expected_allocation_recovery
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
