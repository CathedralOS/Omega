//! Optimizer module role: executable entrance. MOVN proposal computation and deterministic selection.
//!
//! This join admits source roots, constructs the retained baseline roster,
//! runs the bounded scan-and-commit selector, and seals the resulting plan
//! identity. Recipe arithmetic and footprint qualification remain named,
//! independently reviewable leaves.

mod budget;
mod materialization;
mod recipe;
mod selection;
mod source;

#[cfg(test)]
mod tests;

use omega_optimization_core::OptimizationWorkBudget;
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};

use crate::{
    Aarch64MovnMaterializationError, Aarch64MovnMaterializationIdentity,
    Aarch64MovnMaterializationPlan, Aarch64MovnMaterializationPolicy,
    PostAllocationMachineIdentity, PostAllocationMachinePlan, ValidatedPostAllocationMachinePlan,
    aarch64_movn_materialization_identity,
};

pub(crate) use recipe::{movn_recipe, zero_seed_word_count};

pub(crate) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64MovnMaterializationPlan, Aarch64MovnMaterializationError> {
    compute_from_parts(
        selected.selected_plan(),
        selected.selected_identity(),
        source.plan(),
        source.receipt().identity(),
        physical,
        budget,
    )
}

pub(crate) fn compute_from_parts(
    selected: &SelectedInstructionPlan,
    selected_identity: SelectedInstructionPlanIdentity,
    source_plan: &PostAllocationMachinePlan,
    source_identity: PostAllocationMachineIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64MovnMaterializationPlan, Aarch64MovnMaterializationError> {
    source::validate_roots(
        selected,
        selected_identity,
        source_plan,
        source_identity,
        physical,
    )?;
    let selected_rewrites = selection::select(
        selected,
        selected_identity,
        source_plan,
        source_identity,
        physical,
        budget,
        source::baseline_roster(source_plan),
    )?;
    let output_revision = super::identity::revision_identity(
        source_identity,
        selected_identity,
        source_plan.target,
        physical.identity(),
        &selected_rewrites.functions,
    );
    let mut plan = Aarch64MovnMaterializationPlan {
        identity: Aarch64MovnMaterializationIdentity::from_bytes([0; 32]),
        source: source_identity,
        selected: selected_identity,
        target: source_plan.target,
        physical_register_model: physical.identity(),
        policy:
            Aarch64MovnMaterializationPolicy::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        budget,
        usage: selected_rewrites.usage,
        output_revision,
        attempts: selected_rewrites.attempts,
        actions: selected_rewrites.actions,
        functions: selected_rewrites.functions,
    };
    plan.identity = aarch64_movn_materialization_identity(&plan);
    Ok(plan)
}
