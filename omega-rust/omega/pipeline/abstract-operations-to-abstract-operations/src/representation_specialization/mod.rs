//! Optimizer module role: executable entrance. Established-case membership specialization boundary.
//!
//! One bounded specialization family: a `StructuralCaseMembership` observation
//! whose `source` place is declared `StructuralPlaceKind::OperationResult` and
//! whose producer node in the same function is an `EstablishScalarCase` reads
//! a place whose case is fixed at establishment — the operation-result place
//! is assigned exactly once by its producer and no operation can rewrite it,
//! so the membership's Boolean answer is `result_case == case` at every
//! observation site. The traversal "observe the established case" specializes
//! into `BooleanConstant` at the same node: the result keeps its value
//! identity, the node keeps its `PsiProvenance::Operation` custody and fuel
//! settlement, and successors, definitions, uses, and ownership events are
//! unchanged.
//!
//! Memberships on parameter, block-parameter, result, or any non-case
//! producer places are not covered; memberships carrying a non-empty path
//! observe a nested position the root case does not determine and decline.
//! Only machines absent from the authenticated Terminal-cycle component
//! roster are eligible: a machine containing a verified cyclic component is
//! frozen byte-exact under `validate_frozen_component_blocks`. Proposal,
//! independent validation, and application are separate boundaries; the
//! candidate pins the exact input and output revision identities, the
//! specialized place, the producer operation, and every folded observation's
//! site, custody identity, and verdict, so replay rejects forged or stale
//! rows by recomputation rather than trust.

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

mod apply;
mod model;
mod propose;
mod validate;

pub use model::{
    AppliedCaseMembershipSpecialization, CaseMembershipSpecializationCandidate,
    CaseMembershipSpecializationError, ResolvedCaseMembership,
    ValidatedCaseMembershipSpecialization,
};
use model::{EstablishedCasePlan, candidate_identity};

pub fn propose_case_membership_specializations(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<CaseMembershipSpecializationCandidate>, CaseMembershipSpecializationError> {
    propose::all(session, candidate_limit)
}

pub fn validate_case_membership_specialization(
    session: &VerifiedPsiOptimizationSession,
    candidate: &CaseMembershipSpecializationCandidate,
) -> Result<ValidatedCaseMembershipSpecialization, CaseMembershipSpecializationError> {
    validate::candidate(session, candidate)
}

pub fn apply_case_membership_specialization(
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
