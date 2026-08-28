use omega_machine_optimizer::{
    TerminalPostAllocationMachineError, TerminalPostAllocationMachineIdentity,
    ValidatedTerminalPostAllocationMachinePlan, analyze_terminal_post_allocation_machine_plan,
    validate_terminal_post_allocation_machine_plan,
};

use crate::{
    OptimizedActiveResidentRematerializationError, OptimizedMachineEffectPipelineError,
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedPostLiteralFoldHomeCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt, StagedOptimizedMachineEffects,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering, stage_optimized_machine_effects,
    stage_optimized_machine_effects_after_active_resident_rematerialization,
    stage_optimized_machine_effects_after_fixed_view_copies,
    stage_optimized_machine_effects_after_literal_folds,
    stage_optimized_machine_effects_after_selected_lowering,
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

/// Home-aware machine facts joined only through independently replayed source
/// custody. This remains non-emission and non-publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachinePlan {
    effects: StagedOptimizedMachineEffects,
    machine: ValidatedTerminalPostAllocationMachinePlan,
    custody: StagedOptimizedPostAllocationMachineCustodyReceipt,
}

impl StagedOptimizedPostAllocationMachinePlan {
    pub const fn effects(&self) -> &StagedOptimizedMachineEffects {
        &self.effects
    }

    pub const fn machine(&self) -> &ValidatedTerminalPostAllocationMachinePlan {
        &self.machine
    }

    pub const fn custody(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachineCustodyReceipt {
    source: StagedOptimizedPostAllocationMachineSourceCustodyReceipt,
    effects: omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity,
    machine: TerminalPostAllocationMachineIdentity,
    instruction_count: usize,
    operand_count: usize,
    unit_action_count: usize,
}

impl StagedOptimizedPostAllocationMachineCustodyReceipt {
    pub const fn source(&self) -> &StagedOptimizedPostAllocationMachineSourceCustodyReceipt {
        &self.source
    }
    pub const fn effects(
        &self,
    ) -> omega_machine_optimizer::TerminalPreAllocationMachineEffectIdentity {
        self.effects
    }
    pub const fn machine(&self) -> TerminalPostAllocationMachineIdentity {
        self.machine
    }
    pub const fn instruction_count(&self) -> usize {
        self.instruction_count
    }
    pub const fn operand_count(&self) -> usize {
        self.operand_count
    }
    pub const fn unit_action_count(&self) -> usize {
        self.unit_action_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagedOptimizedPostAllocationMachineSourceCustodyReceipt {
    RegisterHomes(StagedOptimizedRegisterHomeCustodyReceipt),
    FixedViewCopies(StagedOptimizedPostCopyRegisterHomeCustodyReceipt),
    LiteralFolds(StagedOptimizedPostLiteralFoldHomeCustodyReceipt),
    SelectedLowering(StagedOptimizedPostSelectedLoweringHomeCustodyReceipt),
    ActiveResidentRematerialization(StagedOptimizedActiveResidentRematerializationCustodyReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostAllocationMachinePipelineError {
    RegisterHomes(OptimizedRegisterHomeCustodyError),
    FixedViewCopies(OptimizedPostCopyRegisterHomeCustodyError),
    LiteralFolds(OptimizedPostLiteralFoldHomeCustodyError),
    SelectedLowering(OptimizedPostSelectedLoweringHomeCustodyError),
    ActiveResidentRematerialization(OptimizedActiveResidentRematerializationError),
    MachineEffects(OptimizedMachineEffectPipelineError),
    PostAllocation(TerminalPostAllocationMachineError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedPostAllocationMachinePipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-allocation machine staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostAllocationMachinePipelineError {}

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
    let machine = analyze_terminal_post_allocation_machine_plan(
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
    Ok(staged(
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
    let machine = analyze_terminal_post_allocation_machine_plan(
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
    Ok(staged(
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
    let machine = analyze_terminal_post_allocation_machine_plan(
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
    Ok(staged(
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
        Some(step) => analyze_terminal_post_allocation_machine_plan(
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
        None => analyze_terminal_post_allocation_machine_plan(
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
    Ok(staged(
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
    let machine = analyze_terminal_post_allocation_machine_plan(
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
    Ok(staged(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::ActiveResidentRematerialization(
            source_receipt,
        ),
        effects,
        machine,
    ))
}

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
    validate_optimized_machine_effect_custody(selected_stage, staged.effects.effects())
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
    Ok(custody(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::RegisterHomes(source_receipt),
        staged.effects.effects(),
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
        staged.effects.effects(),
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
    Ok(custody(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::FixedViewCopies(source_receipt),
        staged.effects.effects(),
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
    validate_optimized_machine_effect_custody_after_literal_folds(folds, staged.effects.effects())
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
    Ok(custody(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::LiteralFolds(source_receipt),
        staged.effects.effects(),
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
        staged.effects.effects(),
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
    Ok(custody(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::SelectedLowering(source_receipt),
        staged.effects.effects(),
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
            staged.effects.effects(),
        )
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    if &effects_receipt != staged.effects.custody() {
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
    let receipt = custody(
        StagedOptimizedPostAllocationMachineSourceCustodyReceipt::ActiveResidentRematerialization(
            source_receipt,
        ),
        staged.effects.effects(),
        &machine,
    );
    if &receipt != staged.custody() {
        return Err(OptimizedPostAllocationMachinePipelineError::ReceiptMismatch);
    }
    Ok(receipt)
}

#[allow(clippy::too_many_arguments)]
fn replay<S: omega_regalloc::ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    ranges: &omega_regalloc::ValidatedTerminalLiveRanges,
    legality: &omega_regalloc::ValidatedTerminalAllocationLegality,
    homes: &omega_regalloc::ValidatedTerminalRegisterHomes,
    manifest: &omega_regalloc::ValidatedPostAllocationOptimizationManifest,
    environment: &crate::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedTerminalPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError>
{
    let replayed = validate_terminal_post_allocation_machine_plan(
        selected,
        staged.effects.effects(),
        ranges,
        legality,
        homes,
        manifest,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        staged.machine.plan().clone(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    if &replayed != staged.machine() {
        return Err(OptimizedPostAllocationMachinePipelineError::ReceiptMismatch);
    }
    Ok(replayed)
}

fn staged(
    source: StagedOptimizedPostAllocationMachineSourceCustodyReceipt,
    effects: StagedOptimizedMachineEffects,
    machine: ValidatedTerminalPostAllocationMachinePlan,
) -> StagedOptimizedPostAllocationMachinePlan {
    let custody = custody(source, effects.effects(), &machine);
    StagedOptimizedPostAllocationMachinePlan {
        effects,
        machine,
        custody,
    }
}

fn custody(
    source: StagedOptimizedPostAllocationMachineSourceCustodyReceipt,
    effects: &omega_machine_optimizer::ValidatedTerminalPreAllocationMachineEffects,
    machine: &ValidatedTerminalPostAllocationMachinePlan,
) -> StagedOptimizedPostAllocationMachineCustodyReceipt {
    StagedOptimizedPostAllocationMachineCustodyReceipt {
        source,
        effects: effects.receipt().identity(),
        machine: machine.receipt().identity(),
        instruction_count: machine.receipt().instruction_count(),
        operand_count: machine.receipt().operand_count(),
        unit_action_count: machine.receipt().unit_action_count(),
    }
}
