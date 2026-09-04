use omega_machine_optimizer::{
    ValidatedPostAllocationMachinePlan, validate_post_allocation_machine_plan,
};

use omega_allocation_legality_to_active_resident_rematerialization::{
    StagedOptimizedActiveResidentRematerialization,
    validate_optimized_active_resident_rematerialization,
};
use omega_allocation_legality_to_register_homes::{
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_register_home_custody,
};
use omega_literal_folds_to_register_homes::{
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
};
use omega_selected_instructions_to_machine_effects::validate_machine_effects;
use omega_target_to_register_environment::ValidatedTargetRegisterEnvironment;

use super::{
    OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt, post_allocation_machine_custody,
};

pub fn validate_optimized_post_allocation_machine_plan_custody(
    source: &StagedOptimizedRegisterHomes,
    staged: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineCustodyReceipt,
    OptimizedPostAllocationMachinePipelineError,
> {
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
    let environment = selected_stage.register_environment();
    let machine = replay(
        selected_stage.selected(),
        staged,
        source.legality_stage().live_range_stage().ranges(),
        source.legality_stage().legality(),
        source.homes(),
        source.post_allocation_manifest(),
        environment,
    )?;
    Ok(post_allocation_machine_custody(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::RegisterHomes(source_receipt),
        staged.effects(),
        &machine,
    ))
}

pub fn validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody(
    source: &StagedOptimizedRegisterHomesAfterFixedViewCopies,
    staged: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineCustodyReceipt,
    OptimizedPostAllocationMachinePipelineError,
> {
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
    let machine = replay(
        copies.copies(),
        staged,
        source.reanalysis_stage().ranges(),
        source.reanalysis_stage().legality(),
        source.homes(),
        source.post_allocation_manifest(),
        selected_stage.register_environment(),
    )?;
    Ok(post_allocation_machine_custody(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::FixedViewCopies(source_receipt),
        staged.effects(),
        &machine,
    ))
}

pub fn validate_optimized_post_allocation_machine_plan_after_literal_fold_custody(
    source: &StagedOptimizedRegisterHomesAfterLiteralFolds,
    staged: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineCustodyReceipt,
    OptimizedPostAllocationMachinePipelineError,
> {
    let source_receipt = validate_optimized_register_home_after_literal_fold_custody(source)
        .map_err(OptimizedPostAllocationMachinePipelineError::LiteralFolds)?;
    let folds = source.fold_stage();
    let selected_stage = folds
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let machine = replay(
        folds.final_step().fold(),
        staged,
        folds.final_step().ranges(),
        folds.final_step().legality(),
        source.homes(),
        source.post_allocation_manifest(),
        selected_stage.register_environment(),
    )?;
    Ok(post_allocation_machine_custody(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::LiteralFolds(source_receipt),
        staged.effects(),
        &machine,
    ))
}

pub fn validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    staged: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineCustodyReceipt,
    OptimizedPostAllocationMachinePipelineError,
> {
    let source_receipt = validate_optimized_register_home_after_selected_lowering_custody(source)
        .map_err(OptimizedPostAllocationMachinePipelineError::SelectedLowering)?;
    let run = source.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let environment = selected_stage.register_environment();
    let machine = match run.steps().last() {
        Some(step) => replay(
            step.fold(),
            staged,
            step.ranges(),
            step.legality(),
            source.homes(),
            source.post_allocation_manifest(),
            environment,
        ),
        None => replay(
            selected_stage.selected(),
            staged,
            run.source_legality_stage().live_range_stage().ranges(),
            run.source_legality_stage().legality(),
            source.homes(),
            source.post_allocation_manifest(),
            environment,
        ),
    }?;
    Ok(post_allocation_machine_custody(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::SelectedLowering(source_receipt),
        staged.effects(),
        &machine,
    ))
}

pub fn validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(
    source: &StagedOptimizedActiveResidentRematerialization,
    staged: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineCustodyReceipt,
    OptimizedPostAllocationMachinePipelineError,
> {
    let source_receipt = validate_optimized_active_resident_rematerialization(source)
        .map_err(OptimizedPostAllocationMachinePipelineError::ActiveResidentRematerialization)?;
    let selected_stage = source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let machine = replay(
        source.rematerialization(),
        staged,
        source.ranges(),
        source.legality(),
        source.homes(),
        source.post_allocation_manifest(),
        selected_stage.register_environment(),
    )?;
    let receipt = post_allocation_machine_custody(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::ActiveResidentRematerialization(
            source_receipt,
        ),
        staged.effects(),
        &machine,
    );
    if &receipt != staged.custody() {
        return Err(OptimizedPostAllocationMachinePipelineError::ReceiptMismatch);
    }
    Ok(receipt)
}

#[allow(clippy::too_many_arguments)]
fn replay<S: omega_regalloc::ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    ranges: &omega_regalloc::ValidatedLiveRanges,
    legality: &omega_regalloc::ValidatedAllocationLegality,
    homes: &omega_regalloc::ValidatedRegisterHomes,
    manifest: &omega_regalloc::ValidatedPostAllocationOptimizationManifest,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    validate_machine_effects(selected, environment, staged.effects())
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    let replayed = validate_post_allocation_machine_plan(
        selected,
        staged.effects(),
        ranges,
        legality,
        homes,
        manifest,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        staged.machine().plan().clone(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    if &replayed != staged.machine() {
        return Err(OptimizedPostAllocationMachinePipelineError::ReceiptMismatch);
    }
    Ok(replayed)
}
