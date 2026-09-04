use omega_literal_folds_to_register_homes::{
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    validate_optimized_register_home_after_literal_fold_custody,
};

use super::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt, analyze_and_seal,
};

pub fn stage_optimized_post_allocation_machine_plan_after_literal_folds(
    source: &StagedOptimizedRegisterHomesAfterLiteralFolds,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let source_receipt = validate_optimized_register_home_after_literal_fold_custody(source)
        .map_err(OptimizedPostAllocationMachinePipelineError::LiteralFolds)?;
    let folds = source.fold_stage();
    let selected_stage = folds
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    analyze_and_seal(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::LiteralFolds(source_receipt),
        folds.final_step().fold(),
        folds.final_step().ranges(),
        folds.final_step().legality(),
        source.homes(),
        source.post_allocation_manifest(),
        selected_stage.register_environment(),
    )
}
