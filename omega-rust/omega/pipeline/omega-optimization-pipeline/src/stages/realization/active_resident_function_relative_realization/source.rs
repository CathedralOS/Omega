use omega_optimization_core::OptimizationExecutionPhase;

use crate::StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout;

use super::model::OptimizedActiveResidentRematerializationFunctionRelativeRealizationError;

pub(super) struct SourceArtifacts<'source> {
    pub(super) selected: &'source omega_regalloc::ValidatedPressureRematerialization,
    pub(super) machine: &'source crate::StagedOptimizedPostAllocationMachinePlan,
    pub(super) physical: &'source omega_register_model::ValidatedPhysicalRegisterModel,
    pub(super) encoding: &'source crate::StagedOptimizedSelectedFormEncoding,
    pub(super) layout: &'source crate::StagedOptimizedResolvedSelectedFormLayout,
}

pub(super) fn artifacts(
    source: &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) -> Result<
    SourceArtifacts<'_>,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
> {
    let rematerialization = source.pre_layout().source();
    let selected_stage = rematerialization
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selections = selected_stage.optimized_target().optimized().selections();
    let allocation_recovery = selections.for_phase(OptimizationExecutionPhase::AllocationRecovery);
    if !selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .is_empty()
        || allocation_recovery.as_slice()
            != [omega_optimization_core::Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1]
        || !selections
            .for_phase(OptimizationExecutionPhase::PostAllocationMachine)
            .is_empty()
        || !selections
            .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
            .is_empty()
    {
        return Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::LaterPhaseSelected,
        );
    }
    Ok(SourceArtifacts {
        selected: rematerialization.rematerialization(),
        machine: source.pre_layout().machine(),
        physical: selected_stage.register_environment().physical(),
        encoding: source.pre_layout().encoding(),
        layout: source.layout(),
    })
}
