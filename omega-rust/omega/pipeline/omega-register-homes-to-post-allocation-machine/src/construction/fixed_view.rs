use omega_allocation_legality_to_register_homes::{
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    validate_optimized_register_home_after_fixed_view_copy_custody,
};
use omega_selected_instructions_to_machine_effects::stage_optimized_machine_effects_after_fixed_view_copies;

use super::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt, analyze_and_seal,
};

pub fn stage_optimized_post_allocation_machine_plan_after_fixed_view_copies(
    source: &StagedOptimizedRegisterHomesAfterFixedViewCopies,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let source_receipt = validate_optimized_register_home_after_fixed_view_copy_custody(
        source.reanalysis_stage(),
        source.homes(),
        source.post_allocation_manifest(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::FixedViewCopies)?;
    let copies = source.reanalysis_stage().transformation_stage();
    let selected_stage = copies
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let effects = stage_optimized_machine_effects_after_fixed_view_copies(copies)
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    analyze_and_seal(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::FixedViewCopies(source_receipt),
        copies.copies(),
        effects,
        source.reanalysis_stage().ranges(),
        source.reanalysis_stage().legality(),
        source.homes(),
        source.post_allocation_manifest(),
        selected_stage.register_environment(),
    )
}
