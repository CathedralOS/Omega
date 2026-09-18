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
/// scalar-signature, borrow unit, and borrow scalar-result
/// structural calls, byte-sequence-literal, primitive-local, record, and
/// scalar-array establishments, and
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
/// custody movement at all and the observed root lands where the relocated
/// run can see it: visible at the
/// preheader insertion point directly, produced by a node the same run
/// already covers (the persistent cell holds the same contents on every
/// traversal, so the read relocates byte-exact behind its producer), or
/// through a member structural
/// parameter every reaching edge resolves to the same preheader-visible or
/// run-covered
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
/// requirement obligations move byte-exact inside the operation. A
/// `CallUnit` — a structural-signature call producing no result — adds the
/// family's second call relocation on top of that evidence: the component
/// must additionally preserve member-visible place custody (the callee can
/// observe caller places through its borrows), every structural argument
/// must be a borrow — owned access would move the caller's place into the
/// callee outright, so it stays refused — its claim roster must be empty
/// (the vacuous `ClaimTransfer` ownership row moves byte-exact with the
/// operation), and each shared-borrow argument's root must be visible at the
/// preheader insertion point either directly, through an invariant member
/// structural parameter's representative (rebound on the moved node), or
/// through a node earlier in the same run that produced the root (the
/// argument stays byte-exact). A mutable or write-only borrow's root is
/// narrower still: it may name only a member-produced place whose unique
/// producer relocates in the same run and no other member observes — the
/// callee's writes then land in a cell nothing else reads.
/// `CallStructuralScalar` — the same borrow call returning one
/// scalar — adds the family's third call relocation on unchanged evidence:
/// its preserved result joins the run's relocated values, so a member node
/// consuming the call's return relocates behind it in the same run, and the
/// place-custody bound is also what makes the hoisted invocation return
/// what every in-loop traversal's invocation returned.
/// `CallStructural` — the call returning a fresh structural place — adds the
/// family's fourth call relocation and its second custody-rewriting shape:
/// the admitted form is the one the cyclic eligibility fence already
/// confines, an affine claim-free result the producing member block
/// dispatches through a `StructuralCase` or returns outright, carrying no
/// structural arguments, claims, obligations, crash routes, or selected
/// evidence. The callee passes the same purity bar and the member roster
/// stays unobservable; the result place obeys the scalar-case containment
/// bound, so hoisting the call keeps the one persistent preheader place
/// live across member-internal edges while every exit edge and member
/// return disposes it — the same frontier re-expression the establishment
/// performs, replayed independently at validation. Its scalar arguments
/// obey the shared member-parameter substitution, and its declared place
/// joins the run's relocated roots.
/// An `EstablishPrimitiveLocal` adds the family's second establishment
/// relocation — and the storage prerequisite that lets the structural-scalar
/// call leave at all: the cyclic eligibility fence only lets a borrow
/// structural argument name a `let mut` primitive local's place, so the call
/// relocates only when the establishment that produced its borrowed root
/// leaves in the same run and the run keeps the declared place identity
/// byte-exact for the argument to keep spelling. The establishment reads one
/// scalar initializer — admitted under the same use-site substitution a
/// computation obeys — and the place-custody bound is what makes hoisting a
/// *re-established-every-iteration* cell sound: only when no member observes
/// the declared place's evolving contents does a cell initialized once still
/// read `value` on every traversal. When a member call borrows that cell
/// mutably, the coupling runs the other way too: the establishment relocates
/// only when every mutable borrower is itself admissible under the run
/// extended by its root, so a borrower that could read accumulated
/// post-write contents never stays behind while its producer leaves.
/// An `EstablishRecord` adds the family's third establishment relocation —
/// the primitive local's multi-field sibling: it declares a fresh claim-free
/// record place whose declaration-ordered scalar initializers
/// each obey the same use-site substitution, while a structural field
/// initializer copies a whole unrestricted root whose landing obeys the
/// shared-borrow call-argument rule — already visible at the preheader
/// insertion point, the representative an invariant member structural
/// parameter resolves to (rebound on the moved node), or produced by a node
/// earlier in the same run (spelled byte-exact). The declared place,
/// structural type, result custody, declaration order, and
/// bounded-integer range obligations move byte-exact inside the moved
/// operation. The same whole-component custody bound makes the hoist sound:
/// only when no member stores to the declared place does a record
/// established once still read its initializers on every traversal. An
/// affine `EstablishRecord` — admitted only for the field-free declaration
/// the composed-control lowering emits for a trivial affine local — instead
/// joins the scalar-case family's custody-rewriting move: the cyclic
/// eligibility fence already confined the fresh place to the member block
/// that established it, discarding it on every departing edge, so hoisting
/// the establishment keeps the one persistent preheader place live across
/// member-internal edges while every exit edge and member return disposes
/// it — the same containment bound and frontier re-expression the affine
/// scalar-case and structural-call results obey.
/// An `EstablishScalarArray` adds the family's fourth establishment
/// relocation — the record's flat sibling: it declares a fresh claim-free
/// unrestricted scalar-array place whose row-major scalar leaves each obey
/// the same use-site substitution, and the declared place, structural type,
/// and result custody move byte-exact inside the moved operation. The
/// custody bound carries the array's own wrinkle: a member call may spell
/// the fresh root as an `Owned` argument — copying an unrestricted payload
/// into the callee is an observation, not custody movement — so the staying
/// call keeps reading the persistent place each traversal while the
/// establishment it consumes relocates once into the preheader. The same
/// bound makes the hoist sound: only when no member stores to the declared
/// place does a payload established once still read its elements on every
/// traversal.
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
