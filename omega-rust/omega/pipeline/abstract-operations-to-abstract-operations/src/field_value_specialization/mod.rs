//! Optimizer module role: executable entrance. Proven field-value specialization boundary.
//!
//! One bounded specialization family: a `BooleanStructuralField` or
//! `IntegerStructuralField` observation whose `source` place carries a stored
//! field value the unit itself proves. Three proofs qualify. Record
//! establishment: the place is declared `StructuralPlaceKind::OperationResult`
//! and its producer node in the same function is an `EstablishRecord` — the
//! operation-result place is assigned exactly once by its producer and no
//! operation can rewrite it, so the record field initializer fixes the field's
//! stored value permanently. Variant establishment: at a lone `Case` path the
//! producer is an `EstablishScalarCase` whose `result_case` matches the
//! observed case, so its scalar case-field initializer fixes the payload
//! field's value the same way. Declared bound: the observed field's declared
//! type is a `BoundedInteger` whose inclusive bound closes over exactly one
//! value, so every inhabitant of the position holds that value independently
//! of how the place arrived — parameters, block parameters, results, and
//! non-establishing producers all qualify, at any resolvable path depth.
//!
//! What the proven initializer admits depends on its shape. A
//! `BooleanConstant`/`IntegerConstant` in the same function — or a bound
//! singleton — is a `Constant` resolution: the observation folds in place,
//! the result keeps its value identity, and the node keeps its
//! `PsiProvenance::Operation` custody, fuel settlement, successors,
//! definitions, uses, and ownership events. A nonconstant scalar initializer
//! is a `Forward` resolution when the substitution can retire the read
//! exactly: the initializer must be defined in the same function at the
//! read's own scalar type, dominate every use site, and every scalar-operand
//! use of the read's result must sit in a position the substitution lane
//! rewrites — establishments, scalar stores, ordinary calls, arithmetic,
//! comparisons, shifts, jump and conditional bindings, returns, and atomic
//! operands — while the observation node itself must be cleanly removable.
//! Forwarding then rebinds those uses, retires the observation node, and
//! fuses the read's provenance and fuel settlement into the node inheriting
//! the vacated index; the function's derived metadata restamps from
//! operation shape.
//!
//! Reads whose field the unit cannot prove — an unestablished field on a
//! multi-valued bound, a `Case` path on a place established under a different
//! case, an `Erased` or structural field position, or a path that fails to
//! resolve — are not covered, and neither is a nonconstant initializer whose
//! uses escape the substitution lane or which fails to dominate a use.
//! Only machines absent from the authenticated Terminal-cycle component
//! roster are eligible: a machine containing a verified cyclic component is
//! frozen byte-exact under `validate_frozen_component_blocks`. Proposal,
//! independent validation, and application are separate boundaries; the
//! candidate pins the exact input and output revision identities, the
//! specialized place, the place's establishing witness, and every resolved
//! observation's site, custody identity, path, field, proof basis, and
//! resolution, so replay rejects forged or stale rows by recomputation rather
//! than trust.

use optimization_core::{
    OptimizationCandidateIdentity, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};
use optimization_unit::{
    FieldValueResolution, FoldedFieldValue, NodeLocation, ProvenanceDisposition, ProvenanceRewrite,
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

pub(crate) fn validate_field_value_specialization(
    session: &VerifiedPsiOptimizationSession,
    candidate: &FieldValueSpecializationCandidate,
) -> Result<ValidatedFieldValueSpecialization, FieldValueSpecializationError> {
    validate::candidate(session, candidate)
}

pub(crate) fn apply_field_value_specialization(
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
