#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Custody-preserving projection from a completed Psi optimization run to
//! executable abstract operations.
//!
//! This entrance owns the ordered join: exact Psi selection projection,
//! source-shape projection, the sealed projection receipt, and pre-physical
//! manifest publication. The run is this compile's own output: each rewrite
//! was validated when the pass manager committed it, so publication seals it
//! rather than replaying the run. Source mechanics descend into their subtree.

mod error;
mod source;

use crate::OptimizationRun;
use optimization_core::OptimizationExecutionPhase;

use crate::validation::{
    project_pre_physical_optimization_manifest, seal_optimized_abstract_plan_projection,
};

use crate::validation::{
    ValidatedOptimizedAbstractPlanProjection, ValidatedPrePhysicalOptimizationManifest,
};
use crate::{OptimizationRunUsage, PsiOptimizationCommit, PsiValidatedCandidateDeclaration};
use abstract_operations::AbstractOperationPlan;
pub use error::OptimizedAbstractProjectionError;
use optimization_core::{
    BaselineDecisionLog, ExternalDecisionLog, OptimizationIdentityBundle,
    OptimizationPassManifestRecord, OptimizationSelections,
};
use optimization_unit::{
    AbstractOptimizationEvidence, PsiOptimizationUnit, PsiTransformationLedger,
};
use std::sync::Arc;
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput;

pub fn publish_optimization_run(
    run: OptimizationRun,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizedAbstractProjectionError> {
    if run.psi_selections() != &run.selections().for_phase(OptimizationExecutionPhase::Psi) {
        return Err(OptimizedAbstractProjectionError::PsiSelectionProjectionMismatch);
    }
    let plan = source::project_plan(run.session().input().plan(), run.session().unit())?;
    let validation = seal_optimized_abstract_plan_projection(
        run.session().unit(),
        run.selections(),
        run.psi_selections(),
        run.transformation_ledger(),
        run.identity_bundle(),
    );
    let pre_physical_manifest = project_pre_physical_optimization_manifest(
        run.session().input(),
        run.session().unit(),
        run.selections(),
        run.psi_selections(),
        run.budget_per_pass(),
        work_usage(run.usage()),
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

/// An abstract plan inseparable from independently replayed optimizer custody.
#[derive(Debug)]
pub struct ValidatedOptimizedAbstractPlan {
    plan: Arc<AbstractOperationPlan>,
    replay_input: VerifiedPsiOptimizationInput,
    replay_unit: PsiOptimizationUnit,
    evidence: AbstractOptimizationEvidence,
    validation: ValidatedOptimizedAbstractPlanProjection,
    pre_physical_manifest: ValidatedPrePhysicalOptimizationManifest,
}

impl ValidatedOptimizedAbstractPlan {
    fn new(
        run: OptimizationRun,
        plan: AbstractOperationPlan,
        validation: ValidatedOptimizedAbstractPlanProjection,
        pre_physical_manifest: ValidatedPrePhysicalOptimizationManifest,
    ) -> Self {
        let OptimizationRun {
            session,
            selections,
            psi_selections,
            budget_per_pass,
            commits,
            validated_candidates,
            usage,
            decisions,
            external_decisions,
            pass_manifests,
            transformation_ledger,
            identity_bundle,
        } = run;
        // The completed producer and its analysis cache end here. Only explicit
        // replay inputs and immutable evidence accompany the current program.
        let (replay_input, replay_unit) = session.into_parts();
        Self {
            plan: Arc::new(plan),
            replay_input,
            replay_unit,
            evidence: AbstractOptimizationEvidence {
                selections,
                psi_selections,
                budget_per_pass,
                commits,
                validated_candidates,
                usage,
                decisions,
                external_decisions,
                pass_manifests,
                transformation_ledger,
                identity_bundle,
            },
            validation,
            pre_physical_manifest,
        }
    }

    pub fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }
    pub fn shared_program(&self) -> Arc<AbstractOperationPlan> {
        Arc::clone(&self.plan)
    }
    pub const fn evidence(&self) -> &AbstractOptimizationEvidence {
        &self.evidence
    }
    pub const fn verified_input(&self) -> &VerifiedPsiOptimizationInput {
        &self.replay_input
    }
    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.replay_unit
    }
    pub const fn selections(&self) -> &OptimizationSelections {
        &self.evidence.selections
    }
    pub const fn psi_selections(&self) -> &OptimizationSelections {
        &self.evidence.psi_selections
    }
    pub const fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
        self.evidence.budget_per_pass
    }
    pub fn commits(&self) -> &[PsiOptimizationCommit] {
        &self.evidence.commits
    }
    pub fn validated_candidates(&self) -> &[PsiValidatedCandidateDeclaration] {
        &self.evidence.validated_candidates
    }
    pub const fn usage(&self) -> OptimizationRunUsage {
        self.evidence.usage
    }
    pub const fn decisions(&self) -> &BaselineDecisionLog {
        &self.evidence.decisions
    }
    pub const fn external_decisions(&self) -> &ExternalDecisionLog {
        &self.evidence.external_decisions
    }
    pub fn pass_manifests(&self) -> &[OptimizationPassManifestRecord] {
        &self.evidence.pass_manifests
    }
    pub const fn transformation_ledger(&self) -> &PsiTransformationLedger {
        &self.evidence.transformation_ledger
    }
    pub const fn identity_bundle(&self) -> OptimizationIdentityBundle {
        self.evidence.identity_bundle
    }
    pub const fn validation(&self) -> ValidatedOptimizedAbstractPlanProjection {
        self.validation
    }
    pub const fn pre_physical_manifest(&self) -> &ValidatedPrePhysicalOptimizationManifest {
        &self.pre_physical_manifest
    }
}

const fn work_usage(usage: OptimizationRunUsage) -> optimization_core::OptimizationWorkUsage {
    optimization_core::OptimizationWorkUsage {
        rule_evaluations: usage.rule_evaluations,
        candidates: usage.candidates,
        validation_steps: usage.validation_steps,
        commits: usage.commits,
        iterations: usage.iterations,
    }
}
