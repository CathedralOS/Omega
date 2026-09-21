//! Optimizer module role: executable entrance. Proven field-value specialization boundary.
//!
//! One bounded specialization family: a `BooleanStructuralField` or
//! `IntegerStructuralField` observation whose `source` place carries a stored
//! field value the unit itself proves folds to a `BooleanConstant` or
//! `IntegerConstant` holding that value. Three proofs qualify. Record
//! establishment: the place is declared `StructuralPlaceKind::OperationResult`
//! and its producer node in the same function is an `EstablishRecord` — the
//! operation-result place is assigned exactly once by its producer and no
//! operation can rewrite it, so the record field initializer fixes the field's
//! stored value permanently; when that initializer's scalar is defined by a
//! `BooleanConstant`/`IntegerConstant` in the same function the observation
//! folds to it. Variant establishment: at a lone `Case` path the producer is
//! an `EstablishScalarCase` whose `result_case` matches the observed case, so
//! its scalar case-field initializer fixes the payload field's value the same
//! way. Declared bound: the observed field's declared type is a
//! `BoundedInteger` whose inclusive bound closes over exactly one value, so
//! every inhabitant of the position holds that value independently of how the
//! place arrived — parameters, block parameters, results, and
//! non-establishing producers all qualify, at any resolvable path depth. The
//! observation specializes into the proven constant at the same node: the
//! result keeps its value identity, the node keeps its
//! `PsiProvenance::Operation` custody and fuel settlement, and successors,
//! definitions, uses, and ownership events are unchanged.
//!
//! Reads whose field the unit cannot prove — an unestablished or
//! non-constant-initialized field on a multi-valued bound, a `Case` path on a
//! place established under a different case, an `Erased` or structural field
//! position, or a path that fails to resolve — are not covered. Forwarding an
//! initializer's non-constant scalar to the read's uses would need
//! operand-substitution machinery this family deliberately does not carry.
//! Only machines absent from the authenticated Terminal-cycle component
//! roster are eligible: a machine containing a verified cyclic component is
//! frozen byte-exact under `validate_frozen_component_blocks`. Proposal,
//! independent validation, and application are separate boundaries; the
//! candidate pins the exact input and output revision identities, the
//! specialized place, the place's establishing witness, and every folded
//! observation's site, custody identity, path, field, proof basis, and
//! verdict, so replay rejects forged or stale rows by recomputation rather
//! than trust.

use optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};
use optimization_unit::{
    FoldedFieldValue, NodeLocation, ProvenanceDisposition, ProvenanceRewrite,
    PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite, PsiTransformationLedger,
    PsiTransformationRecord, recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, ScalarType, StructuralPlaceKind};

use abstract_operations::AbstractOperation as O;

use crate::VerifiedPsiOptimizationSession;

mod admission;
mod apply;
mod model;
pub(crate) mod propose;
pub(crate) mod validate;

pub use model::{
    AppliedFieldValueSpecialization, FieldValueSpecializationCandidate,
    FieldValueSpecializationError, ResolvedFieldValue, ValidatedFieldValueSpecialization,
};
use model::{FieldValuePlan, candidate_identity};

pub fn propose_field_value_specializations(
    session: &VerifiedPsiOptimizationSession,
    candidate_limit: u64,
) -> Result<Vec<FieldValueSpecializationCandidate>, FieldValueSpecializationError> {
    propose::all(session, candidate_limit)
}

pub fn validate_field_value_specialization(
    session: &VerifiedPsiOptimizationSession,
    candidate: &FieldValueSpecializationCandidate,
) -> Result<ValidatedFieldValueSpecialization, FieldValueSpecializationError> {
    validate::candidate(session, candidate)
}

pub fn apply_field_value_specialization(
    session: VerifiedPsiOptimizationSession,
    validated: ValidatedFieldValueSpecialization,
) -> Result<AppliedFieldValueSpecialization, FieldValueSpecializationError> {
    apply::validated(session, validated)
}

fn rule_identity() -> OptimizationRuleIdentity {
    OptimizationRuleIdentity::from_canonical_bytes(b"omega.psi-rule.field-value-specialization.v1")
}

fn validator_identity() -> OptimizationValidatorIdentity {
    OptimizationValidatorIdentity::from_canonical_bytes(
        b"omega.psi-validator.field-value-specialization.v1",
    )
}

#[cfg(test)]
mod tests;
