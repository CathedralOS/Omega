//! The closed set of candidate declarations a Psi optimization run may
//! retain and commit.
//!
//! Every selected pass emits one of these records per independently
//! validated candidate. The shared contract axes — identity, input, rule,
//! required and invalidated analyses, safety class, consumed facts, and
//! predicted cost delta — are what commits, decision manifests, baseline
//! logs, and external-policy rows bind, so a scheduled pass cannot drift
//! between the declaration it proposed and the transformation it recorded.

use super::loop_invariant_scalar_motion::LoopInvariantScalarMotionCandidate;
use super::{
    OptimizationCandidateIdentity, OptimizationFactReference, OptimizationRuleIdentity,
    OptimizationSafetyClass, OptimizationUnitIdentity, ProvenanceRewrite, PsiRewriteCandidate,
};
use crate::PrunedMachineCustody;
use optimization_core::{AnalysisInvalidationSet, AnalysisSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiCandidateDeclaration {
    /// An atomic patch proposed by a catalog rule from analysis products and
    /// validated by `optimization-unit-semantics`.
    Rewrite(PsiRewriteCandidate),
    /// A session-level cyclic-component relocation proposed, validated, and
    /// applied by the loop-invariant scalar-motion boundary. Its custody —
    /// components, member blocks, effects, custody reconstruction — is owned
    /// by the verified session rather than the analysis manager, so the
    /// declaration binds the component and relocation roster directly.
    LoopInvariantScalarMotion(LoopInvariantScalarMotionCandidate),
}

impl PsiCandidateDeclaration {
    pub const fn identity(&self) -> OptimizationCandidateIdentity {
        match self {
            Self::Rewrite(candidate) => candidate.identity(),
            Self::LoopInvariantScalarMotion(candidate) => candidate.identity(),
        }
    }

    pub const fn input(&self) -> OptimizationUnitIdentity {
        match self {
            Self::Rewrite(candidate) => candidate.input(),
            Self::LoopInvariantScalarMotion(candidate) => candidate.input(),
        }
    }

    pub const fn rule(&self) -> OptimizationRuleIdentity {
        match self {
            Self::Rewrite(candidate) => candidate.rule(),
            Self::LoopInvariantScalarMotion(candidate) => candidate.rule(),
        }
    }

    pub const fn required_analyses(&self) -> AnalysisSet {
        match self {
            Self::Rewrite(candidate) => candidate.required_analyses(),
            Self::LoopInvariantScalarMotion(candidate) => candidate.required_analyses(),
        }
    }

    pub const fn invalidated_analyses(&self) -> AnalysisInvalidationSet {
        match self {
            Self::Rewrite(candidate) => candidate.invalidated_analyses(),
            Self::LoopInvariantScalarMotion(candidate) => candidate.invalidated_analyses(),
        }
    }

    pub const fn safety_class(&self) -> OptimizationSafetyClass {
        match self {
            Self::Rewrite(candidate) => candidate.safety_class(),
            Self::LoopInvariantScalarMotion(candidate) => candidate.safety_class(),
        }
    }

    /// Shared optimizer facts the declaration consumed. A relocation
    /// candidate binds its evidence in the roster itself and consumes none.
    pub fn consumed_facts(&self) -> Vec<OptimizationFactReference> {
        match self {
            Self::Rewrite(candidate) => candidate.consumed_facts(),
            Self::LoopInvariantScalarMotion(candidate) => candidate.consumed_facts(),
        }
    }

    pub fn predicted_cost_delta(&self) -> i64 {
        match self {
            Self::Rewrite(candidate) => candidate.predicted_cost_delta(),
            Self::LoopInvariantScalarMotion(candidate) => candidate.predicted_cost_delta(),
        }
    }

    /// Provenance rows the declaration itself proposed. A relocation
    /// candidate proposes no provenance — the validator reconstructs the
    /// rows independently and the commit binds those, so this is empty.
    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        match self {
            Self::Rewrite(candidate) => candidate.provenance(),
            Self::LoopInvariantScalarMotion(_) => &[],
        }
    }

    /// Private-machine custody pruned by the declaration. Relocation moves
    /// no machines, so the custody slice is empty.
    pub fn pruned_machine_custody(&self) -> &[PrunedMachineCustody] {
        match self {
            Self::Rewrite(candidate) => candidate.patch_ref().pruned_machine_custody(),
            Self::LoopInvariantScalarMotion(_) => &[],
        }
    }

    /// The atomic rewrite declaration, when this row is one.
    pub const fn rewrite(&self) -> Option<&PsiRewriteCandidate> {
        match self {
            Self::Rewrite(candidate) => Some(candidate),
            Self::LoopInvariantScalarMotion(_) => None,
        }
    }

    /// The loop-invariant relocation declaration, when this row is one.
    pub const fn loop_invariant_scalar_motion(
        &self,
    ) -> Option<&LoopInvariantScalarMotionCandidate> {
        match self {
            Self::Rewrite(_) => None,
            Self::LoopInvariantScalarMotion(candidate) => Some(candidate),
        }
    }
}
