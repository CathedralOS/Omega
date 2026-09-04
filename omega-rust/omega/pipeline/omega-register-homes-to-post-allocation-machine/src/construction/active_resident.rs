use omega_allocation_legality_to_active_resident_rematerialization::{
    StagedOptimizedActiveResidentRematerialization,
    validate_optimized_active_resident_rematerialization,
};
use omega_selected_instructions_to_machine_effects::stage_optimized_machine_effects_after_active_resident_rematerialization;

use super::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt, analyze_and_seal,
};

pub fn stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
    source: &StagedOptimizedActiveResidentRematerialization,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let source_receipt = validate_optimized_active_resident_rematerialization(source)
        .map_err(OptimizedPostAllocationMachinePipelineError::ActiveResidentRematerialization)?;
    let selected_stage = source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let effects = stage_optimized_machine_effects_after_active_resident_rematerialization(source)
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    analyze_and_seal(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::ActiveResidentRematerialization(
            source_receipt,
        ),
        source.rematerialization(),
        effects,
        source.ranges(),
        source.legality(),
        source.homes(),
        source.post_allocation_manifest(),
        selected_stage.register_environment(),
    )
}
