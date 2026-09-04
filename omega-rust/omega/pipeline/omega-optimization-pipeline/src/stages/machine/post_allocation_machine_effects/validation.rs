use omega_machine_optimizer::{
    ValidatedPostAllocationMachinePlan, validate_post_allocation_machine_plan,
};

use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    validate_optimized_active_resident_rematerialization,
    validate_optimized_machine_effect_custody,
    validate_optimized_machine_effect_custody_after_active_resident_rematerialization,
    validate_optimized_machine_effect_custody_after_fixed_view_copies,
    validate_optimized_machine_effect_custody_after_literal_folds,
    validate_optimized_machine_effect_custody_after_selected_lowering,
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
    validate_optimized_register_home_custody,
};

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
    validate_optimized_machine_effect_custody(selected_stage, staged.effects().effects())
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
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
        staged.effects().effects(),
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
    validate_optimized_machine_effect_custody_after_fixed_view_copies(
        copies,
        staged.effects().effects(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
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
        staged.effects().effects(),
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
    validate_optimized_machine_effect_custody_after_literal_folds(
        folds,
        staged.effects().effects(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
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
        staged.effects().effects(),
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
    validate_optimized_machine_effect_custody_after_selected_lowering(
        run,
        staged.effects().effects(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
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
        staged.effects().effects(),
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
    let effects_receipt =
        validate_optimized_machine_effect_custody_after_active_resident_rematerialization(
            source,
            staged.effects().effects(),
        )
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    if &effects_receipt != staged.effects().custody() {
        return Err(OptimizedPostAllocationMachinePipelineError::ReceiptMismatch);
    }
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
        staged.effects().effects(),
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
    environment: &crate::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let replayed = validate_post_allocation_machine_plan(
        selected,
        staged.effects().effects(),
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
