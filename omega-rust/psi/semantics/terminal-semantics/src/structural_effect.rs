//! Exact structural/effect leaf schemas and their local observations.

use semantic_vocabulary::{
    IntegerSign, IntegerType, ObligationId, PlaceId, Proposition, ScalarTerm, ScalarType,
    ServiceId, StructuralCaseId, StructuralCaseSubject, StructuralFieldId, ValueId,
};
use terminal_psi::{Operation, OperationKind, OperationResult};

mod byte_extent;
pub use byte_extent::{literal_length_equation, subslice_length_equation};

#[cfg(test)]
mod subslice_tests;

#[cfg(test)]
mod case_membership_tests;

use super::{OperationSemanticError, OperationSemanticTag};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectResultShape {
    Scalar,
    Boolean,
    Integer,
    ByteCount,
    Byte,
    Structural,
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectCustody {
    ExactReferenceSource,
    ExactReferenceCarrier,
    ExactPrimitiveLocal,
    ExactReadablePrimitiveRoot,
    ExactReadableSumRoot,
    ExactWriteOnlyPrimitiveRoot,
    ExactStructuralScalarField,
    ExactStructuralByteSequenceField,
    ExactByteSequenceLiteral,
    ExactImmutableByteView,
    ExactMutableByteView,
    ExactByteViewMetadata,
    ExactLiveBooleanField,
    ExactLiveIntegerField,
    ExactPublishedService,
    ExactEmptyAffineLocal,
    ExactRecord,
    ExactScalarCase,
    ExactScalarArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectAction {
    EstablishReference,
    ReleaseReference,
    EstablishPrimitiveLocal,
    ReadPrimitive,
    ObserveCaseMembership,
    StorePrimitive,
    StoreScalarField,
    StoreByteSequenceField,
    ReadByteSequenceFieldLength,
    StoreByteSequenceFieldByte,
    EstablishByteSequencePlace,
    ReadByteSequenceLength,
    ReadByteSequence,
    WriteByteSequence,
    EstablishByteSequenceSubslice,
    ReadBooleanField,
    ReadIntegerField,
    EmitPortWrite,
    EstablishAffinePlace,
    EstablishRecord,
    EstablishScalarCase,
    EstablishScalarArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectExternalEffect {
    None,
    PortWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectFuelPolicy {
    ConsumeOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectGoalShape {
    None,
    ByteIndexInBounds,
    ByteRangeInBounds,
    /// Requires the destination field's independently resolved capacity.
    ByteLengthWithinFieldCapacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectFrontierPolicy {
    /// Capture an existing loan; add carrier ownership without moving its referent.
    CapturesLoanAndAddsReferenceCarrier,
    /// End carrier custody and its loan, without disposing the referent.
    ReleasesReferenceCarrier,
    /// Consume exact whole owned children before publishing the complete parent.
    TransfersOwnedChildrenAndAddsOwnedPlace,
    RequiresAndKeepsWriteOnlyPrimitivePlace,
    RequiresAndKeepsStructuralPlace,
    RequiresViewAndEstablishesBorrowedView,
    AddsUnrestrictedPlace,
    RequiresAndKeepsAffinePlace,
    KeepsPlaceFrontier,
    AddsAffinePlace,
    AddsOwnedPlace,
}

/// One structural/effect leaf row. This cohort remains separate from scalar
/// denotation and call composition because place custody, external effects,
/// and frontier transitions are independent semantic axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralEffectLeafSchema {
    result: StructuralEffectResultShape,
    custody: StructuralEffectCustody,
    action: StructuralEffectAction,
    external_effect: StructuralEffectExternalEffect,
    fuel: StructuralEffectFuelPolicy,
    goal: StructuralEffectGoalShape,
    frontier: StructuralEffectFrontierPolicy,
}

impl StructuralEffectLeafSchema {
    pub const fn result(self) -> StructuralEffectResultShape {
        self.result
    }

    pub const fn custody(self) -> StructuralEffectCustody {
        self.custody
    }

    pub const fn action(self) -> StructuralEffectAction {
        self.action
    }

    pub const fn external_effect(self) -> StructuralEffectExternalEffect {
        self.external_effect
    }

    pub const fn fuel(self) -> StructuralEffectFuelPolicy {
        self.fuel
    }

    pub const fn goal(self) -> StructuralEffectGoalShape {
        self.goal
    }

    pub const fn frontier(self) -> StructuralEffectFrontierPolicy {
        self.frontier
    }
}

const fn structural_effect_leaf(
    result: StructuralEffectResultShape,
    custody: StructuralEffectCustody,
    action: StructuralEffectAction,
    external_effect: StructuralEffectExternalEffect,
    frontier: StructuralEffectFrontierPolicy,
) -> StructuralEffectLeafSchema {
    StructuralEffectLeafSchema {
        result,
        custody,
        action,
        external_effect,
        fuel: StructuralEffectFuelPolicy::ConsumeOne,
        goal: match action {
            StructuralEffectAction::StoreByteSequenceField => {
                StructuralEffectGoalShape::ByteLengthWithinFieldCapacity
            }
            StructuralEffectAction::ReadByteSequence
            | StructuralEffectAction::WriteByteSequence
            | StructuralEffectAction::StoreByteSequenceFieldByte => {
                StructuralEffectGoalShape::ByteIndexInBounds
            }
            StructuralEffectAction::EstablishByteSequenceSubslice => {
                StructuralEffectGoalShape::ByteRangeInBounds
            }
            _ => StructuralEffectGoalShape::None,
        },
        frontier,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralEffectSemanticRow {
    tag: OperationSemanticTag,
    schema: StructuralEffectLeafSchema,
}

impl StructuralEffectSemanticRow {
    pub const ALL: [Self; 22] = [
        Self {
            tag: OperationSemanticTag::EstablishReference,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Structural,
                StructuralEffectCustody::ExactReferenceSource,
                StructuralEffectAction::EstablishReference,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::CapturesLoanAndAddsReferenceCarrier,
            ),
        },
        Self {
            tag: OperationSemanticTag::ReleaseReference,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Unit,
                StructuralEffectCustody::ExactReferenceCarrier,
                StructuralEffectAction::ReleaseReference,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::ReleasesReferenceCarrier,
            ),
        },
        Self {
            tag: OperationSemanticTag::EstablishScalarArray,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Structural,
                StructuralEffectCustody::ExactScalarArray,
                StructuralEffectAction::EstablishScalarArray,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::AddsUnrestrictedPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::EstablishPrimitiveLocal,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Structural,
                StructuralEffectCustody::ExactPrimitiveLocal,
                StructuralEffectAction::EstablishPrimitiveLocal,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::AddsUnrestrictedPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::PrimitiveScalarRead,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Scalar,
                StructuralEffectCustody::ExactReadablePrimitiveRoot,
                StructuralEffectAction::ReadPrimitive,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::StructuralCaseMembership,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Boolean,
                StructuralEffectCustody::ExactReadableSumRoot,
                StructuralEffectAction::ObserveCaseMembership,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::StructuralByteSequenceFieldLength,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::ByteCount,
                StructuralEffectCustody::ExactStructuralByteSequenceField,
                StructuralEffectAction::ReadByteSequenceFieldLength,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::StructuralByteSequenceFieldByteStore,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Unit,
                StructuralEffectCustody::ExactStructuralByteSequenceField,
                StructuralEffectAction::StoreByteSequenceFieldByte,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::StructuralByteSequenceFieldStore,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Unit,
                StructuralEffectCustody::ExactStructuralByteSequenceField,
                StructuralEffectAction::StoreByteSequenceField,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::ByteSequenceSubslice,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Structural,
                StructuralEffectCustody::ExactImmutableByteView,
                StructuralEffectAction::EstablishByteSequenceSubslice,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresViewAndEstablishesBorrowedView,
            ),
        },
        Self {
            tag: OperationSemanticTag::ByteSequenceRead,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Byte,
                StructuralEffectCustody::ExactImmutableByteView,
                StructuralEffectAction::ReadByteSequence,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::ByteSequenceWrite,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Unit,
                StructuralEffectCustody::ExactMutableByteView,
                StructuralEffectAction::WriteByteSequence,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::ByteSequenceLength,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::ByteCount,
                StructuralEffectCustody::ExactByteViewMetadata,
                StructuralEffectAction::ReadByteSequenceLength,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::WriteOnlyPrimitiveStore,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Unit,
                StructuralEffectCustody::ExactWriteOnlyPrimitiveRoot,
                StructuralEffectAction::StorePrimitive,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsWriteOnlyPrimitivePlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::StructuralScalarFieldStore,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Unit,
                StructuralEffectCustody::ExactStructuralScalarField,
                StructuralEffectAction::StoreScalarField,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::EstablishScalarCase,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Structural,
                StructuralEffectCustody::ExactScalarCase,
                StructuralEffectAction::EstablishScalarCase,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::AddsOwnedPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::EstablishByteSequenceLiteral,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Unit,
                StructuralEffectCustody::ExactByteSequenceLiteral,
                StructuralEffectAction::EstablishByteSequencePlace,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::AddsUnrestrictedPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::BooleanStructuralField,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Boolean,
                StructuralEffectCustody::ExactLiveBooleanField,
                StructuralEffectAction::ReadBooleanField,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsAffinePlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::IntegerStructuralField,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Integer,
                StructuralEffectCustody::ExactLiveIntegerField,
                StructuralEffectAction::ReadIntegerField,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::PortWrite,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Unit,
                StructuralEffectCustody::ExactPublishedService,
                StructuralEffectAction::EmitPortWrite,
                StructuralEffectExternalEffect::PortWrite,
                StructuralEffectFrontierPolicy::KeepsPlaceFrontier,
            ),
        },
        Self {
            tag: OperationSemanticTag::EstablishTrivialAffineLocal,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Unit,
                StructuralEffectCustody::ExactEmptyAffineLocal,
                StructuralEffectAction::EstablishAffinePlace,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::AddsAffinePlace,
            ),
        },
        Self {
            tag: OperationSemanticTag::EstablishRecord,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Structural,
                StructuralEffectCustody::ExactRecord,
                StructuralEffectAction::EstablishRecord,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::TransfersOwnedChildrenAndAddsOwnedPlace,
            ),
        },
    ];

    pub const fn tag(self) -> OperationSemanticTag {
        self.tag
    }

    pub const fn schema(self) -> StructuralEffectLeafSchema {
        self.schema
    }
}

const fn is_structural_effect_tag(tag: OperationSemanticTag) -> bool {
    matches!(
        tag,
        OperationSemanticTag::EstablishPrimitiveLocal
            | OperationSemanticTag::EstablishReference
            | OperationSemanticTag::ReleaseReference
            | OperationSemanticTag::PrimitiveScalarRead
            | OperationSemanticTag::StructuralCaseMembership
            | OperationSemanticTag::WriteOnlyPrimitiveStore
            | OperationSemanticTag::StructuralScalarFieldStore
            | OperationSemanticTag::StructuralByteSequenceFieldStore
            | OperationSemanticTag::StructuralByteSequenceFieldLength
            | OperationSemanticTag::StructuralByteSequenceFieldByteStore
            | OperationSemanticTag::EstablishScalarCase
            | OperationSemanticTag::EstablishScalarArray
            | OperationSemanticTag::EstablishByteSequenceLiteral
            | OperationSemanticTag::ByteSequenceLength
            | OperationSemanticTag::ByteSequenceWrite
            | OperationSemanticTag::ByteSequenceRead
            | OperationSemanticTag::ByteSequenceSubslice
            | OperationSemanticTag::BooleanStructuralField
            | OperationSemanticTag::IntegerStructuralField
            | OperationSemanticTag::PortWrite
            | OperationSemanticTag::EstablishTrivialAffineLocal
            | OperationSemanticTag::EstablishRecord
    )
}

pub fn exact_structural_effect_semantic_row_in(
    tag: OperationSemanticTag,
    rows: &[StructuralEffectSemanticRow],
) -> Result<Option<&StructuralEffectSemanticRow>, OperationSemanticError> {
    if !is_structural_effect_tag(tag) {
        return Ok(None);
    }
    let mut matches = rows.iter().filter(|row| row.tag == tag);
    let row = matches
        .next()
        .ok_or(OperationSemanticError::MissingStructuralEffectRow(tag))?;
    if matches.next().is_some() {
        return Err(OperationSemanticError::DuplicateStructuralEffectRow(tag));
    }
    Ok(Some(row))
}

pub fn structural_effect_semantic_row(
    operation: &OperationKind,
) -> Result<Option<&'static StructuralEffectSemanticRow>, OperationSemanticError> {
    validate_structural_effect_semantic_rows(&StructuralEffectSemanticRow::ALL)?;
    exact_structural_effect_semantic_row_in(
        OperationSemanticTag::for_operation(operation),
        &StructuralEffectSemanticRow::ALL,
    )
}

pub fn validate_structural_effect_semantic_rows(
    rows: &[StructuralEffectSemanticRow],
) -> Result<(), OperationSemanticError> {
    for row in rows {
        if !is_structural_effect_tag(row.tag) {
            return Err(OperationSemanticError::UnexpectedStructuralEffectRow(
                row.tag,
            ));
        }
    }
    for tag in [
        OperationSemanticTag::EstablishReference,
        OperationSemanticTag::ReleaseReference,
        OperationSemanticTag::EstablishPrimitiveLocal,
        OperationSemanticTag::PrimitiveScalarRead,
        OperationSemanticTag::StructuralCaseMembership,
        OperationSemanticTag::WriteOnlyPrimitiveStore,
        OperationSemanticTag::StructuralScalarFieldStore,
        OperationSemanticTag::StructuralByteSequenceFieldStore,
        OperationSemanticTag::StructuralByteSequenceFieldLength,
        OperationSemanticTag::StructuralByteSequenceFieldByteStore,
        OperationSemanticTag::EstablishScalarCase,
        OperationSemanticTag::EstablishScalarArray,
        OperationSemanticTag::EstablishByteSequenceLiteral,
        OperationSemanticTag::ByteSequenceLength,
        OperationSemanticTag::ByteSequenceRead,
        OperationSemanticTag::ByteSequenceWrite,
        OperationSemanticTag::ByteSequenceSubslice,
        OperationSemanticTag::BooleanStructuralField,
        OperationSemanticTag::IntegerStructuralField,
        OperationSemanticTag::PortWrite,
        OperationSemanticTag::EstablishTrivialAffineLocal,
        OperationSemanticTag::EstablishRecord,
    ] {
        let row = exact_structural_effect_semantic_row_in(tag, rows)?
            .expect("the requested tag belongs to the structural/effect cohort");
        validate_structural_effect_schema(tag, row.schema)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralEffectObservation {
    ReferenceEstablished {
        source: terminal_psi::StructuralArgument,
        destination: PlaceId,
    },
    ReferenceReleased {
        source: PlaceId,
    },
    ScalarArrayEstablished {
        destination: PlaceId,
        elements: Vec<ValueId>,
    },
    PrimitiveLocalEstablished {
        destination: PlaceId,
        value: ValueId,
    },
    PrimitiveRead {
        source: PlaceId,
        result: ValueId,
    },
    /// Captures one tag observation without a reusable current-storage fact.
    CaseMembershipObserved {
        source: PlaceId,
        case: StructuralCaseId,
        result: ValueId,
    },
    ByteSequenceFieldLengthRead {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: StructuralFieldId,
        result: ValueId,
    },
    ByteSequenceFieldByteStored {
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: StructuralFieldId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    ByteSequenceFieldStored {
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: StructuralFieldId,
        source: PlaceId,
        length: ValueId,
        obligation: ObligationId,
    },
    PrimitiveStored {
        destination: PlaceId,
        value: semantic_vocabulary::ValueId,
    },
    ScalarFieldStored {
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: StructuralFieldId,
        value: ValueId,
    },
    ByteSequencePlaceEstablished {
        destination: PlaceId,
    },
    ByteSequenceLengthRead {
        source: PlaceId,
        result: ValueId,
    },
    /// Read from the exact immutable view after discharging index < length.
    /// Length-source provenance and operand dominance are artifact validation,
    /// not extra producer-supplied scalar equations.
    ByteSequenceRead {
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        result: ValueId,
        obligation: ObligationId,
    },
    ByteSequenceWrite {
        destination: PlaceId,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    ByteSequenceSubslice {
        source: PlaceId,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        destination: PlaceId,
        obligation: ObligationId,
    },
    BooleanFieldEquation(Proposition),
    IntegerFieldRead {
        source: PlaceId,
        field: StructuralFieldId,
        result: ValueId,
    },
    PortWrite {
        service: ServiceId,
        port: u16,
        value: u8,
    },
    AffinePlaceEstablished {
        destination: PlaceId,
    },
    RecordEstablished {
        destination: PlaceId,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    ScalarCaseEstablished {
        membership: Proposition,
        fields: Vec<terminal_psi::ScalarCaseField>,
    },
}

impl StructuralEffectObservation {
    /// Operand-only canonical goals. Contextual field-capacity goals require
    /// the verifier's independently resolved module declaration instead.
    pub fn canonical_obligation(&self) -> Option<(ObligationId, Proposition)> {
        let scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
        let value = |identity| ScalarTerm::value(identity, scalar_type);
        match self {
            // This obligation must be reconstructed with the module's exact
            // destination field capacity before ordinary leaf processing.
            Self::ByteSequenceFieldStored { .. } => None,
            Self::ByteSequenceFieldByteStored {
                index,
                length,
                obligation,
                ..
            }
            | Self::ByteSequenceWrite {
                index,
                length,
                obligation,
                ..
            }
            | Self::ByteSequenceRead {
                index,
                length,
                obligation,
                ..
            } => Some((
                *obligation,
                Proposition::LessThan(value(*index), value(*length)),
            )),
            Self::ByteSequenceSubslice {
                start,
                end,
                length,
                obligation,
                ..
            } => Some((
                *obligation,
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(value(*start), value(*end)),
                    Proposition::LessOrEqual(value(*end), value(*length)),
                ]),
            )),
            _ => None,
        }
    }

    pub fn local_equation(&self) -> Option<&Proposition> {
        match self {
            Self::BooleanFieldEquation(proposition)
            | Self::ScalarCaseEstablished {
                membership: proposition,
                ..
            } => Some(proposition),
            Self::RecordEstablished { .. }
            | Self::ReferenceEstablished { .. }
            | Self::ReferenceReleased { .. }
            | Self::PrimitiveLocalEstablished { .. }
            | Self::ScalarArrayEstablished { .. }
            | Self::PrimitiveRead { .. }
            | Self::CaseMembershipObserved { .. }
            | Self::PrimitiveStored { .. }
            | Self::ByteSequenceFieldLengthRead { .. }
            | Self::ByteSequenceFieldByteStored { .. }
            | Self::ByteSequenceFieldStored { .. }
            | Self::ScalarFieldStored { .. }
            | Self::ByteSequencePlaceEstablished { .. }
            | Self::ByteSequenceLengthRead { .. }
            | Self::ByteSequenceWrite { .. }
            | Self::ByteSequenceRead { .. }
            | Self::ByteSequenceSubslice { .. }
            | Self::IntegerFieldRead { .. }
            | Self::PortWrite { .. }
            | Self::AffinePlaceEstablished { .. } => None,
        }
    }
}

fn validate_structural_effect_schema(
    tag: OperationSemanticTag,
    schema: StructuralEffectLeafSchema,
) -> Result<(), OperationSemanticError> {
    let action_tag = match schema.action {
        StructuralEffectAction::EstablishReference => OperationSemanticTag::EstablishReference,
        StructuralEffectAction::ReleaseReference => OperationSemanticTag::ReleaseReference,
        StructuralEffectAction::EstablishPrimitiveLocal => {
            OperationSemanticTag::EstablishPrimitiveLocal
        }
        StructuralEffectAction::ReadPrimitive => OperationSemanticTag::PrimitiveScalarRead,
        StructuralEffectAction::ObserveCaseMembership => {
            OperationSemanticTag::StructuralCaseMembership
        }
        StructuralEffectAction::ReadByteSequenceFieldLength => {
            OperationSemanticTag::StructuralByteSequenceFieldLength
        }
        StructuralEffectAction::StoreByteSequenceFieldByte => {
            OperationSemanticTag::StructuralByteSequenceFieldByteStore
        }
        StructuralEffectAction::StoreByteSequenceField => {
            OperationSemanticTag::StructuralByteSequenceFieldStore
        }
        StructuralEffectAction::EstablishByteSequenceSubslice => {
            OperationSemanticTag::ByteSequenceSubslice
        }
        StructuralEffectAction::WriteByteSequence => OperationSemanticTag::ByteSequenceWrite,
        StructuralEffectAction::ReadByteSequence => OperationSemanticTag::ByteSequenceRead,
        StructuralEffectAction::ReadByteSequenceLength => OperationSemanticTag::ByteSequenceLength,
        StructuralEffectAction::StorePrimitive => OperationSemanticTag::WriteOnlyPrimitiveStore,
        StructuralEffectAction::StoreScalarField => {
            OperationSemanticTag::StructuralScalarFieldStore
        }
        StructuralEffectAction::EstablishByteSequencePlace => {
            OperationSemanticTag::EstablishByteSequenceLiteral
        }
        StructuralEffectAction::ReadBooleanField => OperationSemanticTag::BooleanStructuralField,
        StructuralEffectAction::ReadIntegerField => OperationSemanticTag::IntegerStructuralField,
        StructuralEffectAction::EmitPortWrite => OperationSemanticTag::PortWrite,
        StructuralEffectAction::EstablishAffinePlace => {
            OperationSemanticTag::EstablishTrivialAffineLocal
        }
        StructuralEffectAction::EstablishRecord => OperationSemanticTag::EstablishRecord,
        StructuralEffectAction::EstablishScalarCase => OperationSemanticTag::EstablishScalarCase,
        StructuralEffectAction::EstablishScalarArray => OperationSemanticTag::EstablishScalarArray,
    };
    let expected_goal = match schema.action {
        StructuralEffectAction::StoreByteSequenceField => {
            StructuralEffectGoalShape::ByteLengthWithinFieldCapacity
        }
        StructuralEffectAction::EstablishByteSequenceSubslice => {
            StructuralEffectGoalShape::ByteRangeInBounds
        }
        StructuralEffectAction::ReadByteSequence
        | StructuralEffectAction::WriteByteSequence
        | StructuralEffectAction::StoreByteSequenceFieldByte => {
            StructuralEffectGoalShape::ByteIndexInBounds
        }
        _ => StructuralEffectGoalShape::None,
    };
    let valid = action_tag == tag
        && schema.goal == expected_goal
        && match schema.action {
            StructuralEffectAction::EstablishReference => {
                schema.result == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactReferenceSource
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::CapturesLoanAndAddsReferenceCarrier
            }
            StructuralEffectAction::ReleaseReference => {
                schema.result == StructuralEffectResultShape::Unit
                    && schema.custody == StructuralEffectCustody::ExactReferenceCarrier
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier == StructuralEffectFrontierPolicy::ReleasesReferenceCarrier
            }
            StructuralEffectAction::EstablishPrimitiveLocal => {
                schema.result == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactPrimitiveLocal
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier == StructuralEffectFrontierPolicy::AddsUnrestrictedPlace
            }
            StructuralEffectAction::ReadPrimitive => {
                schema.result == StructuralEffectResultShape::Scalar
                    && schema.custody == StructuralEffectCustody::ExactReadablePrimitiveRoot
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
            }
            StructuralEffectAction::ObserveCaseMembership => {
                schema.result == StructuralEffectResultShape::Boolean
                    && schema.custody == StructuralEffectCustody::ExactReadableSumRoot
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
            }
            StructuralEffectAction::ReadByteSequenceFieldLength
            | StructuralEffectAction::StoreByteSequenceFieldByte => {
                schema.result
                    == if schema.action == StructuralEffectAction::ReadByteSequenceFieldLength {
                        StructuralEffectResultShape::ByteCount
                    } else {
                        StructuralEffectResultShape::Unit
                    }
                    && schema.custody == StructuralEffectCustody::ExactStructuralByteSequenceField
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
            }
            StructuralEffectAction::StoreByteSequenceField => {
                schema.result == StructuralEffectResultShape::Unit
                    && schema.custody == StructuralEffectCustody::ExactStructuralByteSequenceField
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
            }
            StructuralEffectAction::EstablishByteSequenceSubslice => {
                schema.result == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactImmutableByteView
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresViewAndEstablishesBorrowedView
            }
            StructuralEffectAction::ReadByteSequence => {
                schema.result == StructuralEffectResultShape::Byte
                    && schema.custody == StructuralEffectCustody::ExactImmutableByteView
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
            }
            StructuralEffectAction::WriteByteSequence => {
                schema.result == StructuralEffectResultShape::Unit
                    && schema.custody == StructuralEffectCustody::ExactMutableByteView
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
            }
            StructuralEffectAction::ReadByteSequenceLength => {
                schema.result == StructuralEffectResultShape::ByteCount
                    && schema.custody == StructuralEffectCustody::ExactByteViewMetadata
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
            }
            StructuralEffectAction::StorePrimitive => {
                schema.result == StructuralEffectResultShape::Unit
                    && schema.custody == StructuralEffectCustody::ExactWriteOnlyPrimitiveRoot
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsWriteOnlyPrimitivePlace
            }
            StructuralEffectAction::StoreScalarField => {
                schema.result == StructuralEffectResultShape::Unit
                    && schema.custody == StructuralEffectCustody::ExactStructuralScalarField
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
            }
            StructuralEffectAction::EstablishByteSequencePlace => {
                schema.result == StructuralEffectResultShape::Unit
                    && schema.custody == StructuralEffectCustody::ExactByteSequenceLiteral
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier == StructuralEffectFrontierPolicy::AddsUnrestrictedPlace
            }
            StructuralEffectAction::ReadBooleanField => {
                schema.result == StructuralEffectResultShape::Boolean
                    && schema.custody == StructuralEffectCustody::ExactLiveBooleanField
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsAffinePlace
            }
            StructuralEffectAction::ReadIntegerField => {
                schema.result == StructuralEffectResultShape::Integer
                    && schema.custody == StructuralEffectCustody::ExactLiveIntegerField
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
            }
            StructuralEffectAction::EmitPortWrite => {
                schema.result == StructuralEffectResultShape::Unit
                    && schema.custody == StructuralEffectCustody::ExactPublishedService
                    && schema.external_effect == StructuralEffectExternalEffect::PortWrite
                    && schema.frontier == StructuralEffectFrontierPolicy::KeepsPlaceFrontier
            }
            StructuralEffectAction::EstablishAffinePlace => {
                schema.result == StructuralEffectResultShape::Unit
                    && schema.custody == StructuralEffectCustody::ExactEmptyAffineLocal
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier == StructuralEffectFrontierPolicy::AddsAffinePlace
            }
            StructuralEffectAction::EstablishRecord => {
                schema.result == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactRecord
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::TransfersOwnedChildrenAndAddsOwnedPlace
            }
            StructuralEffectAction::EstablishScalarCase => {
                schema.result == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactScalarCase
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier == StructuralEffectFrontierPolicy::AddsOwnedPlace
            }
            StructuralEffectAction::EstablishScalarArray => {
                schema.result == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactScalarArray
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier == StructuralEffectFrontierPolicy::AddsUnrestrictedPlace
            }
        }
        && schema.fuel == StructuralEffectFuelPolicy::ConsumeOne;
    valid
        .then_some(())
        .ok_or(OperationSemanticError::StructuralEffectSchemaMismatch(tag))
}

fn validate_structural_effect_result(
    operation: &Operation,
    tag: OperationSemanticTag,
    schema: StructuralEffectLeafSchema,
) -> Result<(), OperationSemanticError> {
    let valid = match schema.result {
        StructuralEffectResultShape::Scalar => operation.result.scalar_ref().is_some(),
        StructuralEffectResultShape::Byte => operation.result.scalar_ref().is_some_and(|result| {
            matches!(result.scalar_type, ScalarType::Integer(integer) if integer == IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"))
        }),
        StructuralEffectResultShape::ByteCount => operation.result.scalar_ref().is_some_and(|result| {
            matches!(result.scalar_type, ScalarType::Integer(integer) if integer == IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"))
        }),
        StructuralEffectResultShape::Boolean => operation
            .result
            .scalar_ref()
            .is_some_and(|result| result.scalar_type == ScalarType::Boolean),
        StructuralEffectResultShape::Integer => operation
            .result
            .scalar_ref()
            .is_some_and(|result| matches!(result.scalar_type, ScalarType::Integer(_))),
        StructuralEffectResultShape::Structural => operation.result.structural().is_some(),
        StructuralEffectResultShape::Unit => matches!(operation.result, OperationResult::Unit),
    };
    valid
        .then_some(())
        .ok_or(OperationSemanticError::StructuralEffectResultShapeMismatch(
            tag,
        ))
}

/// Interpret one structural/effect leaf through the supplied exact-unique
/// schema table. The returned observation keeps facts, effects, and ownership
/// frontier events distinct; `Ok(None)` means another semantic algebra owns
/// the operation.
pub fn structural_effect_leaf_observation_in(
    operation: &Operation,
    rows: &[StructuralEffectSemanticRow],
) -> Result<Option<StructuralEffectObservation>, OperationSemanticError> {
    let tag = OperationSemanticTag::for_operation(&operation.kind);
    if is_structural_effect_tag(tag) {
        validate_structural_effect_semantic_rows(rows)?;
    }
    let Some(row) = exact_structural_effect_semantic_row_in(tag, rows)? else {
        return Ok(None);
    };
    let schema = row.schema;
    validate_structural_effect_result(operation, tag, schema)?;
    let observation = match (schema.action, &operation.kind) {
        (
            StructuralEffectAction::EstablishReference,
            OperationKind::EstablishReference { source },
        ) => {
            let result = operation.result.structural().ok_or(
                OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
            )?;
            if source.access != terminal_psi::StructuralAccess::MutableBorrow
                || result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
            {
                return Err(OperationSemanticError::StructuralEffectActionShapeMismatch(
                    tag,
                ));
            }
            StructuralEffectObservation::ReferenceEstablished {
                source: source.clone(),
                destination: result.place,
            }
        }
        (StructuralEffectAction::ReleaseReference, OperationKind::ReleaseReference { source }) => {
            StructuralEffectObservation::ReferenceReleased { source: *source }
        }
        (
            StructuralEffectAction::EstablishPrimitiveLocal,
            OperationKind::EstablishPrimitiveLocal { value },
        ) => {
            let result = operation.result.structural().ok_or(
                OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
            )?;
            StructuralEffectObservation::PrimitiveLocalEstablished {
                destination: result.place,
                value: *value,
            }
        }
        (StructuralEffectAction::ReadPrimitive, OperationKind::PrimitiveScalarRead { source }) => {
            let result = operation.result.scalar().ok_or(
                OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
            )?;
            StructuralEffectObservation::PrimitiveRead {
                source: *source,
                result: result.id,
            }
        }
        (
            StructuralEffectAction::ObserveCaseMembership,
            OperationKind::StructuralCaseMembership { source, case },
        ) => StructuralEffectObservation::CaseMembershipObserved {
            source: *source,
            case: *case,
            result: operation.result.expect_scalar().id,
        },
        (
            StructuralEffectAction::ReadByteSequenceFieldLength,
            OperationKind::StructuralByteSequenceFieldLength {
                source,
                path,
                field,
            },
        ) => StructuralEffectObservation::ByteSequenceFieldLengthRead {
            source: *source,
            path: path.clone(),
            field: *field,
            result: operation.result.expect_scalar().id,
        },
        (
            StructuralEffectAction::StoreByteSequenceFieldByte,
            OperationKind::StructuralByteSequenceFieldByteStore {
                destination,
                path,
                field,
                index,
                value,
                length,
                obligation,
            },
        ) => StructuralEffectObservation::ByteSequenceFieldByteStored {
            destination: *destination,
            path: path.clone(),
            field: *field,
            index: *index,
            value: *value,
            length: *length,
            obligation: *obligation,
        },
        (
            StructuralEffectAction::EstablishByteSequenceSubslice,
            OperationKind::ByteSequenceSubslice {
                source,
                start,
                end,
                length,
                obligation,
            },
        ) => StructuralEffectObservation::ByteSequenceSubslice {
            source: *source,
            start: *start,
            end: *end,
            length: *length,
            destination: operation
                .result
                .structural()
                .expect("validated structural result")
                .place,
            obligation: *obligation,
        },
        (
            StructuralEffectAction::ReadByteSequence,
            OperationKind::ByteSequenceRead {
                source,
                index,
                length,
                obligation,
            },
        ) => StructuralEffectObservation::ByteSequenceRead {
            source: *source,
            index: *index,
            length: *length,
            result: operation.result.expect_scalar().id,
            obligation: *obligation,
        },
        (
            StructuralEffectAction::WriteByteSequence,
            OperationKind::ByteSequenceWrite {
                destination,
                index,
                value,
                length,
                obligation,
            },
        ) => StructuralEffectObservation::ByteSequenceWrite {
            destination: *destination,
            index: *index,
            value: *value,
            length: *length,
            obligation: *obligation,
        },
        (
            StructuralEffectAction::ReadByteSequenceLength,
            OperationKind::ByteSequenceLength { source },
        ) => StructuralEffectObservation::ByteSequenceLengthRead {
            source: *source,
            result: operation.result.expect_scalar().id,
        },
        (
            StructuralEffectAction::StorePrimitive,
            OperationKind::WriteOnlyPrimitiveStore { destination, value },
        ) => StructuralEffectObservation::PrimitiveStored {
            destination: *destination,
            value: *value,
        },
        (
            StructuralEffectAction::StoreByteSequenceField,
            OperationKind::StructuralByteSequenceFieldStore {
                destination,
                path,
                field,
                source,
                length,
                obligation,
            },
        ) => StructuralEffectObservation::ByteSequenceFieldStored {
            destination: *destination,
            path: path.clone(),
            field: *field,
            source: *source,
            length: *length,
            obligation: *obligation,
        },
        (
            StructuralEffectAction::StoreScalarField,
            OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
            },
        ) => StructuralEffectObservation::ScalarFieldStored {
            destination: *destination,
            path: path.clone(),
            field: *field,
            value: *value,
        },
        (
            StructuralEffectAction::EstablishScalarArray,
            OperationKind::EstablishScalarArray { elements },
        ) => {
            let result = operation
                .result
                .structural()
                .ok_or(OperationSemanticError::StructuralEffectSchemaMismatch(tag))?;
            StructuralEffectObservation::ScalarArrayEstablished {
                destination: result.place,
                elements: elements.clone(),
            }
        }
        (
            StructuralEffectAction::EstablishScalarCase,
            OperationKind::EstablishScalarCase {
                result_case,
                fields,
            },
        ) => {
            let result = operation
                .result
                .structural()
                .expect("validated scalar-case structural result");
            StructuralEffectObservation::ScalarCaseEstablished {
                membership: Proposition::StructuralCaseMembership {
                    subject: StructuralCaseSubject::new(result.place, Vec::new()),
                    case: *result_case,
                },
                fields: fields.clone(),
            }
        }
        (
            StructuralEffectAction::ReadIntegerField,
            OperationKind::IntegerStructuralField { source, field },
        ) => StructuralEffectObservation::IntegerFieldRead {
            source: *source,
            field: *field,
            result: operation
                .result
                .scalar_ref()
                .expect("validated integer structural-field result")
                .id,
        },
        (
            StructuralEffectAction::EstablishByteSequencePlace,
            OperationKind::EstablishByteSequenceLiteral { destination, .. },
        ) => StructuralEffectObservation::ByteSequencePlaceEstablished {
            destination: *destination,
        },
        (
            StructuralEffectAction::ReadBooleanField,
            OperationKind::BooleanStructuralField { source, field },
        ) => {
            let result = operation
                .result
                .scalar_ref()
                .expect("validated Boolean structural-field result");
            StructuralEffectObservation::BooleanFieldEquation(Proposition::Equal(
                ScalarTerm::value(result.id, result.scalar_type),
                ScalarTerm::boolean_field(*source, *field),
            ))
        }
        (
            StructuralEffectAction::EmitPortWrite,
            OperationKind::PortWrite {
                service,
                port,
                value,
            },
        ) => StructuralEffectObservation::PortWrite {
            service: *service,
            port: *port,
            value: *value,
        },
        (
            StructuralEffectAction::EstablishAffinePlace,
            OperationKind::EstablishTrivialAffineLocal { destination },
        ) => StructuralEffectObservation::AffinePlaceEstablished {
            destination: *destination,
        },
        (StructuralEffectAction::EstablishRecord, OperationKind::EstablishRecord { fields }) => {
            let result = operation
                .result
                .structural()
                .expect("validated scalar-record structural result");
            StructuralEffectObservation::RecordEstablished {
                destination: result.place,
                fields: fields.clone(),
            }
        }
        _ => {
            return Err(OperationSemanticError::StructuralEffectActionShapeMismatch(
                tag,
            ));
        }
    };
    Ok(Some(observation))
}

pub fn structural_effect_leaf_observation(
    operation: &Operation,
) -> Result<Option<StructuralEffectObservation>, OperationSemanticError> {
    structural_effect_leaf_observation_in(operation, &StructuralEffectSemanticRow::ALL)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use semantic_vocabulary::{OperationId, StructuralFieldId, ValueId};
    use terminal_psi::{OperationResult, ValueDeclaration};

    use super::*;

    #[test]
    fn reference_actions_preserve_source_and_separate_carrier_custody() {
        let source = terminal_psi::StructuralArgument {
            place: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            access: terminal_psi::StructuralAccess::MutableBorrow,
        };
        let destination = PlaceId::new(2).unwrap();
        let establish = Operation {
            static_reach_binding: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: destination,
                structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishReference {
                source: source.clone(),
            },
        };
        let release = Operation {
            static_reach_binding: None,
            id: OperationId::new(2).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::ReleaseReference {
                source: destination,
            },
        };
        for (operation, expected, frontier) in [
            (
                &establish,
                StructuralEffectObservation::ReferenceEstablished {
                    source,
                    destination,
                },
                StructuralEffectFrontierPolicy::CapturesLoanAndAddsReferenceCarrier,
            ),
            (
                &release,
                StructuralEffectObservation::ReferenceReleased {
                    source: destination,
                },
                StructuralEffectFrontierPolicy::ReleasesReferenceCarrier,
            ),
        ] {
            let observation = structural_effect_leaf_observation(operation)
                .unwrap()
                .unwrap();
            assert_eq!(observation, expected);
            assert!(observation.local_equation().is_none());
            assert!(observation.canonical_obligation().is_none());
            let schema = structural_effect_semantic_row(&operation.kind)
                .unwrap()
                .unwrap()
                .schema();
            assert_eq!(schema.frontier(), frontier);
            assert_eq!(schema.fuel(), StructuralEffectFuelPolicy::ConsumeOne);
            assert_eq!(
                schema.external_effect(),
                StructuralEffectExternalEffect::None
            );
            let mut wrong_result = operation.clone();
            wrong_result.result = if operation.result == OperationResult::Unit {
                establish.result.clone()
            } else {
                OperationResult::Unit
            };
            assert!(structural_effect_leaf_observation(&wrong_result).is_err());
        }
        let mut ownership_forgery = establish;
        let OperationKind::EstablishReference { source } = &mut ownership_forgery.kind else {
            unreachable!()
        };
        source.access = terminal_psi::StructuralAccess::Owned;
        assert!(structural_effect_leaf_observation(&ownership_forgery).is_err());
        let mut drifted = StructuralEffectSemanticRow::ALL;
        let release_row = drifted
            .iter_mut()
            .find(|row| row.tag == OperationSemanticTag::ReleaseReference)
            .unwrap();
        release_row.schema.frontier = StructuralEffectFrontierPolicy::KeepsPlaceFrontier;
        assert!(structural_effect_leaf_observation_in(&release, &drifted).is_err());
    }

    #[test]
    fn primitive_local_actions_retain_exact_subjects_without_scalar_equations() {
        let local = OperationResult::Structural(terminal_psi::StructuralOperationResult {
            place: PlaceId::new(23).unwrap(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(7).unwrap(),
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        });
        let scalar = OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(47).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        for (operation, expected, result_shape, custody, action, frontier, wrong_result) in [
            (
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(31).unwrap(),
                    result: local.clone(),
                    kind: OperationKind::EstablishPrimitiveLocal {
                        value: ValueId::new(11).unwrap(),
                    },
                },
                StructuralEffectObservation::PrimitiveLocalEstablished {
                    destination: PlaceId::new(23).unwrap(),
                    value: ValueId::new(11).unwrap(),
                },
                StructuralEffectResultShape::Structural,
                StructuralEffectCustody::ExactPrimitiveLocal,
                StructuralEffectAction::EstablishPrimitiveLocal,
                StructuralEffectFrontierPolicy::AddsUnrestrictedPlace,
                scalar.clone(),
            ),
            (
                Operation {
                    static_reach_binding: None,
                    id: OperationId::new(32).unwrap(),
                    result: scalar,
                    kind: OperationKind::PrimitiveScalarRead {
                        source: PlaceId::new(23).unwrap(),
                    },
                },
                StructuralEffectObservation::PrimitiveRead {
                    source: PlaceId::new(23).unwrap(),
                    result: ValueId::new(47).unwrap(),
                },
                StructuralEffectResultShape::Scalar,
                StructuralEffectCustody::ExactReadablePrimitiveRoot,
                StructuralEffectAction::ReadPrimitive,
                StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace,
                local,
            ),
        ] {
            let row = structural_effect_semantic_row(&operation.kind)
                .unwrap()
                .unwrap();
            let schema = row.schema();
            assert_eq!(schema.result(), result_shape);
            assert_eq!(schema.custody(), custody);
            assert_eq!(schema.action(), action);
            assert_eq!(schema.frontier(), frontier);
            assert_eq!(
                schema.external_effect(),
                StructuralEffectExternalEffect::None
            );
            assert_eq!(schema.fuel(), StructuralEffectFuelPolicy::ConsumeOne);
            assert_eq!(schema.goal(), StructuralEffectGoalShape::None);
            let observation = structural_effect_leaf_observation(&operation)
                .unwrap()
                .unwrap();
            assert_eq!(observation, expected);
            assert_eq!(observation.local_equation(), None);
            assert_eq!(observation.canonical_obligation(), None);
            assert!(!crate::is_unconditionally_total_scalar(&operation.kind));

            for result in [OperationResult::Unit, wrong_result] {
                let mut malformed = operation.clone();
                malformed.result = result;
                assert_eq!(
                    structural_effect_leaf_observation(&malformed),
                    Err(OperationSemanticError::StructuralEffectResultShapeMismatch(
                        row.tag()
                    )),
                );
            }
            for axis in 0..6 {
                let mut drifted = StructuralEffectSemanticRow::ALL;
                let schema = &mut drifted
                    .iter_mut()
                    .find(|candidate| candidate.tag == row.tag())
                    .unwrap()
                    .schema;
                match axis {
                    0 => schema.result = StructuralEffectResultShape::Unit,
                    1 => schema.custody = StructuralEffectCustody::ExactWriteOnlyPrimitiveRoot,
                    2 => schema.action = StructuralEffectAction::StorePrimitive,
                    3 => schema.external_effect = StructuralEffectExternalEffect::PortWrite,
                    4 => schema.frontier = StructuralEffectFrontierPolicy::KeepsPlaceFrontier,
                    _ => schema.goal = StructuralEffectGoalShape::ByteIndexInBounds,
                }
                assert_eq!(
                    structural_effect_leaf_observation_in(&operation, &drifted),
                    Err(OperationSemanticError::StructuralEffectSchemaMismatch(
                        row.tag()
                    )),
                );
            }
        }
    }

    #[test]
    fn byte_field_store_requires_contextual_capacity_and_keeps_exact_subjects() {
        let operation = Operation {
            static_reach_binding: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralByteSequenceFieldStore {
                destination: PlaceId::new(2).unwrap(),
                path: vec![terminal_psi::StructuralPathSegment::Field("nested".into())],
                field: StructuralFieldId::new(3).unwrap(),
                source: PlaceId::new(4).unwrap(),
                length: ValueId::new(5).unwrap(),
                obligation: ObligationId::new(6).unwrap(),
            },
        };
        let observation = structural_effect_leaf_observation(&operation)
            .unwrap()
            .unwrap();
        let StructuralEffectObservation::ByteSequenceFieldStored {
            destination,
            path,
            field,
            source,
            length,
            obligation,
        } = &observation
        else {
            panic!("exact byte field store observation")
        };
        assert_eq!(*destination, PlaceId::new(2).unwrap());
        assert_eq!(
            path,
            &vec![terminal_psi::StructuralPathSegment::Field("nested".into())]
        );
        assert_eq!(*field, StructuralFieldId::new(3).unwrap());
        assert_eq!(*source, PlaceId::new(4).unwrap());
        assert_eq!(*length, ValueId::new(5).unwrap());
        assert_eq!(*obligation, ObligationId::new(6).unwrap());
        assert_eq!(observation.local_equation(), None);
        assert_eq!(observation.canonical_obligation(), None);
        let row = StructuralEffectSemanticRow::ALL
            .iter()
            .find(|row| row.tag == OperationSemanticTag::StructuralByteSequenceFieldStore)
            .unwrap();
        assert_eq!(
            row.schema.goal,
            StructuralEffectGoalShape::ByteLengthWithinFieldCapacity
        );
        let mut malformed = operation;
        malformed.result = OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(7).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        assert!(structural_effect_leaf_observation(&malformed).is_err());
    }

    #[test]
    fn byte_sequence_length_retains_source_without_inventing_a_proof_equation() {
        let operation = Operation {
            static_reach_binding: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: ValueId::new(2).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            }),
            kind: OperationKind::ByteSequenceLength {
                source: PlaceId::new(3).unwrap(),
            },
        };
        let observation = structural_effect_leaf_observation(&operation)
            .unwrap()
            .unwrap();
        assert_eq!(
            observation,
            StructuralEffectObservation::ByteSequenceLengthRead {
                source: PlaceId::new(3).unwrap(),
                result: ValueId::new(2).unwrap()
            }
        );
        assert_eq!(observation.local_equation(), None);
        let mut wrong_result = operation;
        wrong_result.result = OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(2).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
        });
        assert!(structural_effect_leaf_observation(&wrong_result).is_err());
    }

    #[test]
    fn inventory_is_exact_unique_and_keeps_axes_separate() {
        assert_eq!(StructuralEffectSemanticRow::ALL.len(), 22);
        assert_eq!(
            StructuralEffectSemanticRow::ALL
                .iter()
                .map(|row| row.tag())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                OperationSemanticTag::EstablishReference,
                OperationSemanticTag::ReleaseReference,
                OperationSemanticTag::EstablishPrimitiveLocal,
                OperationSemanticTag::PrimitiveScalarRead,
                OperationSemanticTag::StructuralCaseMembership,
                OperationSemanticTag::StructuralByteSequenceFieldLength,
                OperationSemanticTag::StructuralByteSequenceFieldByteStore,
                OperationSemanticTag::StructuralByteSequenceFieldStore,
                OperationSemanticTag::ByteSequenceSubslice,
                OperationSemanticTag::ByteSequenceRead,
                OperationSemanticTag::ByteSequenceWrite,
                OperationSemanticTag::ByteSequenceLength,
                OperationSemanticTag::WriteOnlyPrimitiveStore,
                OperationSemanticTag::StructuralScalarFieldStore,
                OperationSemanticTag::EstablishScalarCase,
                OperationSemanticTag::EstablishScalarArray,
                OperationSemanticTag::EstablishByteSequenceLiteral,
                OperationSemanticTag::BooleanStructuralField,
                OperationSemanticTag::IntegerStructuralField,
                OperationSemanticTag::PortWrite,
                OperationSemanticTag::EstablishTrivialAffineLocal,
                OperationSemanticTag::EstablishRecord,
            ]),
        );
        let boolean = structural_effect_semantic_row(&OperationKind::BooleanStructuralField {
            source: PlaceId::new(1).unwrap(),
            field: StructuralFieldId::new(1).unwrap(),
        })
        .unwrap()
        .unwrap()
        .schema();
        assert_eq!(boolean.result(), StructuralEffectResultShape::Boolean);
        assert_eq!(
            boolean.custody(),
            StructuralEffectCustody::ExactLiveBooleanField,
        );
        assert_eq!(boolean.action(), StructuralEffectAction::ReadBooleanField);
        assert_eq!(
            boolean.external_effect(),
            StructuralEffectExternalEffect::None,
        );
        assert_eq!(boolean.fuel(), StructuralEffectFuelPolicy::ConsumeOne);
        assert_eq!(
            boolean.frontier(),
            StructuralEffectFrontierPolicy::RequiresAndKeepsAffinePlace,
        );

        let store = structural_effect_semantic_row(&OperationKind::WriteOnlyPrimitiveStore {
            destination: PlaceId::new(2).unwrap(),
            value: ValueId::new(3).unwrap(),
        })
        .unwrap()
        .unwrap()
        .schema();
        assert_eq!(store.result(), StructuralEffectResultShape::Unit);
        assert_eq!(
            store.custody(),
            StructuralEffectCustody::ExactWriteOnlyPrimitiveRoot,
        );
        assert_eq!(store.action(), StructuralEffectAction::StorePrimitive);
        assert_eq!(
            store.frontier(),
            StructuralEffectFrontierPolicy::RequiresAndKeepsWriteOnlyPrimitivePlace,
        );
    }

    #[test]
    fn byte_write_retains_mutable_custody_unit_and_exact_bounds_goal() {
        let operation = Operation {
            static_reach_binding: None,
            id: OperationId::new(5).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::ByteSequenceWrite {
                destination: PlaceId::new(1).unwrap(),
                index: ValueId::new(2).unwrap(),
                value: ValueId::new(6).unwrap(),
                length: ValueId::new(3).unwrap(),
                obligation: ObligationId::new(4).unwrap(),
            },
        };
        let schema = structural_effect_semantic_row(&operation.kind)
            .unwrap()
            .unwrap()
            .schema();
        assert_eq!(schema.goal(), StructuralEffectGoalShape::ByteIndexInBounds);
        assert_eq!(schema.result(), StructuralEffectResultShape::Unit);
        assert_eq!(
            schema.custody(),
            StructuralEffectCustody::ExactMutableByteView
        );
        assert_eq!(schema.fuel(), StructuralEffectFuelPolicy::ConsumeOne);
        let observation = structural_effect_leaf_observation(&operation)
            .unwrap()
            .unwrap();
        assert!(observation.local_equation().is_none());
        let count_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        assert_eq!(
            observation.canonical_obligation(),
            Some((
                ObligationId::new(4).unwrap(),
                Proposition::LessThan(
                    ScalarTerm::value(ValueId::new(2).unwrap(), count_type),
                    ScalarTerm::value(ValueId::new(3).unwrap(), count_type),
                ),
            ))
        );
    }

    #[test]
    fn byte_read_has_an_exact_canonical_goal_without_a_scalar_denotation_equation() {
        let source = PlaceId::new(1).unwrap();
        let index = ValueId::new(2).unwrap();
        let length = ValueId::new(3).unwrap();
        let obligation = ObligationId::new(4).unwrap();
        let operation = Operation {
            static_reach_binding: None,
            id: OperationId::new(5).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: ValueId::new(6).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                ),
            }),
            kind: OperationKind::ByteSequenceRead {
                source,
                index,
                length,
                obligation,
            },
        };
        let schema = structural_effect_semantic_row(&operation.kind)
            .unwrap()
            .unwrap()
            .schema();
        assert_eq!(schema.goal(), StructuralEffectGoalShape::ByteIndexInBounds);
        assert_eq!(schema.result(), StructuralEffectResultShape::Byte);
        assert_eq!(
            schema.custody(),
            StructuralEffectCustody::ExactImmutableByteView
        );
        assert_eq!(schema.fuel(), StructuralEffectFuelPolicy::ConsumeOne);
        assert_eq!(
            schema.external_effect(),
            StructuralEffectExternalEffect::None
        );
        assert_eq!(
            schema.frontier(),
            StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
        );
        let observation = structural_effect_leaf_observation(&operation)
            .unwrap()
            .unwrap();
        assert_eq!(observation.local_equation(), None);
        let count_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        assert_eq!(
            observation.canonical_obligation(),
            Some((
                obligation,
                Proposition::LessThan(
                    ScalarTerm::value(index, count_type),
                    ScalarTerm::value(length, count_type),
                )
            ))
        );
        assert!(!crate::is_unconditionally_total_scalar(&operation.kind));
        let mut drifted = StructuralEffectSemanticRow::ALL;
        let read_row = drifted
            .iter_mut()
            .find(|row| row.tag == OperationSemanticTag::ByteSequenceRead)
            .unwrap();
        read_row.schema.goal = StructuralEffectGoalShape::None;
        assert!(structural_effect_leaf_observation_in(&operation, &drifted).is_err());
        let mut wrong_result = operation;
        wrong_result.result = OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(6).unwrap(),
            scalar_type: count_type,
        });
        assert!(structural_effect_leaf_observation(&wrong_result).is_err());
    }

    #[test]
    fn primitive_store_observation_binds_destination_and_value_without_a_fact() {
        let operation = Operation {
            static_reach_binding: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: PlaceId::new(2).unwrap(),
                value: ValueId::new(3).unwrap(),
            },
        };
        let observation = structural_effect_leaf_observation(&operation)
            .unwrap()
            .expect("write-only store structural observation");
        assert_eq!(
            observation,
            StructuralEffectObservation::PrimitiveStored {
                destination: PlaceId::new(2).unwrap(),
                value: ValueId::new(3).unwrap(),
            },
        );
        assert_eq!(observation.local_equation(), None);

        let mut forged = operation;
        forged.result = OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(4).unwrap(),
            scalar_type: ScalarType::Boolean,
        });
        assert_eq!(
            structural_effect_leaf_observation(&forged),
            Err(OperationSemanticError::StructuralEffectResultShapeMismatch(
                OperationSemanticTag::WriteOnlyPrimitiveStore,
            )),
        );
    }

    #[test]
    fn lookup_rejects_missing_duplicate_and_axis_drift() {
        let tag = OperationSemanticTag::BooleanStructuralField;
        let canonical =
            *exact_structural_effect_semantic_row_in(tag, &StructuralEffectSemanticRow::ALL)
                .unwrap()
                .unwrap();
        let missing = StructuralEffectSemanticRow::ALL
            .iter()
            .copied()
            .filter(|row| row.tag != tag)
            .collect::<Vec<_>>();
        assert_eq!(
            exact_structural_effect_semantic_row_in(tag, &missing),
            Err(OperationSemanticError::MissingStructuralEffectRow(tag)),
        );
        let mut duplicate = StructuralEffectSemanticRow::ALL.to_vec();
        duplicate.push(canonical);
        assert_eq!(
            exact_structural_effect_semantic_row_in(tag, &duplicate),
            Err(OperationSemanticError::DuplicateStructuralEffectRow(tag)),
        );

        let mut drifted = StructuralEffectSemanticRow::ALL;
        let boolean_index = drifted
            .iter()
            .position(|row| row.tag == tag)
            .expect("Boolean structural row");
        drifted[boolean_index].schema.frontier = StructuralEffectFrontierPolicy::KeepsPlaceFrontier;
        let operation = Operation {
            static_reach_binding: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField {
                source: PlaceId::new(1).unwrap(),
                field: StructuralFieldId::new(1).unwrap(),
            },
        };
        assert_eq!(
            structural_effect_leaf_observation_in(&operation, &drifted),
            Err(OperationSemanticError::StructuralEffectSchemaMismatch(tag)),
        );
    }

    #[test]
    fn rows_emit_distinct_fact_effect_and_frontier_observations() {
        let result = ValueId::new(1).unwrap();
        let source = PlaceId::new(1).unwrap();
        let field = StructuralFieldId::new(1).unwrap();
        let boolean = Operation {
            static_reach_binding: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField { source, field },
        };
        assert_eq!(
            structural_effect_leaf_observation(&boolean).unwrap(),
            Some(StructuralEffectObservation::BooleanFieldEquation(
                Proposition::Equal(
                    ScalarTerm::value(result, ScalarType::Boolean),
                    ScalarTerm::boolean_field(source, field),
                ),
            )),
        );

        let service = ServiceId::new(1).unwrap();
        let port = Operation {
            static_reach_binding: None,
            id: OperationId::new(2).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::PortWrite {
                service,
                port: 0x3f8,
                value: 75,
            },
        };
        assert_eq!(
            structural_effect_leaf_observation(&port).unwrap(),
            Some(StructuralEffectObservation::PortWrite {
                service,
                port: 0x3f8,
                value: 75,
            }),
        );

        let destination = PlaceId::new(2).unwrap();
        let establish = Operation {
            static_reach_binding: None,
            id: OperationId::new(3).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishTrivialAffineLocal { destination },
        };
        assert_eq!(
            structural_effect_leaf_observation(&establish).unwrap(),
            Some(StructuralEffectObservation::AffinePlaceEstablished { destination }),
        );
    }

    #[test]
    fn rows_fail_closed_on_result_or_action_drift() {
        let boolean = Operation {
            static_reach_binding: None,
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::BooleanStructuralField {
                source: PlaceId::new(1).unwrap(),
                field: StructuralFieldId::new(1).unwrap(),
            },
        };
        assert_eq!(
            structural_effect_leaf_observation(&boolean),
            Err(OperationSemanticError::StructuralEffectResultShapeMismatch(
                OperationSemanticTag::BooleanStructuralField,
            )),
        );

        let mut crossed = StructuralEffectSemanticRow::ALL;
        let boolean_index = crossed
            .iter()
            .position(|row| row.tag == OperationSemanticTag::BooleanStructuralField)
            .expect("Boolean structural row");
        crossed[boolean_index].schema.action = StructuralEffectAction::EmitPortWrite;
        assert_eq!(
            structural_effect_leaf_observation_in(
                &Operation {
                    static_reach_binding: None,
                    id: OperationId::new(2).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: ValueId::new(1).unwrap(),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanStructuralField {
                        source: PlaceId::new(1).unwrap(),
                        field: StructuralFieldId::new(1).unwrap(),
                    },
                },
                &crossed,
            ),
            Err(OperationSemanticError::StructuralEffectSchemaMismatch(
                OperationSemanticTag::BooleanStructuralField,
            )),
        );
    }
}
