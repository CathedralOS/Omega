//! Close the admitted abstract program and prepare its target execution plan.
use crate::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
    VerifiedNativeArtifactInput, abstract_optimization, target_lowering,
};
use optimization_core::{OptimizationWorkBudget, PostTerminalOptimizationSelections};
use std::sync::atomic::{AtomicU64, Ordering};

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

pub struct NativeBodyPlan {
    pub selection: legalized_operations::PreparedInstructionSelection,
    pub facts: verified_optimization_input::publication::SemanticEmissionFacts,
    pub private_references: Vec<function_identity::MachineFunctionIdentity>,
    pub original_for_audit: Option<std::sync::Arc<ValidatedOptimizedTargetOperations>>,
}

impl NativeBodyPlan {
    pub fn execution_plan(&self) -> &legalized_operations::PreparedExecutionPlan {
        &self.selection.plan
    }
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
    let (program, facts) = optimized.into_lowering_parts();
    let target = target_lowering::lower_optimized_to_target_operations(program, facts, target)
        .map_err(NativeBodyPlanningError::Target)?;
    let optimized = target.optimized();
    let selection = legalized_operations::PreparedInstructionSelection {
        plan: target.shared_execution_plan(),
        optimization: optimized.identity_bundle().identity(),
        projection: optimized.projection(),
        manifest: optimized.pre_physical_manifest().identity,
        selections: optimized.selections().clone(),
        budget_per_pass: optimized.budget_per_pass(),
    };
    Ok(NativeBodyPlan {
        selection,
        facts: crate::emission_facts::project(&target),
        private_references: target.retained_private_references().to_vec(),
        original_for_audit: retain_original.then(|| std::sync::Arc::new(target)),
    })
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
    let (program, facts) = optimized.into_lowering_parts();
    target_lowering::capture_runtime_borrowed_storage_layouts(&program, &facts, target)
        .map_err(NativeBodyPlanningError::Target)
}
