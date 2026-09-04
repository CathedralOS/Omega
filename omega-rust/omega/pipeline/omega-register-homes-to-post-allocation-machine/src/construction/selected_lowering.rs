use omega_literal_folds_to_register_homes::{
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    validate_optimized_register_home_after_selected_lowering_custody,
};

use super::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt, analyze_and_seal,
};

pub fn stage_optimized_post_allocation_machine_plan_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let source_receipt = validate_optimized_register_home_after_selected_lowering_custody(source)
        .map_err(OptimizedPostAllocationMachinePipelineError::SelectedLowering)?;
    let run = source.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let receipt =
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::SelectedLowering(source_receipt);
    match run.steps().last() {
        Some(step) => analyze_and_seal(
            receipt,
            step.fold(),
            step.ranges(),
            step.legality(),
            source.homes(),
            source.post_allocation_manifest(),
            selected_stage.register_environment(),
        ),
        None => analyze_and_seal(
            receipt,
            selected_stage.selected(),
            run.source_legality_stage().live_range_stage().ranges(),
            run.source_legality_stage().legality(),
            source.homes(),
            source.post_allocation_manifest(),
            selected_stage.register_environment(),
        ),
    }
}
