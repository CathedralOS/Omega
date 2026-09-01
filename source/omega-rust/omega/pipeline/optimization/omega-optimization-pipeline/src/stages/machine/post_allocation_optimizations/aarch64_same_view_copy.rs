use omega_machine_optimizer::{
    Aarch64SameViewCopyElisionIdentity, ValidatedAarch64SameViewCopyElision,
    optimize_aarch64_same_view_copy_i64_before_compare_zero,
    optimize_aarch64_same_view_copy_i64_before_return, require_post_allocation_machine_rule,
    validate_aarch64_same_view_copy_elision,
    validate_aarch64_same_view_copy_i64_before_compare_zero,
};
use omega_optimization_core::{
    Optimization, OptimizationSelectionIdentity, OptimizationSelections, OptimizationWorkBudget,
};
use omega_regalloc::{ValidatedLiveness, ValidatedSelectedAnalysis};
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
};

use super::OptimizedPostAllocationMachineOptimizationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedAarch64SameViewCopyElision {
    elision: ValidatedAarch64SameViewCopyElision,
    custody: StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
}

impl StagedOptimizedAarch64SameViewCopyElision {
    pub const fn elision(&self) -> &ValidatedAarch64SameViewCopyElision {
        &self.elision
    }

    pub const fn custody(&self) -> StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt {
    optimization: Optimization,
    selections: OptimizationSelectionIdentity,
    post_allocation_machine_selections: OptimizationSelectionIdentity,
    source: omega_machine_optimizer::PostAllocationMachineIdentity,
    elision: Aarch64SameViewCopyElisionIdentity,
    action_count: usize,
}

impl StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt {
    pub const fn optimization(self) -> Optimization {
        self.optimization
    }
    pub const fn selections(self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn post_allocation_machine_selections(self) -> OptimizationSelectionIdentity {
        self.post_allocation_machine_selections
    }
    pub const fn source(self) -> omega_machine_optimizer::PostAllocationMachineIdentity {
        self.source
    }
    pub const fn elision(self) -> Aarch64SameViewCopyElisionIdentity {
        self.elision
    }
    pub const fn action_count(self) -> usize {
        self.action_count
    }
}

pub fn stage_optimized_aarch64_same_view_copy_elision(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
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
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
    )
}

pub fn stage_optimized_aarch64_same_view_copy_before_compare_zero_elision(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
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
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1,
    )
}

pub fn validate_optimized_aarch64_same_view_copy_elision_custody(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64SameViewCopyElision,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
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

pub fn stage_optimized_aarch64_same_view_copy_elision_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
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
            step.liveness(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
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
            Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
        ),
    }
}

pub fn stage_optimized_aarch64_same_view_copy_before_compare_zero_elision_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
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
            step.liveness(),
            machine,
            selected_stage.register_environment().physical(),
            optimized.selections(),
            optimized.budget_per_pass(),
            Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1,
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
            Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1,
        ),
    }
}

pub fn validate_optimized_aarch64_same_view_copy_elision_after_selected_lowering_custody(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    staged: &StagedOptimizedAarch64SameViewCopyElision,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
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
    optimization: Optimization,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElision,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = require_post_allocation_machine_rule(
        selections,
        optimization,
        machine.machine().plan().target.architecture,
    )?;
    let elision = match optimization {
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1 => {
            optimize_aarch64_same_view_copy_i64_before_return(
                selected,
                liveness,
                machine.machine(),
                physical,
                budget,
            )
        }
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1 => {
            optimize_aarch64_same_view_copy_i64_before_compare_zero(
                selected,
                liveness,
                machine.machine(),
                physical,
                budget,
            )
        }
        _ => unreachable!("catalog dispatch supplies an exact same-view copy rule"),
    }
    .map_err(OptimizedPostAllocationMachineOptimizationError::SameViewCopyElision)?;
    let custody = custody_receipt(optimization, selections, &phase, &elision);
    Ok(StagedOptimizedAarch64SameViewCopyElision { elision, custody })
}

#[allow(clippy::too_many_arguments)]
fn validate_with_inputs<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
    staged: &StagedOptimizedAarch64SameViewCopyElision,
) -> Result<
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = require_post_allocation_machine_rule(
        selections,
        staged.custody.optimization,
        machine.machine().plan().target.architecture,
    )?;
    if staged.elision.plan().budget != budget {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let replayed = match staged.custody.optimization {
        Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1 => {
            validate_aarch64_same_view_copy_elision(
                selected,
                liveness,
                machine.machine(),
                physical,
                staged.elision.plan().clone(),
            )
        }
        Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1 => {
            validate_aarch64_same_view_copy_i64_before_compare_zero(
                selected,
                liveness,
                machine.machine(),
                physical,
                staged.elision.plan().clone(),
            )
        }
        _ => return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch),
    }
    .map_err(OptimizedPostAllocationMachineOptimizationError::SameViewCopyElision)?;
    if replayed.receipt() != staged.elision.receipt() {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    let custody = custody_receipt(staged.custody.optimization, selections, &phase, &replayed);
    if custody != staged.custody {
        return Err(OptimizedPostAllocationMachineOptimizationError::ReceiptMismatch);
    }
    Ok(custody)
}

fn custody_receipt(
    optimization: Optimization,
    selections: &OptimizationSelections,
    phase: &OptimizationSelections,
    elision: &ValidatedAarch64SameViewCopyElision,
) -> StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt {
    let receipt = elision.receipt();
    StagedOptimizedAarch64SameViewCopyElisionCustodyReceipt {
        optimization,
        selections: selections.identity(),
        post_allocation_machine_selections: phase.identity(),
        source: receipt.source(),
        elision: receipt.identity(),
        action_count: receipt.action_count(),
    }
}
