use omega_machine_optimizer::{
    Aarch64MovnMaterializationIdentity, ValidatedAarch64MovnMaterialization,
    optimize_aarch64_materialize_i64_with_shortest_movn_seed, require_post_allocation_machine_rule,
    validate_aarch64_movn_materialization,
};
use omega_optimization_core::{
    Optimization, OptimizationSelectionIdentity, OptimizationSelections, OptimizationWorkBudget,
};
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
};

use super::OptimizedPostAllocationMachineOptimizationError;

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
    stage_with_inputs(
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
    validate_with_inputs(
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
        Some(step) => stage_with_inputs(
            step.fold(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
        ),
        None => stage_with_inputs(
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
        Some(step) => validate_with_inputs(
            step.fold(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            staged,
        ),
        None => validate_with_inputs(
            selected_stage.selected(),
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
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedAarch64MovnMaterialization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = require_post_allocation_machine_rule(
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
    let custody = custody_receipt(selections, &phase, &materialization);
    Ok(StagedOptimizedAarch64MovnMaterialization {
        materialization,
        custody,
    })
}

fn validate_with_inputs<S: ValidatedSelectedAnalysis>(
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
    let phase = require_post_allocation_machine_rule(
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
    let custody = custody_receipt(selections, &phase, &replayed);
    if custody != staged.custody {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn custody_receipt(
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
