use omega_machine_optimizer::{
    Aarch64CbnzFusionError, Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationError,
    Aarch64MovnMaterializationIdentity, ValidatedAarch64CbnzFusion,
    ValidatedAarch64MovnMaterialization, optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz,
    optimize_aarch64_materialize_i64_with_shortest_movn_seed, validate_aarch64_cbnz_fusion,
    validate_aarch64_movn_materialization,
};
use omega_optimization_core::{
    Optimization, OptimizationSelectionIdentity, OptimizationSelections, OptimizationWorkBudget,
};
use omega_regalloc::{ValidatedLiveness, ValidatedSelectedAnalysis};
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterSelectedLowering,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
};

/// Custody-preserving result of the exact named AArch64 post-allocation
/// compare/branch fusion. The raw optimizer artifact is never sufficient on
/// its own: this receipt also binds it to the complete source-visible suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedAarch64CbnzFusion {
    fusion: ValidatedAarch64CbnzFusion,
    custody: StagedOptimizedAarch64CbnzFusionCustodyReceipt,
}

impl StagedOptimizedAarch64CbnzFusion {
    pub const fn fusion(&self) -> &ValidatedAarch64CbnzFusion {
        &self.fusion
    }

    pub const fn custody(&self) -> StagedOptimizedAarch64CbnzFusionCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedAarch64CbnzFusionCustodyReceipt {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: omega_machine_optimizer::PostAllocationMachineIdentity,
    fusion: Aarch64CbnzFusionIdentity,
    action_count: usize,
}

impl StagedOptimizedAarch64CbnzFusionCustodyReceipt {
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }

    pub const fn source(self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.source
    }

    pub const fn fusion(self) -> Aarch64CbnzFusionIdentity {
        self.fusion
    }

    pub const fn action_count(self) -> usize {
        self.action_count
    }
}

/// Custody-preserving result of the exact named AArch64 shortest-MOVN
/// materialization selection. It owns no encoded bytes, layout, or publication
/// authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedAarch64MovnMaterialization {
    materialization: ValidatedAarch64MovnMaterialization,
    custody: StagedOptimizedAarch64MovnMaterializationCustodyReceipt,
}

impl StagedOptimizedAarch64MovnMaterialization {
    pub const fn materialization(&self) -> &ValidatedAarch64MovnMaterialization {
        &self.materialization
    }

    pub const fn custody(&self) -> StagedOptimizedAarch64MovnMaterializationCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedAarch64MovnMaterializationCustodyReceipt {
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: omega_machine_optimizer::PostAllocationMachineIdentity,
    materialization: Aarch64MovnMaterializationIdentity,
    action_count: usize,
    baseline_words: u64,
    selected_words: u64,
}

impl StagedOptimizedAarch64MovnMaterializationCustodyReceipt {
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }

    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }

    pub const fn source(self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.source
    }

    pub const fn materialization(self) -> Aarch64MovnMaterializationIdentity {
        self.materialization
    }

    pub const fn action_count(self) -> usize {
        self.action_count
    }

    pub const fn baseline_words(self) -> u64 {
        self.baseline_words
    }

    pub const fn selected_words(self) -> u64 {
        self.selected_words
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostAllocationMachineOptimizationError {
    Source(OptimizedPostAllocationMachinePipelineError),
    MissingPostAllocationMachineOptimization,
    UnsupportedPostAllocationMachineOptimization(Optimization),
    Fusion(Aarch64CbnzFusionError),
    MovnMaterialization(Aarch64MovnMaterializationError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedPostAllocationMachineOptimizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-allocation machine transformation failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostAllocationMachineOptimizationError {}

pub fn stage_optimized_aarch64_cbnz_fusion(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<StagedOptimizedAarch64CbnzFusion, OptimizedPostAllocationMachineOptimizationError> {
    validate_optimized_post_allocation_machine_plan_custody(source, machine)
        .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let ranges = source.legality_stage().live_range_stage();
    let selected_stage = ranges.liveness_stage().selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    stage_with_inputs(
        selected_stage.selected(),
        ranges.liveness_stage().liveness(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
    )
}

pub fn validate_optimized_aarch64_cbnz_fusion_custody(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64CbnzFusion,
) -> Result<
    StagedOptimizedAarch64CbnzFusionCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_custody(source, machine)
        .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let ranges = source.legality_stage().live_range_stage();
    let selected_stage = ranges.liveness_stage().selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    validate_with_inputs(
        selected_stage.selected(),
        ranges.liveness_stage().liveness(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
        staged,
    )
}

pub fn stage_optimized_aarch64_cbnz_fusion_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<StagedOptimizedAarch64CbnzFusion, OptimizedPostAllocationMachineOptimizationError> {
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        source, machine,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let run = source.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    match run.steps().last() {
        Some(step) => stage_with_inputs(
            step.fold(),
            step.liveness(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
        ),
        None => stage_with_inputs(
            selected_stage.selected(),
            run.source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .liveness(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
        ),
    }
}

pub fn validate_optimized_aarch64_cbnz_fusion_after_selected_lowering_custody(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64CbnzFusion,
) -> Result<
    StagedOptimizedAarch64CbnzFusionCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        source, machine,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let run = source.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    match run.steps().last() {
        Some(step) => validate_with_inputs(
            step.fold(),
            step.liveness(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            staged,
        ),
        None => validate_with_inputs(
            selected_stage.selected(),
            run.source_legality_stage()
                .live_range_stage()
                .liveness_stage()
                .liveness(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            staged,
        ),
    }
}

fn stage_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedAarch64CbnzFusion, OptimizedPostAllocationMachineOptimizationError> {
    let phase = post_allocation_machine_contract(
        selections,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
    )?;
    let fusion = optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz(
        selected,
        liveness,
        machine.machine(),
        physical,
        budget,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Fusion)?;
    let custody = custody_receipt(selections, &phase, &fusion);
    Ok(StagedOptimizedAarch64CbnzFusion { fusion, custody })
}

#[allow(clippy::too_many_arguments)]
fn validate_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
    staged: &StagedOptimizedAarch64CbnzFusion,
) -> Result<
    StagedOptimizedAarch64CbnzFusionCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = post_allocation_machine_contract(
        selections,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
    )?;
    if staged.fusion.plan().budget != budget {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let replayed = validate_aarch64_cbnz_fusion(
        selected,
        liveness,
        machine.machine(),
        physical,
        staged.fusion.plan().clone(),
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Fusion)?;
    if replayed.receipt() != staged.fusion.receipt() {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let custody = custody_receipt(selections, &phase, &replayed);
    if custody != staged.custody {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn post_allocation_machine_contract(
    selections: &OptimizationSelections,
    expected: Optimization,
) -> Result<OptimizationSelections, OptimizedPostAllocationMachineOptimizationError> {
    let phase = selections
        .for_phase(omega_optimization_core::OptimizationExecutionPhase::PostAllocationMachine);
    match phase.as_slice() {
        [selected] if *selected == expected => Ok(phase),
        [] => Err(
            OptimizedPostAllocationMachineOptimizationError::MissingPostAllocationMachineOptimization,
        ),
        selections => Err(
            OptimizedPostAllocationMachineOptimizationError::UnsupportedPostAllocationMachineOptimization(
                selections[0],
            ),
        ),
    }
}

pub fn stage_optimized_aarch64_movn_materialization(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64MovnMaterialization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_custody(source, machine)
        .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let selected_stage = source
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    stage_movn_with_inputs(
        selected_stage.selected(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
    )
}

pub fn validate_optimized_aarch64_movn_materialization_custody(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64MovnMaterialization,
) -> Result<
    StagedOptimizedAarch64MovnMaterializationCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_custody(source, machine)
        .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let selected_stage = source
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    validate_movn_with_inputs(
        selected_stage.selected(),
        machine,
        selected_stage.register_environment().physical(),
        optimized.selections(),
        optimized.budget_per_pass(),
        staged,
    )
}

pub fn stage_optimized_aarch64_movn_materialization_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64MovnMaterialization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        source, machine,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let run = source.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    match run.steps().last() {
        Some(step) => stage_movn_with_inputs(
            step.fold(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
        ),
        None => stage_movn_with_inputs(
            selected_stage.selected(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
        ),
    }
}

pub fn validate_optimized_aarch64_movn_materialization_after_selected_lowering_custody(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64MovnMaterialization,
) -> Result<
    StagedOptimizedAarch64MovnMaterializationCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody(
        source, machine,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    let run = source.selected_lowering_run();
    let selected_stage = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    match run.steps().last() {
        Some(step) => validate_movn_with_inputs(
            step.fold(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            staged,
        ),
        None => validate_movn_with_inputs(
            selected_stage.selected(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            staged,
        ),
    }
}

fn stage_movn_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedAarch64MovnMaterialization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = post_allocation_machine_contract(
        selections,
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    )?;
    let materialization = optimize_aarch64_materialize_i64_with_shortest_movn_seed(
        selected,
        machine.machine(),
        physical,
        budget,
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::MovnMaterialization)?;
    let custody = movn_custody_receipt(selections, &phase, &materialization);
    Ok(StagedOptimizedAarch64MovnMaterialization {
        materialization,
        custody,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_movn_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
    staged: &StagedOptimizedAarch64MovnMaterialization,
) -> Result<
    StagedOptimizedAarch64MovnMaterializationCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = post_allocation_machine_contract(
        selections,
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    )?;
    if staged.materialization.plan().budget != budget {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let replayed = validate_aarch64_movn_materialization(
        selected,
        machine.machine(),
        physical,
        staged.materialization.plan().clone(),
    )
    .map_err(OptimizedPostAllocationMachineOptimizationError::MovnMaterialization)?;
    if replayed.receipt() != staged.materialization.receipt() {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let custody = movn_custody_receipt(selections, &phase, &replayed);
    if custody != staged.custody {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn custody_receipt(
    selections: &OptimizationSelections,
    phase: &OptimizationSelections,
    fusion: &ValidatedAarch64CbnzFusion,
) -> StagedOptimizedAarch64CbnzFusionCustodyReceipt {
    let receipt = fusion.receipt();
    StagedOptimizedAarch64CbnzFusionCustodyReceipt {
        selections: selections.identity(),
        post_allocation_machine_selections: phase.identity(),
        source: receipt.source(),
        fusion: receipt.identity(),
        action_count: receipt.action_count(),
    }
}

fn movn_custody_receipt(
    selections: &OptimizationSelections,
    phase: &OptimizationSelections,
    materialization: &ValidatedAarch64MovnMaterialization,
) -> StagedOptimizedAarch64MovnMaterializationCustodyReceipt {
    let receipt = materialization.receipt();
    StagedOptimizedAarch64MovnMaterializationCustodyReceipt {
        selections: selections.identity(),
        post_allocation_machine_selections: phase.identity(),
        source: receipt.source(),
        materialization: receipt.identity(),
        action_count: receipt.action_count(),
        baseline_words: receipt.baseline_words(),
        selected_words: receipt.selected_words(),
    }
}
