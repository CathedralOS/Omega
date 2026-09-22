//! Optimizer module role: executable entrance. Proven-case membership specialization boundary.
//!
//! One bounded specialization family: a `StructuralCaseMembership`
//! observation whose `source` place carries a case the unit itself proves
//! folds to a `BooleanConstant` holding that verdict. Two proofs qualify.
//! Establishment: the place is declared `StructuralPlaceKind::OperationResult`
//! and its producer node in the same function is an `EstablishScalarCase` —
//! the operation-result place is assigned exactly once by its producer and
//! no operation can rewrite it. Sole-case roster: the place's declared
//! structural type is a closed `Sum` or `Mixed` roster of exactly one case,
//! so every inhabitant of the place holds that case independently of how it
//! arrived — parameters, results, and non-`EstablishScalarCase` producers
//! all qualify. In either proof the membership's Boolean answer is
//! `proven_case == case` at every observation site. The traversal "observe
//! the proven case" specializes into `BooleanConstant` at the same node: the
//! result keeps its value identity, the node keeps its
//! `PsiProvenance::Operation` custody and fuel settlement, and successors,
//! definitions, uses, and ownership events are unchanged.
//!
//! Memberships on places whose declared type holds more than one case and
//! carries no `EstablishScalarCase` producer are not covered. A membership's
//! non-empty path names a nested position — a `Record`/`Mixed` common field,
//! a `FixedArray` element, or a `Reference` referent — which folds when the
//! roster the path resolves to closes over exactly one case; positions that
//! do not resolve to a sole-case roster decline. Only machines absent from
//! the authenticated
//! Terminal-cycle component roster are eligible: a machine containing a
//! verified cyclic component is frozen byte-exact under
//! `validate_frozen_component_blocks`. Proposal, independent validation, and
//! application are separate boundaries; the candidate pins the exact input
//! and output revision identities, the specialized place, the proof basis,
//! and every folded observation's site, custody identity, and verdict, so
//! replay rejects forged or stale rows by recomputation rather than trust.

use optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};
use optimization_unit::{
    NodeLocation, ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiRealizationSite, PsiTransformationLedger, PsiTransformationRecord,
    recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{
    MachineId, OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralPlaceKind,
};

use abstract_operations::AbstractOperation as O;

use crate::VerifiedPsiOptimizationSession;

pub(crate) mod admission;
mod apply;
pub(crate) mod propose;
pub(crate) mod validate;

pub fn propose_case_membership_specializations(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<CaseMembershipSpecializationCandidate>, CaseMembershipSpecializationError> {
    propose::all(session, candidate_limit)
}

pub(crate) fn validate_case_membership_specialization(
    session: &VerifiedPsiOptimizationSession,
    candidate: &CaseMembershipSpecializationCandidate,
) -> Result<ValidatedCaseMembershipSpecialization, CaseMembershipSpecializationError> {
    validate::candidate(session, candidate)
}

pub(crate) fn apply_case_membership_specialization(
    session: VerifiedPsiOptimizationSession,
    validated: ValidatedCaseMembershipSpecialization,
) -> Result<AppliedCaseMembershipSpecialization, CaseMembershipSpecializationError> {
    apply::validated(session, validated)
}

fn rule_identity() -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(
        b"omega.psi-rule.case-membership-specialization.v1",
    )
}

fn validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.psi-validator.case-membership-specialization.v1",
    )
}

#[cfg(test)]
mod tests;

/// One membership observation folded to its proven Boolean. The row records
/// where the observation sits (`site`), which source custody identity the
/// folded constant retains (`psi_operation`), which value it still defines
/// (`result`), the place it observed (`source` produced by `producer` when
/// the basis is establishment), the case the source asked about
/// (`observed_case`), the case proven at the observed position
/// (`proven_case`), and the proven verdict (`outcome`, always
/// `proven_case == observed_case`). `producer` is `Some` when the verdict is
/// establishment-derived and `None` when a declared sole-case roster — the
/// place's root type or the roster the membership's path resolves to —
/// proves the case independently of any producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCaseMembership {
    site: NodeLocation,
    psi_operation: OperationId,
    result: semantic_vocabulary::ValueId,
    source: PlaceId,
    producer: Option<OperationId>,
    observed_case: StructuralCaseId,
    proven_case: StructuralCaseId,
    outcome: bool,
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

    /// The `EstablishScalarCase` producer identity proving the observed
    /// root case, or `None` when a sole-case roster proves the verdict —
    /// either at the place's declared type or at the nested position the
    /// membership's path resolves to.
    pub const fn producer(&self) -> Option<OperationId> {
        self.producer
    }

    /// The case the observation tested.
    pub const fn observed_case(&self) -> StructuralCaseId {
        self.observed_case
    }

    /// The case the observed position is proven to hold.
    pub const fn proven_case(&self) -> StructuralCaseId {
        self.proven_case
    }

    /// The proven membership verdict folded into the constant.
    pub const fn outcome(&self) -> bool {
        self.outcome
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseMembershipSpecializationCandidate {
    identity: OptimizationCandidateIdentity,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    machine: MachineId,
    place: PlaceId,
    producer: Option<OperationId>,
    memberships: Vec<ResolvedCaseMembership>,
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

    /// The place whose case observations fold.
    pub const fn place(&self) -> PlaceId {
        self.place
    }

    /// The `EstablishScalarCase` identity fixing the place's root case, or
    /// `None` when the root case has no establishment witness — including
    /// candidates whose rows are all proven by nested sole-case rosters.
    pub const fn producer(&self) -> Option<OperationId> {
        self.producer
    }

    pub fn memberships(&self) -> &[ResolvedCaseMembership] {
        &self.memberships
    }
}

#[derive(Debug)]
pub struct ValidatedCaseMembershipSpecialization {
    candidate: CaseMembershipSpecializationCandidate,
    output: PsiOptimizationUnit,
    provenance: Vec<ProvenanceRewrite>,
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
    session: VerifiedPsiOptimizationSession,
    candidate: CaseMembershipSpecializationCandidate,
    ledger: PsiTransformationLedger,
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
    /// machine holds an authenticated cyclic component, the place carries no
    /// case proof — neither an `EstablishScalarCase` producer nor a declared
    /// sole-case roster at any observed position — or it no longer exists.
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

/// The independently derived specialization plan for one proven place:
/// every `StructuralCaseMembership` observing it whose verdict the per-site
/// basis proves, sorted by node location. `producer` records the place's
/// `EstablishScalarCase` root-case witness when one exists; each row still
/// carries its own basis — establishment rows at empty paths, roster rows
/// at the place's root type or at a resolved nested position. Proposal and
/// validation both recompute this plan; the candidate is accepted only when
/// its claimed rows equal the replayed plan exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CaseMembershipPlan {
    pub(crate) machine: MachineId,
    pub(crate) place: PlaceId,
    pub(crate) producer: Option<OperationId>,
    pub(crate) memberships: Vec<ResolvedCaseMembership>,
}

fn candidate_identity(
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
        canonical.extend_from_slice(
            &row.producer
                .map(|producer| producer.get())
                .unwrap_or(0)
                .to_le_bytes(),
        );
        canonical.extend_from_slice(&row.observed_case.get().to_le_bytes());
        canonical.extend_from_slice(&row.proven_case.get().to_le_bytes());
        canonical.push(u8::from(row.outcome));
    }
    OptimizationCandidateIdentity::from_canonical_bytes(&canonical)
}
