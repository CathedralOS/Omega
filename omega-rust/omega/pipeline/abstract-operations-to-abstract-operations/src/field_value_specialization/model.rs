//! Optimizer module role: model leaf. Field-value specialization candidate and row types.

use super::{
    MachineId, NodeLocation, OperationId, OptimizationCandidateIdentity, OptimizationUnitIdentity,
    PlaceId, ProvenanceRewrite, PsiOptimizationUnit, PsiTransformationLedger,
    VerifiedPsiOptimizationSession,
};
use optimization_unit::{FieldValueResolution, FoldedFieldValue};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerCarrier, IntegerSign, ScalarType, StructuralFieldId,
    ValueId,
};

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

pub(super) fn candidate_identity(
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
