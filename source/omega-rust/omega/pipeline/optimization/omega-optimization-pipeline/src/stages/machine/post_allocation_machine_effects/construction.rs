use omega_machine_optimizer::analyze_post_allocation_machine_plan;

use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering, stage_optimized_machine_effects,
    stage_optimized_machine_effects_after_active_resident_rematerialization,
    stage_optimized_machine_effects_after_fixed_view_copies,
    stage_optimized_machine_effects_after_literal_folds,
    stage_optimized_machine_effects_after_selected_lowering,
    validate_optimized_active_resident_rematerialization,
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
    validate_optimized_register_home_custody,
};

use super::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostAllocationMachineSourceCustodyReceipt, seal_staged_post_allocation_machine,
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
    let environment = selected_stage.register_environment();
    let machine = analyze_post_allocation_machine_plan(
        selected_stage.selected(),
        effects.effects(),
        source.legality_stage().live_range_stage().ranges(),
        source.legality_stage().legality(),
        source.homes(),
        source.post_allocation_manifest(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    Ok(seal_staged_post_allocation_machine(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::RegisterHomes(source_receipt),
        effects,
        machine,
    ))
}

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
    let environment = selected_stage.register_environment();
    let machine = analyze_post_allocation_machine_plan(
        copies.copies(),
        effects.effects(),
        source.reanalysis_stage().ranges(),
        source.reanalysis_stage().legality(),
        source.homes(),
        source.post_allocation_manifest(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    Ok(seal_staged_post_allocation_machine(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::FixedViewCopies(source_receipt),
        effects,
        machine,
    ))
}

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
    let effects = stage_optimized_machine_effects_after_literal_folds(folds)
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    let environment = selected_stage.register_environment();
    let machine = analyze_post_allocation_machine_plan(
        folds.final_step().fold(),
        effects.effects(),
        folds.final_step().ranges(),
        folds.final_step().legality(),
        source.homes(),
        source.post_allocation_manifest(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    Ok(seal_staged_post_allocation_machine(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::LiteralFolds(source_receipt),
        effects,
        machine,
    ))
}

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
    let effects = stage_optimized_machine_effects_after_selected_lowering(run)
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    let environment = selected_stage.register_environment();
    let machine = match run.steps().last() {
        Some(step) => analyze_post_allocation_machine_plan(
            step.fold(),
            effects.effects(),
            step.ranges(),
            step.legality(),
            source.homes(),
            source.post_allocation_manifest(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
        ),
        None => analyze_post_allocation_machine_plan(
            selected_stage.selected(),
            effects.effects(),
            run.source_legality_stage().live_range_stage().ranges(),
            run.source_legality_stage().legality(),
            source.homes(),
            source.post_allocation_manifest(),
            environment.identity(),
            environment.physical(),
            environment.constraints(),
        ),
    }
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    Ok(seal_staged_post_allocation_machine(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::SelectedLowering(source_receipt),
        effects,
        machine,
    ))
}

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
    let environment = selected_stage.register_environment();
    let machine = analyze_post_allocation_machine_plan(
        source.rematerialization(),
        effects.effects(),
        source.ranges(),
        source.legality(),
        source.homes(),
        source.post_allocation_manifest(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    Ok(seal_staged_post_allocation_machine(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::ActiveResidentRematerialization(
            source_receipt,
        ),
        effects,
        machine,
    ))
}
