//! Retained source projection and the evidence authorizing it.

use omega_abstract_operations::AbstractOperationPlan;
use omega_optimization_core::{
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationSelections,
};
use omega_optimization_policy::{BaselineDecisionLog, ExternalDecisionLog};
use omega_optimization_unit::{PsiOptimizationUnit, PsiTransformationLedger};
use omega_optimization_validation::{
    ValidatedOptimizedAbstractPlanProjection, ValidatedPrePhysicalOptimizationManifest,
};
use omega_psi_optimizer::{
    OptimizationRun, OptimizationRunUsage, PsiOptimizationCommit, PsiValidatedCandidateDeclaration,
};
use omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput;

/// An abstract plan inseparable from independently replayed optimizer custody.
#[derive(Debug)]
pub struct ValidatedOptimizedAbstractPlan {
    run: OptimizationRun,
    plan: AbstractOperationPlan,
    validation: ValidatedOptimizedAbstractPlanProjection,
    pre_physical_manifest: ValidatedPrePhysicalOptimizationManifest,
}

impl ValidatedOptimizedAbstractPlan {
    pub(crate) const fn new(
        run: OptimizationRun,
        plan: AbstractOperationPlan,
        validation: ValidatedOptimizedAbstractPlanProjection,
        pre_physical_manifest: ValidatedPrePhysicalOptimizationManifest,
    ) -> Self {
        Self {
            run,
            plan,
            validation,
            pre_physical_manifest,
        }
    }

    pub const fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }
    pub const fn verified_input(&self) -> &VerifiedPsiOptimizationInput {
        self.run.session().input()
    }
    pub const fn unit(&self) -> &PsiOptimizationUnit {
        self.run.session().unit()
    }
    pub const fn selections(&self) -> &OptimizationSelections {
        self.run.selections()
    }
    pub const fn psi_selections(&self) -> &OptimizationSelections {
        self.run.psi_selections()
    }
    pub const fn budget_per_pass(&self) -> omega_optimization_core::OptimizationWorkBudget {
        self.run.budget_per_pass()
    }
    pub fn commits(&self) -> &[PsiOptimizationCommit] {
        self.run.commits()
    }
    pub fn validated_candidates(&self) -> &[PsiValidatedCandidateDeclaration] {
        self.run.validated_candidates()
    }
    pub const fn usage(&self) -> OptimizationRunUsage {
        self.run.usage()
    }
    pub const fn decisions(&self) -> &BaselineDecisionLog {
        self.run.decisions()
    }
    pub const fn external_decisions(&self) -> &ExternalDecisionLog {
        self.run.external_decisions()
    }
    pub fn pass_manifests(&self) -> &[OptimizationPassManifestRecord] {
        self.run.pass_manifests()
    }
    pub const fn transformation_ledger(&self) -> &PsiTransformationLedger {
        self.run.transformation_ledger()
    }
    pub const fn identity_bundle(&self) -> OptimizationIdentityBundle {
        self.run.identity_bundle()
    }
    pub const fn validation(&self) -> ValidatedOptimizedAbstractPlanProjection {
        self.validation
    }
    pub const fn pre_physical_manifest(&self) -> &ValidatedPrePhysicalOptimizationManifest {
        &self.pre_physical_manifest
    }
}
