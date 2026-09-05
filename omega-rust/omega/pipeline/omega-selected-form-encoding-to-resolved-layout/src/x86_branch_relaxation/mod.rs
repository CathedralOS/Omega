//! Optimizer module role: executable entrance. Explicit x86 conditional-branch relaxation and independent replay.

mod catalog;
mod compute;
mod error;
mod identity;
mod model;
mod validation;

pub use catalog::x86_rel8_selected;
pub use catalog::{
    FUNCTION_RELATIVE_LAYOUT_RULE_CATALOG, FunctionRelativeLayoutCatalogError,
    FunctionRelativeLayoutRuleCatalogEntry, ORDERED_FUNCTION_RELATIVE_LAYOUT_RULES,
};
pub use error::*;
pub use model::*;

use omega_optimization_core::OptimizationWorkBudget;
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedResolvedSelectedFormLayout, validate_optimized_resolved_selected_form_layout,
};
use compute::{compute_relaxation, replay_relaxation};
use omega_post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use omega_register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use validation::{compare_replayed_evidence, validate_roots};

pub fn stage_optimized_x86_branch_relaxation<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    source: &StagedOptimizedResolvedSelectedFormLayout,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    validate_optimized_resolved_selected_form_layout(
        selected, machine, physical, pre_layout, source,
    )
    .map_err(OptimizedX86BranchRelaxationError::Source)?;
    validate_roots(source, physical)?;
    let artifact = compute_relaxation(source, physical, budget)?;
    validate_optimized_x86_branch_relaxation(
        selected, machine, physical, pre_layout, source, &artifact,
    )?;
    Ok(artifact)
}

/// Independent replay does not call the production fixed-point driver. It
/// reconstructs the ordered scan, each shrink, every dense offset, the terminal
/// no-change sweep, work usage, revisions, and final receipt.
pub fn validate_optimized_x86_branch_relaxation<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    source: &StagedOptimizedResolvedSelectedFormLayout,
    artifact: &StagedOptimizedX86BranchRelaxation,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    validate_optimized_resolved_selected_form_layout(
        selected, machine, physical, pre_layout, source,
    )
    .map_err(OptimizedX86BranchRelaxationError::Source)?;
    validate_roots(source, physical)?;
    if artifact.source != source.identity()
        || artifact.selected != source.selected()
        || artifact.machine != source.machine()
        || artifact.pre_layout != source.pre_layout()
        || artifact.target != source.target()
        || artifact.policy != X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1
        || !artifact.usage.within(artifact.budget)
    {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    let replayed = replay_relaxation(source, physical, artifact.budget)?;
    compare_replayed_evidence(artifact, &replayed)?;
    if artifact != &replayed {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    Ok(())
}
