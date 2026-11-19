//! Close the admitted abstract program and prepare its target execution plan.
use artifact_lowering::VerifiedNativeArtifactInput;
use optimization_core::{OptimizationWorkBudget, PostTerminalOptimizationSelections};
use std::sync::atomic::{AtomicU64, Ordering};
use target_lowering::{NativeBodyPlan, OptimizedTargetLoweringRequest};

#[derive(Clone, Copy)]
pub struct NativeBodyObservation<'a> {
    pub abstract_optimization: &'a AtomicU64,
    pub target_lowering: &'a AtomicU64,
}

#[derive(Debug)]
pub enum NativeBodyPlanningError {
    AbstractOptimization(abstract_optimization::AbstractOptimizationError),
    Target(target_lowering::LoweringError),
}

fn optimize(
    input: VerifiedNativeArtifactInput,
    selections: &PostTerminalOptimizationSelections,
    budget: OptimizationWorkBudget,
    entered: &AtomicU64,
) -> Result<abstract_optimization::ValidatedOptimizedAbstractPlan, NativeBodyPlanningError> {
    entered.fetch_add(1, Ordering::Relaxed);
    let selections = selections.selections();
    abstract_optimization::optimize_abstract_operations(
        input.into_optimization_input(),
        selections,
        &selections.project_psi(),
        budget,
    )
    .map_err(NativeBodyPlanningError::AbstractOptimization)
}

pub fn plan_native_body(
    input: VerifiedNativeArtifactInput,
    target: OptimizedTargetLoweringRequest<'_>,
    selections: &PostTerminalOptimizationSelections,
    budget: OptimizationWorkBudget,
    observation: NativeBodyObservation<'_>,
    retain_original: bool,
) -> Result<NativeBodyPlan, NativeBodyPlanningError> {
    let optimized = optimize(input, selections, budget, observation.abstract_optimization)?;
    observation.target_lowering.fetch_add(1, Ordering::Relaxed);
    target_lowering::lower_native_body(optimized.into_lowering_input(), target, retain_original)
        .map_err(NativeBodyPlanningError::Target)
}

/// Runtime-only admission needs the exact storage layout of optimized borrows.
pub fn plan_runtime_storage(
    input: VerifiedNativeArtifactInput,
    target: target::NativeTarget,
    selections: &PostTerminalOptimizationSelections,
    budget: OptimizationWorkBudget,
    entered: &AtomicU64,
) -> Result<runtime_abi::NativeBorrowedStorageLayouts, NativeBodyPlanningError> {
    let optimized = optimize(input, selections, budget, entered)?;
    target_lowering::runtime_storage(&optimized.into_lowering_input(), target)
        .map_err(NativeBodyPlanningError::Target)
}
