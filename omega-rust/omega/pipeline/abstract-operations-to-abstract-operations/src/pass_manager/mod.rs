//! Optimizer module role: stage group. Deterministic execution of exact selected Psi optimization passes.
//!
//! The run and replay entries live in `abstract_optimization.rs`; they open a
//! [`VerifiedPsiOptimizationSession`] and hand it to `run_registries` here.
//! [`model`] owns the run carrier and closed error surface, [`execution`] owns
//! candidate dispatch and external-decision replay, and [`accounting`] owns
//! convergence and manifests.

mod accounting;
mod baseline;
mod execution;
mod external_policy;
mod model;

use optimization_core::TargetCostModelIdentity;

pub(crate) use execution::{run_registries, run_registries_with_external_decisions};
pub use external_policy::validate_external_decision_recording;
pub use model::{
    CandidateContractAxis, ExternalDecisionContextAxis, ExternalDecisionReplayError,
    OptimizationRun, OptimizationRunError, OptimizationRunUsage, PsiOptimizationCommit,
    PsiValidatedCandidateDeclaration, VerifiedPsiOptimizationSession,
};

/// Identity of the deterministic structural cost policy used by every current
/// target-neutral Psi pass.
pub fn baseline_psi_cost_model_identity() -> TargetCostModelIdentity {
    TargetCostModelIdentity::from_canonical_bytes(b"omega.psi-baseline-structural-cost-model.v1")
}

#[cfg(test)]
use accounting::register_revision;
#[cfg(test)]
use execution::{run_unit, run_unit_inner};
#[cfg(test)]
use external_policy::ExternalDecisionReplayCursor;

#[cfg(test)]
mod tests;
