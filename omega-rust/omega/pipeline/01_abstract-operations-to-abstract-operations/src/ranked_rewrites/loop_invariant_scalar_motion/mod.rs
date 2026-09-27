//! Optimizer module role: executable entrance. Exact unique-preheader loop-invariant scalar relocation boundary.

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationSafetyClass,
    OptimizationUnitIdentity, OptimizationValidatorIdentity,
};
use optimization_unit::{
    EffectLink, NodeLocation, OptimizationNode, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance, PsiRealizationSite,
    PsiTransformationLedger, PsiTransformationRecord, ValueDefinitionSite,
    recompute_psi_optimization_unit_identity,
};
pub use optimization_unit::{
    LoopInvariantNodeResult, LoopInvariantScalarMotionCandidate, LoopInvariantScalarNode,
    LoopInvariantScalarRelocation,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, PlaceId, ValueId};

use crate::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
    CountedLoopAnalysisError, VerifiedPsiOptimizationSession,
};

mod apply;
mod propose;
mod validate;

#[cfg(test)]
pub(crate) mod tests;

/// Propose every unique-preheader cyclic component that still retains admissible
/// loop-invariant scalar nodes inside its member blocks: scalar-constant
/// leaves, invariant place observations, byte observations, pure-callee
/// scalar-signature, borrow-and-copyable-owned unit, and
/// borrow-and-copyable-owned scalar-result
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
/// on the rebound root, and a subslice preserves its structural view result
/// and bounds obligation byte-exact inside the moved operation. A
/// `StructuralByteSequenceFieldRead` shares that root resolution but is
/// proposition-pinned: its accepted bounds fact records `index < length`
/// over the operation's own operand identities, so both operands stay
/// byte-exact — each member-internal operand must already be a result the
/// same run preserves, and the `length` operand must name a
/// `StructuralByteSequenceFieldLength` measuring the rebound root's same
/// `path` and `field`. The
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
/// must be a borrow or a copyable owned whole-root spelling — `Owned` over
/// an affine or linear root would move the caller's place into the callee
/// outright, so it stays refused, while `Owned` over an unrestricted owned
/// parameter or an unrestricted claim-free scalar-array result copies the
/// payload into the callee, the same observation a shared borrow makes —
/// its claim roster must be empty
/// (the vacuous `ClaimTransfer` ownership row moves byte-exact with the
/// operation), and each argument's root must be visible at the
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
/// family's fourth call relocation and its second custody-rewriting shape,
/// in the two forms the cyclic eligibility fence confines: an affine
/// claim-free result the producing member block
/// dispatches through a `StructuralCase` or returns outright, or an
/// unrestricted claim-free result spelling one of the frontier's
/// plain-source shapes — a copy payload that never enters `owned_places`
/// and carries no disposal roster at all — either carrying no
/// claims, obligations, crash routes, or selected
/// evidence and structural arguments confined to the same borrow or
/// copyable-owned whitelist the other structural-signature calls obey. The
/// callee passes the same purity bar and the member roster
/// stays unobservable. The affine result place obeys the scalar-case
/// containment bound, so hoisting the call keeps the one persistent
/// preheader place
/// live across member-internal edges while every exit edge and member
/// return disposes it — the same frontier re-expression the establishment
/// performs, replayed independently at validation; the unrestricted result
/// needs no containment and no custody rewrite — it moves byte-exact under
/// the same argument the unrestricted record establishment uses. Its scalar
/// arguments
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
/// into the callee is an observation, not custody movement — so a staying
/// call keeps reading the persistent place each traversal while the
/// establishment it consumes relocates once into the preheader, and the
/// consuming call itself relocates behind its producer when the array is
/// the run-covered root its `Owned` argument names. The same
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

pub(crate) const LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME: &[u8] =
    b"omega.psi-pass.loop-invariant-scalar-motion.v1";

pub(crate) fn rule_identity() -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.loop-invariant-scalar-motion.v1",
    )
}

pub(crate) fn validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.psi-validator.loop-invariant-scalar-motion.v1",
    )
}

/// The scheduled contract for the relocation boundary. The pass requires the
/// transitive effect table the proposal computes itself from the session
/// unit; invalidation declares the full analysis surface because every
/// accepted relocation produces an independently verified transformed unit
/// rather than a patched revision.
pub(crate) fn contract() -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        rule_identity(),
        OptimizationPassIdentity::from_canonical_bytes(LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME),
        1,
        AnalysisSet::new([AnalysisKind::EffectSummaries]),
        AnalysisInvalidationSet::new(AnalysisKind::ALL),
        OptimizationSafetyClass::StructuralIdentity,
    )
    .expect("built-in pass contract has nonzero version")
}

#[derive(Debug)]
pub struct ValidatedLoopInvariantScalarMotion {
    candidate: LoopInvariantScalarMotionCandidate,
    output: PsiOptimizationUnit,
    provenance: Vec<ProvenanceRewrite>,
}

impl ValidatedLoopInvariantScalarMotion {
    pub const fn candidate(&self) -> &LoopInvariantScalarMotionCandidate {
        &self.candidate
    }

    pub(crate) fn validator(&self) -> OptimizationValidatorIdentity {
        validator_identity()
    }

    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }
}

#[derive(Debug)]
pub struct AppliedLoopInvariantScalarMotion {
    session: VerifiedPsiOptimizationSession,
    candidate: LoopInvariantScalarMotionCandidate,
    ledger: PsiTransformationLedger,
}

impl AppliedLoopInvariantScalarMotion {
    #[cfg(any(test, feature = "test-support"))]
    pub const fn session(&self) -> &VerifiedPsiOptimizationSession {
        &self.session
    }

    #[cfg(any(test, feature = "test-support"))]
    pub const fn candidate(&self) -> &LoopInvariantScalarMotionCandidate {
        &self.candidate
    }

    #[cfg(any(test, feature = "test-support"))]
    pub const fn ledger(&self) -> &PsiTransformationLedger {
        &self.ledger
    }

    /// Release the applied custody: the transformed session, the bound
    /// declaration, and the per-commit transformation ledger whose single
    /// record the caller chains into the pass ledger.
    pub fn into_parts(
        self,
    ) -> (
        VerifiedPsiOptimizationSession,
        LoopInvariantScalarMotionCandidate,
        PsiTransformationLedger,
    ) {
        (self.session, self.candidate, self.ledger)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn into_session(self) -> VerifiedPsiOptimizationSession {
        self.session
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopInvariantScalarMotionError {
    CandidateBudgetExhausted {
        required: u64,
        limit: u64,
    },
    StaleCandidateRevision {
        candidate: OptimizationUnitIdentity,
        current: OptimizationUnitIdentity,
    },
    UnknownComponent,
    AlreadyRelocated,
    CandidateMismatch,
    MissingNode {
        machine: MachineId,
        block: BlockId,
        node: u32,
    },
    CoordinateOverflow,
    OutputIdentityMismatch {
        candidate: OptimizationUnitIdentity,
        reconstructed: OptimizationUnitIdentity,
    },
    TransformedValidation(optimization_unit_semantics::OptimizationUnitValidationError),
    CountedLoop(CountedLoopAnalysisError),
    InvariantConstant(CountdownInvariantConstantAnalysisError),
    ReconstructedPlacement(CountdownInvariantConstantPlacementAnalysisError),
    InvalidLedger(optimization_unit::InvalidPsiTransformationLedger),
}

impl std::fmt::Display for LoopInvariantScalarMotionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "loop-invariant scalar motion failure: {self:?}")
    }
}

impl std::error::Error for LoopInvariantScalarMotionError {}
