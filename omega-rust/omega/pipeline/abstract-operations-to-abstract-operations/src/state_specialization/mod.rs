//! Optimizer module role: executable entrance. Exact constant state-argument dispatch specialization boundary.
//!
//! One bounded specialization family: a dispatch state is a non-entry block
//! whose terminator is a `Conditional` reading one of that block's own scalar
//! parameters — the state argument. The condition reads the parameter either
//! directly (a Boolean argument) or through an in-block integer comparison
//! `parameter CMP literal` (an integer argument), where every other node in
//! the block is a pure scalar constant. When an incoming edge entering the
//! dispatch binds that parameter to an argument proven to be one exact
//! constant the condition resolves — a Boolean for the direct read, an
//! integer evaluated under the operand type's own ordering for the
//! comparison — the traversal "this edge, then the dispatch's resolved arm"
//! specializes into a single edge: the incoming edge is retargeted to the
//! resolved arm's block with its scalar bindings composed through the edge's
//! own bindings, and the fused edge carries both source edges' custody —
//! its own `PsiProvenance::Edge` first, then the resolved arm's — with one
//! paired fuel settlement each. The incoming edge is either an unconditional
//! `Jump` successor or one arm of a `Conditional` predecessor; a fused
//! conditional arm leaves its sibling arm byte-exact. The dispatch state and
//! both of its arm edges remain for every other incoming edge, so no source
//! work is removed and no block becomes unreachable.
//!
//! Two constant-proof sources admit the bound argument. First, the sparse
//! conditional constant analysis proves the argument's own value. Second —
//! the result specialization — the argument resolves to the scalar `result`
//! of an in-function direct `Call` whose callee's function carries exactly
//! one `Return`, and the callee's own lattice proves that returned value
//! constant: because the lowering delivers evaluated state-call arguments
//! through single-predecessor forwarding blocks, the bound argument is first
//! resolved through unique-incoming-edge parameter bindings to the delivered
//! value, then that value is matched to its producing `Call`. The call still
//! executes at the predecessor — only its proven result resolves the
//! dispatch — so no callee purity is required, a callee with several
//! `Return` nodes or a non-constant return admits nothing, and dynamic,
//! structural, and boundary calls produce their results through different
//! operations and never match.
//!
//! Only machines absent from the authenticated Terminal-cycle component roster
//! are eligible: a machine containing a verified cyclic component is frozen
//! byte-exact under `validate_frozen_component_blocks`, so edge surgery there
//! can never be admitted. Proposal, independent validation, and application
//! are separate boundaries; the candidate pins the exact input and output
//! revision identities, the supplying edge identity, the resolved arm edge,
//! and the proven constant, so replay rejects forged or mismatched edge
//! provenance by recomputation rather than trust.

use optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};
use optimization_unit::{
    NodeLocation, OptimizationBlock, OptimizationEdge, OptimizationNode, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiTransformationLedger, PsiTransformationRecord, ValueUse,
    recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, ValueId};

use abstract_operations::{AbstractOperation as O, ValueBinding};

use crate::{
    AnalysisProduct, ScalarConstant, ScalarConstantAnalysis, VerifiedPsiOptimizationSession,
    compute_analysis,
};

mod admission;
mod apply;
pub(crate) mod propose;
pub(crate) mod validate;

pub fn propose_state_argument_specializations(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<StateArgumentSpecializationCandidate>, StateArgumentSpecializationError> {
    propose::all(session, candidate_limit)
}

pub fn validate_state_argument_specialization(
    session: &VerifiedPsiOptimizationSession,
    candidate: &StateArgumentSpecializationCandidate,
) -> Result<ValidatedStateArgumentSpecialization, StateArgumentSpecializationError> {
    validate::candidate(session, candidate)
}

pub(crate) fn apply_state_argument_specialization(
    session: VerifiedPsiOptimizationSession,
    validated: ValidatedStateArgumentSpecialization,
) -> Result<AppliedStateArgumentSpecialization, StateArgumentSpecializationError> {
    apply::validated(session, validated)
}

fn rule_identity() -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.state-argument-specialization.v1",
    )
}

fn validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.psi-validator.state-argument-specialization.v1",
    )
}

#[cfg(test)]
mod tests;

/// One exact incoming edge that supplies a provably constant state argument to
/// a shared dispatch state. The row records which edge supplied the
/// specialization (`incoming_edge`), which state argument it fixed
/// (`parameter` bound to `argument`, proven `constant`), and which dispatch
/// arm edge that constant resolves (`taken_edge` toward `resolved_target`,
/// while `rejected_edge` stays behind on every other incoming path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecializedStateEdge {
    incoming_edge: EdgeId,
    predecessor: NodeLocation,
    parameter: ValueId,
    argument: ValueId,
    constant: bool,
    taken_edge: EdgeId,
    rejected_edge: EdgeId,
    resolved_target: BlockId,
}

impl SpecializedStateEdge {
    /// Exact edge that supplied the constant state argument.
    pub const fn incoming_edge(&self) -> EdgeId {
        self.incoming_edge
    }

    /// Owner node of the supplying edge.
    pub const fn predecessor(&self) -> NodeLocation {
        self.predecessor
    }

    /// The dispatch state's block parameter the edge fixes.
    pub const fn parameter(&self) -> ValueId {
        self.parameter
    }

    /// The bound argument proven constant.
    pub const fn argument(&self) -> ValueId {
        self.argument
    }

    /// The Boolean the dispatch condition takes on this path — the supplied
    /// constant itself for a Boolean argument, or the evaluated comparison
    /// result (`parameter CMP literal`) for an integer argument.
    pub const fn constant(&self) -> bool {
        self.constant
    }

    /// The dispatch arm edge this path resolves.
    pub const fn taken_edge(&self) -> EdgeId {
        self.taken_edge
    }

    /// The dispatch arm edge this path provably never takes.
    pub const fn rejected_edge(&self) -> EdgeId {
        self.rejected_edge
    }

    /// The resolved arm's target block — the fused edge's new target.
    pub const fn resolved_target(&self) -> BlockId {
        self.resolved_target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateArgumentSpecializationCandidate {
    identity: OptimizationCandidateIdentity,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    machine: MachineId,
    dispatch: BlockId,
    specializations: Vec<SpecializedStateEdge>,
}

impl StateArgumentSpecializationCandidate {
    pub const fn identity(&self) -> OptimizationCandidateIdentity {
        self.identity
    }

    pub const fn input(&self) -> OptimizationUnitIdentity {
        self.input
    }

    pub const fn output(&self) -> OptimizationUnitIdentity {
        self.output
    }

    pub const fn machine(&self) -> MachineId {
        self.machine
    }

    /// The shared dispatch state specialized on its constant-supplied edges.
    pub const fn dispatch(&self) -> BlockId {
        self.dispatch
    }

    pub fn specializations(&self) -> &[SpecializedStateEdge] {
        &self.specializations
    }
}

#[derive(Debug)]
pub struct ValidatedStateArgumentSpecialization {
    candidate: StateArgumentSpecializationCandidate,
    output: PsiOptimizationUnit,
    provenance: Vec<ProvenanceRewrite>,
}

impl ValidatedStateArgumentSpecialization {
    pub const fn candidate(&self) -> &StateArgumentSpecializationCandidate {
        &self.candidate
    }

    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }
}

#[derive(Debug)]
pub struct AppliedStateArgumentSpecialization {
    session: VerifiedPsiOptimizationSession,
    candidate: StateArgumentSpecializationCandidate,
    ledger: PsiTransformationLedger,
}

impl AppliedStateArgumentSpecialization {
    pub const fn session(&self) -> &VerifiedPsiOptimizationSession {
        &self.session
    }

    pub const fn candidate(&self) -> &StateArgumentSpecializationCandidate {
        &self.candidate
    }

    pub const fn ledger(&self) -> &PsiTransformationLedger {
        &self.ledger
    }

    pub fn into_session(self) -> VerifiedPsiOptimizationSession {
        self.session
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateArgumentSpecializationError {
    CandidateBudgetExhausted {
        required: u64,
        limit: u64,
    },
    StaleCandidateRevision {
        candidate: OptimizationUnitIdentity,
        current: OptimizationUnitIdentity,
    },
    /// No specialization plan exists for the claimed dispatch state: either
    /// the machine holds an authenticated cyclic component, the block is not
    /// a parameter-dispatch state, or it no longer exists.
    UnknownDispatch,
    /// The dispatch state exists but currently has no constant-supplied edge
    /// left to specialize.
    AlreadySpecialized,
    CandidateMismatch,
    MissingSite {
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
    InvalidLedger(optimization_unit::InvalidPsiTransformationLedger),
}

impl std::fmt::Display for StateArgumentSpecializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "state-argument specialization failure: {self:?}")
    }
}

impl std::error::Error for StateArgumentSpecializationError {}

/// The independently derived specialization plan for one dispatch state:
/// every constant-supplied incoming edge — unconditional `Jump` successors
/// and `Conditional` predecessor arms — sorted by edge identity. The state
/// argument an edge supplies is a proven Boolean when the dispatch reads the
/// parameter directly, or a proven integer when the condition computes
/// `parameter CMP literal` in-block; its constant may come from the sparse
/// lattice directly or — the result specialization — resolve through
/// single-predecessor forwarding-block parameters to the scalar result of
/// an in-function `Call` whose single-return callee's lattice proves that
/// result constant. Proposal and validation both recompute this plan; the
/// candidate is accepted only when its claimed rows equal the replayed plan
/// exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DispatchSpecializationPlan {
    pub(crate) machine: MachineId,
    pub(crate) dispatch: BlockId,
    pub(crate) edges: Vec<SpecializedStateEdge>,
}

fn candidate_identity(
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    machine: MachineId,
    dispatch: BlockId,
    specializations: &[SpecializedStateEdge],
) -> OptimizationCandidateIdentity {
    let mut canonical = b"omega.psi.state-argument-specialization-candidate.v1".to_vec();
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&output.bytes());
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&dispatch.get().to_le_bytes());
    canonical.extend_from_slice(
        &u64::try_from(specializations.len())
            .expect("specialization count fits u64")
            .to_le_bytes(),
    );
    for row in specializations {
        canonical.extend_from_slice(&row.incoming_edge.get().to_le_bytes());
        canonical.extend_from_slice(&row.predecessor.block.get().to_le_bytes());
        canonical.extend_from_slice(&row.predecessor.node.to_le_bytes());
        canonical.extend_from_slice(&row.parameter.get().to_le_bytes());
        canonical.extend_from_slice(&row.argument.get().to_le_bytes());
        canonical.push(u8::from(row.constant));
        canonical.extend_from_slice(&row.taken_edge.get().to_le_bytes());
        canonical.extend_from_slice(&row.rejected_edge.get().to_le_bytes());
        canonical.extend_from_slice(&row.resolved_target.get().to_le_bytes());
    }
    OptimizationCandidateIdentity::from_canonical_bytes(&canonical)
}
