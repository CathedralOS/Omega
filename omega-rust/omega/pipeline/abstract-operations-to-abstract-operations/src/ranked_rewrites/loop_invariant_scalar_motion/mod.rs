//! Optimizer module role: executable entrance. Exact single-entry loop-invariant scalar-leaf relocation boundary.

use optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};
use optimization_unit::CycleComponentId;
use optimization_unit::{
    EffectLink, NodeLocation, OptimizationNode, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiRealizationSite,
    PsiTransformationLedger, PsiTransformationRecord, ValueDefinitionSite,
    recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, ScalarType, ValueId};

use crate::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
    CountedLoopAnalysisError, VerifiedPsiOptimizationSession,
};

mod apply;
mod model;
mod propose;
mod validate;

use model::candidate_identity;
pub use model::{
    AppliedLoopInvariantScalarMotion, LoopInvariantScalarLeaf, LoopInvariantScalarMotionCandidate,
    LoopInvariantScalarMotionError, LoopInvariantScalarRelocation,
    ValidatedLoopInvariantScalarMotion,
};

/// Propose every single-entry cyclic component that still retains admissible
/// loop-invariant scalar-leaf nodes inside its member blocks. Each candidate
/// relocates the component's complete admissible set into the tail of the
/// component's unique preheader, ahead of the terminator that owns the entry
/// edge and ahead of any already-relocated countdown-certificate constants
/// owned by the dedicated countdown boundary.
pub fn propose_loop_invariant_scalar_motion(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<LoopInvariantScalarMotionCandidate>, LoopInvariantScalarMotionError> {
    propose::all(session, candidate_limit)
}

/// Independently replay the proposal for `candidate`, reconstruct the
/// transformed session, and retain the provenance rows that carry each moved
/// node's source custody into the ledger.
pub fn validate_loop_invariant_scalar_motion(
    session: &VerifiedPsiOptimizationSession,
    candidate: &LoopInvariantScalarMotionCandidate,
) -> Result<ValidatedLoopInvariantScalarMotion, LoopInvariantScalarMotionError> {
    validate::candidate(session, candidate)
}

/// Apply one independently validated candidate atomically and bind the
/// transformed unit to a fresh verified session.
pub fn apply_loop_invariant_scalar_motion(
    session: VerifiedPsiOptimizationSession,
    validated: ValidatedLoopInvariantScalarMotion,
) -> Result<AppliedLoopInvariantScalarMotion, LoopInvariantScalarMotionError> {
    apply::validated(session, validated)
}

fn rule_identity() -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.loop-invariant-scalar-motion.v1",
    )
}

fn validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.psi-validator.loop-invariant-scalar-motion.v1",
    )
}
