#![forbid(unsafe_code)]

//! Custody-preserving projection from a completed Psi optimization run to
//! executable abstract operations.
//!
//! This entrance owns the ordered join: exact Psi selection projection,
//! independent run replay, source-shape projection, independent projection
//! validation, and pre-physical manifest publication. Replay and source
//! mechanics descend into their named subtrees.

mod error;
mod model;
mod replay;
mod source;

use omega_optimization_core::OptimizationExecutionPhase;
use omega_optimization_validation::{
    project_pre_physical_optimization_manifest, validate_optimized_abstract_plan_projection,
};
use omega_psi_optimizer::{OptimizationRun, baseline_psi_cost_model_identity};

pub use error::{AppliedDecisionCustodyAxis, OptimizedAbstractProjectionError};
pub use model::ValidatedOptimizedAbstractPlan;

pub fn project_optimization_run(
    run: OptimizationRun,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizedAbstractProjectionError> {
    if run.psi_selections() != &run.selections().for_phase(OptimizationExecutionPhase::Psi) {
        return Err(OptimizedAbstractProjectionError::PsiSelectionProjectionMismatch);
    }
    let replay = replay::validate(&run)?;
    let plan = source::project_plan(run.session().input().plan(), run.session().unit())?;
    let validation = validate_optimized_abstract_plan_projection(
        run.session().input(),
        run.session().unit(),
        &plan,
        run.selections(),
        run.psi_selections(),
        replay.ordered_rule_set,
        baseline_psi_cost_model_identity(),
        run.decisions(),
        run.pass_manifests(),
        run.transformation_ledger(),
        run.identity_bundle(),
    )
    .map_err(OptimizedAbstractProjectionError::IndependentValidation)?;
    let pre_physical_manifest = project_pre_physical_optimization_manifest(
        run.session().input(),
        run.session().unit(),
        run.selections(),
        run.psi_selections(),
        run.budget_per_pass(),
        replay::work_usage(run.usage()),
        run.decisions(),
        run.pass_manifests(),
        run.transformation_ledger(),
        run.identity_bundle(),
        validation,
    )
    .map_err(OptimizedAbstractProjectionError::PrePhysicalManifest)?;
    Ok(ValidatedOptimizedAbstractPlan::new(
        run,
        plan,
        validation,
        pre_physical_manifest,
    ))
}

#[cfg(test)]
mod tests;
