//! Optimizer module role: carrier leaf. Immutable exact specialization proposal and accepted output.

use super::{
    BlockId, EdgeId, MachineId, NodeLocation, OptimizationCandidateIdentity,
    OptimizationUnitIdentity, ProvenanceRewrite, PsiOptimizationUnit, PsiTransformationLedger,
    ValueId, VerifiedPsiOptimizationSession,
};

/// One exact incoming edge that supplies a provably constant state argument to
/// a shared dispatch state. The row records which edge supplied the
/// specialization (`incoming_edge`), which state argument it fixed
/// (`parameter` bound to `argument`, proven `constant`), and which dispatch
/// arm edge that constant resolves (`taken_edge` toward `resolved_target`,
/// while `rejected_edge` stays behind on every other incoming path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecializedStateEdge {
    pub(super) incoming_edge: EdgeId,
    pub(super) predecessor: NodeLocation,
    pub(super) parameter: ValueId,
    pub(super) argument: ValueId,
    pub(super) constant: bool,
    pub(super) taken_edge: EdgeId,
    pub(super) rejected_edge: EdgeId,
    pub(super) resolved_target: BlockId,
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

    /// The proven Boolean the dispatch condition takes on this path.
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
    pub(super) identity: OptimizationCandidateIdentity,
    pub(super) input: OptimizationUnitIdentity,
    pub(super) output: OptimizationUnitIdentity,
    pub(super) machine: MachineId,
    pub(super) dispatch: BlockId,
    pub(super) specializations: Vec<SpecializedStateEdge>,
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
    pub(super) candidate: StateArgumentSpecializationCandidate,
    pub(super) output: PsiOptimizationUnit,
    pub(super) provenance: Vec<ProvenanceRewrite>,
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
    pub(super) session: VerifiedPsiOptimizationSession,
    pub(super) candidate: StateArgumentSpecializationCandidate,
    pub(super) ledger: PsiTransformationLedger,
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
/// every constant-supplied unconditional incoming edge, sorted by edge
/// identity. Proposal and validation both recompute this plan; the candidate
/// is accepted only when its claimed rows equal the replayed plan exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DispatchSpecializationPlan {
    pub(crate) machine: MachineId,
    pub(crate) dispatch: BlockId,
    pub(crate) edges: Vec<SpecializedStateEdge>,
}

pub(super) fn candidate_identity(
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
