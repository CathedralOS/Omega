use crate::{
    StagedOptimizedRegisterHomes, stage_optimized_machine_effects,
    validate_optimized_register_home_custody,
};

use super::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt, analyze_and_seal,
};

pub fn stage_optimized_post_allocation_machine_plan(
    source: &StagedOptimizedRegisterHomes,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let source_receipt = validate_optimized_register_home_custody(
        source.legality_stage(),
        source.homes(),
        source.post_allocation_manifest(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::RegisterHomes)?;
    let selected_stage = source
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let effects = stage_optimized_machine_effects(selected_stage)
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    analyze_and_seal(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::RegisterHomes(source_receipt),
        selected_stage.selected(),
        effects,
        source.legality_stage().live_range_stage().ranges(),
        source.legality_stage().legality(),
        source.homes(),
        source.post_allocation_manifest(),
        selected_stage.register_environment(),
    )
}
