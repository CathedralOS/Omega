//! Optimizer module role: executable entrance. Exact unique-preheader loop-invariant scalar relocation boundary.

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
use semantic_vocabulary::{BlockId, MachineId, OperationId, PlaceId, ScalarType, ValueId};

use crate::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
    CountedLoopAnalysisError, VerifiedPsiOptimizationSession,
};

mod apply;
mod model;
mod propose;
mod validate;

#[cfg(test)]
mod tests;

use model::candidate_identity;
pub use model::{
    AppliedLoopInvariantScalarMotion, LoopInvariantNodeResult, LoopInvariantScalarMotionCandidate,
    LoopInvariantScalarMotionError, LoopInvariantScalarNode, LoopInvariantScalarRelocation,
    ValidatedLoopInvariantScalarMotion,
};

/// Propose every unique-preheader cyclic component that still retains admissible
/// loop-invariant scalar nodes inside its member blocks: scalar-constant
/// leaves, invariant place observations, byte observations, pure-callee
/// scalar-signature calls, and
/// side-effect-free scalar
/// computations — including exact, saturating, and
/// wrapping variants carrying a verifier-discharged obligation, which moves
/// byte-exact inside the relocated operation —
/// whose operands are all
/// defined outside the component, name provably invariant member parameters
/// (each rebound to the representative every reaching edge agrees on, resolved
/// transitively across member-to-member edges), or are defined by another node
/// the same run relocates — invariant discovery iterates to a fixed point so a
/// chain of computations moves together in def-before-use order. A place
/// observation — one verifier-approved read of an established storage root —
/// is admissible only when its component performs no place mutation or
/// custody movement at all and the observed root is visible at the
/// preheader insertion point: either directly, or through a member structural
/// parameter every reaching edge resolves to the same preheader-visible
/// representative — the root analog of member scalar-parameter resolution,
/// rebound on the moved node rather than relocated byte-exact. The byte
/// family adds its own evidence: a `ByteSequenceRead` or
/// `ByteSequenceSubslice` must also substitute each scalar operand under the
/// same rule and keep its `length` operand coupled to a `ByteSequenceLength`
/// measuring the rebound root, and a subslice preserves its structural view
/// result and bounds obligation byte-exact inside the moved operation. The
/// byte family's non-observation member, an `EstablishByteSequenceLiteral`,
/// declares a fresh immutable view root over constant bytes: it reads no
/// scalar or structural operand and carries no custody events, so the whole
/// operation — declared place, type, and payload — moves byte-exact while
/// consumers keep spelling the same place identity. A
/// scalar-signature `Call` adds the family's first call relocation: its
/// callee's transitive effect summary must prove no observable effect, no
/// crash, and no suspension — the `structural_state` axis is exempt because
/// a scalar call passes no places — and every node inside the member roster
/// must be unobservable, so hoisting the call's possible divergence can
/// never reorder member work anyone could see. Its scalar arguments obey the
/// same member-parameter substitution a computation obeys; discharged
/// requirement obligations move byte-exact inside the operation.
/// Computation,
/// observation, and establishment
/// relocation is non-speculative: every successor of the unique preheader's
/// terminator must be an entry edge into the component — reaching the
/// preheader guarantees entering — and only member blocks guaranteed to
/// execute on every traversal that leaves the component contribute
/// computations — a member block a bypassing exit can skip keeps its
/// computations inside the loop. Scalar-constant leaves are exempt from both
/// halves of the gate: materializing a constant performs no work a traversal
/// could have skipped.
/// Each candidate relocates the component's complete admissible set into the
/// tail of the component's unique preheader, ahead of the terminator that
/// owns the entry edges and ahead of any already-relocated
/// countdown-certificate constants owned by the dedicated countdown boundary.
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
