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
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerCarrier, IntegerSign, StructuralFieldId, ValueId,
};

mod admission;
mod apply;
pub(crate) mod propose;
pub(crate) mod validate;

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

/// One admitted field observation: the read node's exact site, operation
/// custody identity, result value, observed place, canonical path, field,
/// proof witness, and resolution. `producer` is `Some` when the row's proof
/// draws on the place's establishing operation — an `EstablishRecord` at an
/// empty path or an `EstablishScalarCase` whose `result_case` matches at a
/// lone `Case` path — and `None` when the field's declared `BoundedInteger`
/// bound closes over exactly one value. A `Constant` resolution folds the
/// read in place; a `Forward` resolution substitutes the proven nonconstant
/// initializer at every use of the read's result and retires the observation
/// node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFieldValue {
    pub(crate) site: NodeLocation,
    pub(crate) psi_operation: OperationId,
    pub(crate) result: ValueId,
    pub(crate) source: PlaceId,
    pub(crate) path: Vec<CanonicalStructuralPathSegment>,
    pub(crate) field: StructuralFieldId,
    pub(crate) producer: Option<OperationId>,
    pub(crate) resolution: FieldValueResolution,
}

impl ResolvedFieldValue {
    pub const fn site(&self) -> NodeLocation {
        self.site
    }

    pub const fn psi_operation(&self) -> OperationId {
        self.psi_operation
    }

    pub const fn result(&self) -> semantic_vocabulary::ValueId {
        self.result
    }

    pub const fn source(&self) -> PlaceId {
        self.source
    }

    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }

    pub const fn field(&self) -> StructuralFieldId {
        self.field
    }

    pub const fn producer(&self) -> Option<OperationId> {
        self.producer
    }

    pub const fn resolution(&self) -> &FieldValueResolution {
        &self.resolution
    }
}

/// A published specialization candidate: the exact input and output unit
/// revisions, the machine and place every folded observation resolves
/// against, the place's establishing operation witness when one exists, and
/// the complete canonical row set — everything an independent validator
/// needs to recompute the admission rather than trust it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldValueSpecializationCandidate {
    pub(crate) identity: OptimizationCandidateIdentity,
    pub(crate) input: OptimizationUnitIdentity,
    pub(crate) output: OptimizationUnitIdentity,
    pub(crate) machine: MachineId,
    pub(crate) place: PlaceId,
    pub(crate) producer: Option<OperationId>,
    pub(crate) reads: Vec<ResolvedFieldValue>,
}

impl FieldValueSpecializationCandidate {
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

    pub const fn place(&self) -> PlaceId {
        self.place
    }

    pub const fn producer(&self) -> Option<OperationId> {
        self.producer
    }

    pub fn reads(&self) -> &[ResolvedFieldValue] {
        &self.reads
    }
}

/// A candidate that survived independent validation: the replayed admission
/// equals the declared rows exactly, and the reconstructed provenance names
/// the custody the folded observations carry.
#[derive(Debug)]
pub struct ValidatedFieldValueSpecialization {
    pub(crate) candidate: FieldValueSpecializationCandidate,
    pub(crate) output: PsiOptimizationUnit,
    pub(crate) provenance: Vec<ProvenanceRewrite>,
}

impl ValidatedFieldValueSpecialization {
    pub const fn candidate(&self) -> &FieldValueSpecializationCandidate {
        &self.candidate
    }

    pub fn provenance(&self) -> &[ProvenanceRewrite] {
        &self.provenance
    }
}

/// An applied specialization: the transformed session plus the ledger row
/// carrying rule, candidate, validator, revision, and provenance custody.
#[derive(Debug)]
pub struct AppliedFieldValueSpecialization {
    pub(crate) session: VerifiedPsiOptimizationSession,
    pub(crate) candidate: FieldValueSpecializationCandidate,
    pub(crate) ledger: PsiTransformationLedger,
}

impl AppliedFieldValueSpecialization {
    pub const fn session(&self) -> &VerifiedPsiOptimizationSession {
        &self.session
    }

    pub const fn candidate(&self) -> &FieldValueSpecializationCandidate {
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
pub enum FieldValueSpecializationError {
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
    /// field-value proof — neither an establishing producer with a
    /// constant-defined initializer nor a declared singleton bound at any
    /// observed position — or it no longer exists.
    UnknownPlace,
    /// The place exists but currently has no foldable field read left.
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

impl std::fmt::Display for FieldValueSpecializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "field-value specialization failure: {self:?}")
    }
}

impl std::error::Error for FieldValueSpecializationError {}

/// The independently derived specialization plan for one place: every
/// `BooleanStructuralField`/`IntegerStructuralField` observing it whose
/// stored value the per-site basis proves, sorted by node location.
/// `producer` records the place's establishing operation-result witness — an
/// `EstablishRecord` or `EstablishScalarCase` — when one exists; each row
/// still carries its own basis. Proposal and validation both recompute this
/// plan; the candidate is accepted only when its claimed rows equal the
/// replayed plan exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldValuePlan {
    pub(crate) machine: MachineId,
    pub(crate) place: PlaceId,
    pub(crate) producer: Option<OperationId>,
    pub(crate) reads: Vec<ResolvedFieldValue>,
}

fn candidate_identity(
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    machine: MachineId,
    place: PlaceId,
    reads: &[ResolvedFieldValue],
) -> OptimizationCandidateIdentity {
    let mut canonical = b"omega.psi.field-value-specialization-candidate.v1".to_vec();
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&output.bytes());
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&place.get().to_le_bytes());
    canonical.extend_from_slice(
        &u64::try_from(reads.len())
            .expect("read count fits u64")
            .to_le_bytes(),
    );
    for row in reads {
        canonical.extend_from_slice(&row.site.machine.get().to_le_bytes());
        canonical.extend_from_slice(&row.site.block.get().to_le_bytes());
        canonical.extend_from_slice(&row.site.node.to_le_bytes());
        canonical.extend_from_slice(&row.psi_operation.get().to_le_bytes());
        canonical.extend_from_slice(&row.result.get().to_le_bytes());
        canonical.extend_from_slice(&row.source.get().to_le_bytes());
        canonical.extend_from_slice(
            &u64::try_from(row.path.len())
                .expect("path length fits u64")
                .to_le_bytes(),
        );
        for segment in &row.path {
            match segment {
                CanonicalStructuralPathSegment::Field(identity) => {
                    canonical.push(1);
                    canonical.extend_from_slice(&identity.get().to_le_bytes());
                }
                CanonicalStructuralPathSegment::FixedIndex(index) => {
                    canonical.push(2);
                    canonical.extend_from_slice(&index.to_le_bytes());
                }
                CanonicalStructuralPathSegment::Case(identity) => {
                    canonical.push(3);
                    canonical.extend_from_slice(&identity.get().to_le_bytes());
                }
            }
        }
        canonical.extend_from_slice(&row.field.get().to_le_bytes());
        canonical.extend_from_slice(
            &row.producer
                .map(|producer| producer.get())
                .unwrap_or(0)
                .to_le_bytes(),
        );
        match &row.resolution {
            FieldValueResolution::Constant(FoldedFieldValue::Boolean(constant)) => {
                canonical.push(1);
                canonical.push(u8::from(*constant));
            }
            FieldValueResolution::Constant(FoldedFieldValue::Integer(constant)) => {
                canonical.push(2);
                match constant {
                    semantic_vocabulary::IntegerValue::Signed(value) => {
                        canonical.push(1);
                        canonical.extend_from_slice(&value.to_le_bytes());
                    }
                    semantic_vocabulary::IntegerValue::Unsigned(value) => {
                        canonical.push(2);
                        canonical.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
            FieldValueResolution::Forward(forwarded) => {
                canonical.push(3);
                canonical.extend_from_slice(&forwarded.initializer.get().to_le_bytes());
                encode_scalar_type(forwarded.scalar_type, &mut canonical);
                canonical.extend_from_slice(
                    &u64::try_from(forwarded.uses.len())
                        .expect("use count fits u64")
                        .to_le_bytes(),
                );
                for use_site in &forwarded.uses {
                    canonical.extend_from_slice(&use_site.machine.get().to_le_bytes());
                    canonical.extend_from_slice(&use_site.block.get().to_le_bytes());
                    canonical.extend_from_slice(&use_site.node.to_le_bytes());
                }
            }
        }
    }
    OptimizationCandidateIdentity::from_canonical_bytes(&canonical)
}

/// Appends `scalar_type`'s canonical encoding — the same fixed-width scheme
/// the rewrite codec uses — to `canonical`.
fn encode_scalar_type(scalar_type: ScalarType, canonical: &mut Vec<u8>) {
    match scalar_type {
        ScalarType::Boolean => canonical.push(1),
        ScalarType::Integer(integer) => {
            canonical.push(2);
            canonical.push(match integer.carrier() {
                IntegerCarrier::Fixed => 1,
                IntegerCarrier::Address => 2,
            });
            canonical.push(match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            });
            canonical.extend_from_slice(&integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            canonical.push(3);
            canonical.push(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 1,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 2,
            });
        }
    }
}
