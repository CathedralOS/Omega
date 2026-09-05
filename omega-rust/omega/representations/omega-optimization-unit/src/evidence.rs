//! Immutable optimizer evidence. These records carry no executing session or analysis cache.

use crate::{ProvenanceRewrite, PsiRewriteCandidate, PsiTransformationLedger};
use omega_optimization_core::{
    BaselineDecisionLog, ExternalDecisionLog, OptimizationCandidateIdentity,
    OptimizationIdentityBundle, OptimizationPassIdentity, OptimizationPassManifestRecord,
    OptimizationRuleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkBudget,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationCommit {
    pub rule: OptimizationRuleIdentity,
    pub candidate: OptimizationCandidateIdentity,
    pub validator: OptimizationValidatorIdentity,
    pub input: OptimizationUnitIdentity,
    pub output: OptimizationUnitIdentity,
    pub predicted_cost_delta: i64,
    pub pruned_machines: Vec<crate::PrunedMachineCustody>,
    pub provenance: Vec<ProvenanceRewrite>,
    pub declaration: PsiRewriteCandidate,
}

impl PsiOptimizationCommit {
    pub const fn declaration(&self) -> &PsiRewriteCandidate {
        &self.declaration
    }
}

/// Full immutable declaration retained for every independently validated Psi
/// candidate, whether policy applies or skips it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiValidatedCandidateDeclaration {
    pub pass: OptimizationPassIdentity,
    pub declaration: PsiRewriteCandidate,
    pub validator: OptimizationValidatorIdentity,
}

impl PsiValidatedCandidateDeclaration {
    pub const fn pass(&self) -> OptimizationPassIdentity {
        self.pass
    }

    pub const fn declaration(&self) -> &PsiRewriteCandidate {
        &self.declaration
    }

    pub const fn validator(&self) -> OptimizationValidatorIdentity {
        self.validator
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OptimizationRunUsage {
    pub rule_evaluations: u64,
    pub candidates: u64,
    pub validation_steps: u64,
    pub commits: u64,
    pub iterations: u64,
}

/// The retained history needed to replay a published abstract program.
/// Current program data is not recovered by walking this history.
#[derive(Debug, Clone)]
pub struct AbstractOptimizationEvidence {
    pub selections: OptimizationSelections,
    pub psi_selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
    pub commits: Vec<PsiOptimizationCommit>,
    pub validated_candidates: Vec<PsiValidatedCandidateDeclaration>,
    pub usage: OptimizationRunUsage,
    pub decisions: BaselineDecisionLog,
    pub external_decisions: ExternalDecisionLog,
    pub pass_manifests: Vec<OptimizationPassManifestRecord>,
    pub transformation_ledger: PsiTransformationLedger,
    pub identity_bundle: OptimizationIdentityBundle,
}
