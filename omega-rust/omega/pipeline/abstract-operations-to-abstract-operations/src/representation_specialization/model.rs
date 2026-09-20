//! Optimizer module role: carrier leaf. Immutable exact specialization proposal and accepted output.

use super::{
    MachineId, NodeLocation, OperationId, OptimizationCandidateIdentity, OptimizationUnitIdentity,
    PlaceId, ProvenanceRewrite, PsiOptimizationUnit, PsiTransformationLedger, StructuralCaseId,
    VerifiedPsiOptimizationSession,
};

/// One membership observation folded to its proven Boolean. The row records
/// where the observation sits (`site`), which source custody identity the
/// folded constant retains (`psi_operation`), which value it still defines
/// (`result`), the established place it observed (`source` produced by
/// `producer`), the case the source asked about (`observed_case`), the case
/// the establishment fixed (`established_case`), and the proven verdict
/// (`outcome`, always `established_case == observed_case`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCaseMembership {
    pub(super) site: NodeLocation,
    pub(super) psi_operation: OperationId,
    pub(super) result: semantic_vocabulary::ValueId,
    pub(super) source: PlaceId,
    pub(super) producer: OperationId,
    pub(super) observed_case: StructuralCaseId,
    pub(super) established_case: StructuralCaseId,
    pub(super) outcome: bool,
}

impl ResolvedCaseMembership {
    /// Owning node of the folded observation.
    pub const fn site(&self) -> NodeLocation {
        self.site
    }

    /// Source operation custody identity the folded constant retains.
    pub const fn psi_operation(&self) -> OperationId {
        self.psi_operation
    }

    /// The Boolean value the observation defined and the constant keeps.
    pub const fn result(&self) -> semantic_vocabulary::ValueId {
        self.result
    }

    /// The established operation-result place the observation read.
    pub const fn source(&self) -> PlaceId {
        self.source
    }

    /// The `EstablishScalarCase` producer identity for the observed place.
    pub const fn producer(&self) -> OperationId {
        self.producer
    }

    /// The case the observation tested.
    pub const fn observed_case(&self) -> StructuralCaseId {
        self.observed_case
    }

    /// The case the establishment fixed for the observed place.
    pub const fn established_case(&self) -> StructuralCaseId {
        self.established_case
    }

    /// The proven membership verdict folded into the constant.
    pub const fn outcome(&self) -> bool {
        self.outcome
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseMembershipSpecializationCandidate {
    pub(super) identity: OptimizationCandidateIdentity,
    pub(super) input: OptimizationUnitIdentity,
    pub(super) output: OptimizationUnitIdentity,
    pub(super) machine: MachineId,
    pub(super) place: PlaceId,
    pub(super) producer: OperationId,
    pub(super) memberships: Vec<ResolvedCaseMembership>,
}

impl CaseMembershipSpecializationCandidate {
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

    /// The established operation-result place whose case observations fold.
    pub const fn place(&self) -> PlaceId {
        self.place
    }

    /// The `EstablishScalarCase` identity that fixed the place's case.
    pub const fn producer(&self) -> OperationId {
        self.producer
    }

    pub fn memberships(&self) -> &[ResolvedCaseMembership] {
        &self.memberships
    }
}

#[derive(Debug)]
pub struct ValidatedCaseMembershipSpecialization {
    pub(super) candidate: CaseMembershipSpecializationCandidate,
    pub(super) output: PsiOptimizationUnit,
    pub(super) provenance: Vec<ProvenanceRewrite>,
}

impl ValidatedCaseMembershipSpecialization {
    pub const fn candidate(&self) -> &CaseMembershipSpecializationCandidate {
        &self.candidate
    }

    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }
}

#[derive(Debug)]
pub struct AppliedCaseMembershipSpecialization {
    pub(super) session: VerifiedPsiOptimizationSession,
    pub(super) candidate: CaseMembershipSpecializationCandidate,
    pub(super) ledger: PsiTransformationLedger,
}

impl AppliedCaseMembershipSpecialization {
    pub const fn session(&self) -> &VerifiedPsiOptimizationSession {
        &self.session
    }

    pub const fn candidate(&self) -> &CaseMembershipSpecializationCandidate {
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
pub enum CaseMembershipSpecializationError {
    CandidateBudgetExhausted {
        required: u64,
        limit: u64,
    },
    StaleCandidateRevision {
        candidate: OptimizationUnitIdentity,
        current: OptimizationUnitIdentity,
    },
    /// No specialization plan exists for the claimed place: either the
    /// machine holds an authenticated cyclic component, the place is not an
    /// `EstablishScalarCase` operation result, or it no longer exists.
    UnknownPlace,
    /// The place exists but currently has no foldable membership left.
    AlreadySpecialized,
    CandidateMismatch,
    MissingSite {
        machine: MachineId,
        block: semantic_vocabulary::BlockId,
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

impl std::fmt::Display for CaseMembershipSpecializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "case-membership specialization failure: {self:?}"
        )
    }
}

impl std::error::Error for CaseMembershipSpecializationError {}

/// The independently derived specialization plan for one established place:
/// every empty-path `StructuralCaseMembership` observing it, sorted by node
/// location. Proposal and validation both recompute this plan; the candidate
/// is accepted only when its claimed rows equal the replayed plan exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EstablishedCasePlan {
    pub(super) machine: MachineId,
    pub(super) place: PlaceId,
    pub(super) producer: OperationId,
    pub(super) established_case: StructuralCaseId,
    pub(super) memberships: Vec<ResolvedCaseMembership>,
}

pub(super) fn candidate_identity(
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    machine: MachineId,
    place: PlaceId,
    memberships: &[ResolvedCaseMembership],
) -> OptimizationCandidateIdentity {
    let mut canonical = b"omega.psi.case-membership-specialization-candidate.v1".to_vec();
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&output.bytes());
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&place.get().to_le_bytes());
    canonical.extend_from_slice(
        &u64::try_from(memberships.len())
            .expect("membership count fits u64")
            .to_le_bytes(),
    );
    for row in memberships {
        canonical.extend_from_slice(&row.site.machine.get().to_le_bytes());
        canonical.extend_from_slice(&row.site.block.get().to_le_bytes());
        canonical.extend_from_slice(&row.site.node.to_le_bytes());
        canonical.extend_from_slice(&row.psi_operation.get().to_le_bytes());
        canonical.extend_from_slice(&row.result.get().to_le_bytes());
        canonical.extend_from_slice(&row.source.get().to_le_bytes());
        canonical.extend_from_slice(&row.producer.get().to_le_bytes());
        canonical.extend_from_slice(&row.observed_case.get().to_le_bytes());
        canonical.extend_from_slice(&row.established_case.get().to_le_bytes());
        canonical.push(u8::from(row.outcome));
    }
    OptimizationCandidateIdentity::from_canonical_bytes(&canonical)
}
