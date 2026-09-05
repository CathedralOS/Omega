//! Retained source projection and the evidence authorizing it.

use std::sync::Arc;

use crate::{
    OptimizationRun, OptimizationRunUsage, PsiOptimizationCommit, PsiValidatedCandidateDeclaration,
};
use abstract_operations::AbstractOperationPlan;
use optimization_core::{BaselineDecisionLog, ExternalDecisionLog};
use optimization_core::{
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationSelections,
};
use optimization_unit::{
    AbstractOptimizationEvidence, PsiOptimizationUnit, PsiTransformationLedger,
};
use optimization_validation::{
    ValidatedOptimizedAbstractPlanProjection, ValidatedPrePhysicalOptimizationManifest,
};
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput;

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
    pub(super) fn new(
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
