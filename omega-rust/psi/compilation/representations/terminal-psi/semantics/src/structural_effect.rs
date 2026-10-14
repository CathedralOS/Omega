//! Exact structural/effect leaf schemas and their local observations.

use semantic_vocabulary::{
    IntegerSign, IntegerType, ObligationId, PlaceId, Proposition, ScalarTerm, ScalarType,
    ServiceId, StructuralCaseId, StructuralCaseSubject, StructuralFieldId, ValueId,
};
use terminal_psi::{Operation, OperationKind, OperationResult, live_field_by_id};

mod byte_equal;
mod byte_extent;
mod byte_predicate;
pub use byte_equal::byte_equal_equation;
pub use byte_predicate::byte_predicate_equation;
mod observation;
pub use observation::{
    StructuralEffectObservation, same_descriptor_coordinate, structural_effect_leaf_observation,
    structural_effect_leaf_observation_in,
};
mod element_extent;
pub use byte_extent::{literal_length_equation, subslice_length_equation};
pub use element_extent::{
    ElementViewLeaf, element_establishment_length_equation, element_subslice_length_equation,
    element_view_leaf, element_view_leaf_scalar, element_view_write_goal,
};

use super::OperationSemanticError;
use crate::semantic_rows::OperationSemanticTag;

/// The resolved canonical path of a `ByteSequenceOperand::Field`: project
/// the source path, confirm the parent record exposes the named field live
/// as a byte-sequence carrier, and append the field segment. `Whole`
/// operands name no field and return `None`; callers accepting whole
/// operands check that arm themselves.
pub(crate) fn byte_sequence_field_path(
    operand: &terminal_psi::control_flow::ByteSequenceOperand,
    root_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<(
    PlaceId,
    Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
)> {
    let terminal_psi::control_flow::ByteSequenceOperand::Field { source, field, .. } = operand
    else {
        return None;
    };
    let (mut path, parent) =
        crate::canonical_static_projection(structural_types.iter(), root_type, &source.path)?;
    let fields = match &parent.shape {
        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
        _ => return None,
    };
    let selected = live_field_by_id(fields, *field)?;
    selected.field_type.byte_sequence_carrier()?;
    path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
        *field,
    ));
    Some((source.place, path))
}

/// The `u64` count-equality a measured extent read yields: `result = n`.
pub(crate) fn u64_extent_proposition(
    result: ValueId,
    count: u128,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let count = ScalarTerm::integer(
        integer_type,
        semantic_vocabulary::IntegerValue::Unsigned(count),
    )
    .map_err(OperationSemanticError::InvalidProposition)?;
    Ok(Some(Proposition::Equal(
        ScalarTerm::value(result, ScalarType::Integer(integer_type)),
        count,
    )))
}

/// The `u64` subtraction a bounded extent read yields: `result = end - start`.
pub(crate) fn u64_extent_difference_proposition(
    result: ValueId,
    end: ValueId,
    start: ValueId,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let value = |identity| ScalarTerm::value(identity, ScalarType::Integer(integer_type));
    let difference = ScalarTerm::exact_integer_subtract(integer_type, value(end), value(start))
        .map_err(OperationSemanticError::InvalidProposition)?;
    Ok(Some(Proposition::Equal(value(result), difference)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectResultShape {
    Scalar,
    Boolean,
    Integer,
    ByteCount,
    ElementCount,
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
    ExactReadableSumProjection,
    ExactWriteOnlyPrimitiveRoot,
    ExactStructuralScalarField,
    ExactStructuralByteSequenceField,
    ExactByteSequenceLiteral,
    ExactImmutableByteView,
    ExactMutableByteView,
    ExactBoundedByteOwner,
    ExactByteViewMetadata,
    ExactElementViewSource,
    ExactImmutableElementView,
    ExactMutableElementView,
    ExactElementViewMetadata,
    ExactLiveBooleanField,
    ExactLiveScalarField,
    ExactPublishedService,
    ExactEmptyAffineLocal,
    ExactStructuralArray,
    ExactRecord,
    ExactScalarCase,
    ExactStructuralCase,
    ExactScalarArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectAction {
    EstablishPlacedReference,
    EstablishReference,
    ReleaseReference,
    EstablishPrimitiveLocal,
    ReadPrimitive,
    ObserveCaseMembership,
    StorePrimitive,
    StoreScalarField,
    StoreByteSequenceField,
    StoreByteViewField,
    StoreByteSequence,
    ReadByteSequenceFieldLength,
    StoreByteSequenceFieldByte,
    ReadByteSequenceFieldByte,
    EstablishByteSequencePlace,
    ReadByteSequenceLength,
    ReadByteSequence,
    WriteByteSequence,
    EstablishByteSequenceSubslice,
    EvaluateByteSequencePredicate,
    EvaluateByteSequenceEqual,
    EstablishElementView,
    EstablishMutableElementView,
    ReadElementViewLength,
    ReadElementView,
    WriteElementView,
    EstablishElementViewSubslice,
    ReadBooleanField,
    ReadScalarField,
    EmitPortWrite,
    EstablishAffinePlace,
    EstablishStructuralArray,
    EstablishRecord,
    EstablishScalarCase,
    EstablishStructuralCase,
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
    ByteLengthWithinOwnerCapacity,
    ElementIndexInBounds,
    ElementRangeInBounds,
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
            StructuralEffectAction::StoreByteSequence => {
                StructuralEffectGoalShape::ByteLengthWithinOwnerCapacity
            }
            StructuralEffectAction::StoreByteSequenceField => {
                StructuralEffectGoalShape::ByteLengthWithinFieldCapacity
            }
            StructuralEffectAction::ReadByteSequence
            | StructuralEffectAction::WriteByteSequence
            | StructuralEffectAction::StoreByteSequenceFieldByte
            | StructuralEffectAction::ReadByteSequenceFieldByte => {
                StructuralEffectGoalShape::ByteIndexInBounds
            }
            StructuralEffectAction::EstablishByteSequenceSubslice => {
                StructuralEffectGoalShape::ByteRangeInBounds
            }
            StructuralEffectAction::ReadElementView | StructuralEffectAction::WriteElementView => {
                StructuralEffectGoalShape::ElementIndexInBounds
            }
            StructuralEffectAction::EstablishElementViewSubslice => {
                StructuralEffectGoalShape::ElementRangeInBounds
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
    pub const fn tag(self) -> OperationSemanticTag {
        self.tag
    }

    pub const fn schema(self) -> StructuralEffectLeafSchema {
        self.schema
    }
}

/// `{ tag => (result, custody, action, external, frontier) }` — the
/// structural/effect cohort rows and their operation-tag membership in one
/// declarative table. `action` stays explicit: five tags name a different
/// action than their own spelling.
macro_rules! structural_effect_rows {
    ($( $tag:ident => ($result:ident, $custody:ident, $action:ident, $external:ident, $frontier:ident) ),+ $(,)?) => {
        impl StructuralEffectSemanticRow {
            pub const ALL: [Self; structural_effect_rows!(@count $( $tag )+)] = [
                $( Self {
                    tag: OperationSemanticTag::$tag,
                    schema: structural_effect_leaf(
                        StructuralEffectResultShape::$result,
                        StructuralEffectCustody::$custody,
                        StructuralEffectAction::$action,
                        StructuralEffectExternalEffect::$external,
                        StructuralEffectFrontierPolicy::$frontier,
                    ),
                } ),+
            ];
        }

        const fn is_structural_effect_tag(tag: OperationSemanticTag) -> bool {
            matches!(tag, $( OperationSemanticTag::$tag )|+)
        }
    };
    (@count $( $variant:ident )+) => {
        <[()]>::len(&[$(structural_effect_rows!(@unit $variant)),+])
    };
    (@unit $variant:ident) => { () };
}

structural_effect_rows! {
    EstablishPlacedReference => (Structural, ExactReferenceSource, EstablishPlacedReference, None, CapturesLoanAndAddsReferenceCarrier),
    EstablishReference => (Structural, ExactReferenceSource, EstablishReference, None, CapturesLoanAndAddsReferenceCarrier),
    ReleaseReference => (Unit, ExactReferenceCarrier, ReleaseReference, None, ReleasesReferenceCarrier),
    EstablishScalarArray => (Structural, ExactScalarArray, EstablishScalarArray, None, AddsUnrestrictedPlace),
    EstablishPrimitiveLocal => (Structural, ExactPrimitiveLocal, EstablishPrimitiveLocal, None, AddsUnrestrictedPlace),
    PrimitiveScalarRead => (Scalar, ExactReadablePrimitiveRoot, ReadPrimitive, None, RequiresAndKeepsStructuralPlace),
    StructuralCaseMembership => (Boolean, ExactReadableSumProjection, ObserveCaseMembership, None, RequiresAndKeepsStructuralPlace),
    StructuralByteSequenceFieldLength => (ByteCount, ExactStructuralByteSequenceField, ReadByteSequenceFieldLength, None, RequiresAndKeepsStructuralPlace),
    StructuralByteSequenceFieldByteStore => (Unit, ExactStructuralByteSequenceField, StoreByteSequenceFieldByte, None, RequiresAndKeepsStructuralPlace),
    ByteSequenceStore => (Unit, ExactBoundedByteOwner, StoreByteSequence, None, RequiresAndKeepsStructuralPlace),
    StructuralByteSequenceFieldStore => (Unit, ExactStructuralByteSequenceField, StoreByteSequenceField, None, RequiresAndKeepsStructuralPlace),
    StructuralByteViewFieldStore => (Unit, ExactStructuralByteSequenceField, StoreByteViewField, None, RequiresAndKeepsStructuralPlace),
    StructuralByteSequenceFieldRead => (Byte, ExactStructuralByteSequenceField, ReadByteSequenceFieldByte, None, RequiresAndKeepsStructuralPlace),
    ByteSequenceSubslice => (Structural, ExactImmutableByteView, EstablishByteSequenceSubslice, None, RequiresViewAndEstablishesBorrowedView),
    ByteSequencePredicate => (Boolean, ExactImmutableByteView, EvaluateByteSequencePredicate, None, RequiresAndKeepsStructuralPlace),
    ByteSequenceEqual => (Boolean, ExactImmutableByteView, EvaluateByteSequenceEqual, None, RequiresAndKeepsStructuralPlace),
    EstablishElementView => (Structural, ExactElementViewSource, EstablishElementView, None, RequiresViewAndEstablishesBorrowedView),
    EstablishMutableElementView => (Structural, ExactElementViewSource, EstablishMutableElementView, None, RequiresViewAndEstablishesBorrowedView),
    ElementViewLength => (ElementCount, ExactElementViewMetadata, ReadElementViewLength, None, RequiresAndKeepsStructuralPlace),
    ElementViewRead => (Scalar, ExactImmutableElementView, ReadElementView, None, RequiresAndKeepsStructuralPlace),
    ElementViewWrite => (Unit, ExactMutableElementView, WriteElementView, None, RequiresAndKeepsStructuralPlace),
    ElementViewSubslice => (Structural, ExactImmutableElementView, EstablishElementViewSubslice, None, RequiresViewAndEstablishesBorrowedView),
    ByteSequenceRead => (Byte, ExactImmutableByteView, ReadByteSequence, None, RequiresAndKeepsStructuralPlace),
    ByteSequenceWrite => (Unit, ExactMutableByteView, WriteByteSequence, None, RequiresAndKeepsStructuralPlace),
    ByteSequenceLength => (ByteCount, ExactByteViewMetadata, ReadByteSequenceLength, None, RequiresAndKeepsStructuralPlace),
    WriteOnlyPrimitiveStore => (Unit, ExactWriteOnlyPrimitiveRoot, StorePrimitive, None, RequiresAndKeepsWriteOnlyPrimitivePlace),
    StructuralScalarFieldStore => (Unit, ExactStructuralScalarField, StoreScalarField, None, RequiresAndKeepsStructuralPlace),
    EstablishScalarCase => (Structural, ExactScalarCase, EstablishScalarCase, None, AddsOwnedPlace),
    EstablishStructuralCase => (Structural, ExactStructuralCase, EstablishStructuralCase, None, TransfersOwnedChildrenAndAddsOwnedPlace),
    EstablishByteSequenceLiteral => (Unit, ExactByteSequenceLiteral, EstablishByteSequencePlace, None, AddsUnrestrictedPlace),
    BooleanStructuralField => (Boolean, ExactLiveBooleanField, ReadBooleanField, None, RequiresAndKeepsAffinePlace),
    ScalarStructuralField => (Scalar, ExactLiveScalarField, ReadScalarField, None, RequiresAndKeepsStructuralPlace),
    PortWrite => (Unit, ExactPublishedService, EmitPortWrite, PortWrite, KeepsPlaceFrontier),
    EstablishTrivialAffineLocal => (Unit, ExactEmptyAffineLocal, EstablishAffinePlace, None, AddsAffinePlace),
    EstablishStructuralArray => (Structural, ExactStructuralArray, EstablishStructuralArray, None, TransfersOwnedChildrenAndAddsOwnedPlace),
    EstablishRecord => (Structural, ExactRecord, EstablishRecord, None, TransfersOwnedChildrenAndAddsOwnedPlace),
}

pub fn exact_structural_effect_semantic_row_in(
    tag: OperationSemanticTag,
    rows: &[StructuralEffectSemanticRow],
) -> Result<Option<&StructuralEffectSemanticRow>, OperationSemanticError> {
    if !is_structural_effect_tag(tag) {
        return Ok(None);
    }
    crate::semantic_rows::exactly_one_row_for_tag(
        rows,
        tag,
        |row| row.tag,
        OperationSemanticError::MissingStructuralEffectRow,
        OperationSemanticError::DuplicateStructuralEffectRow,
    )
    .map(Some)
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
        OperationSemanticTag::EstablishPlacedReference,
        OperationSemanticTag::EstablishReference,
        OperationSemanticTag::ReleaseReference,
        OperationSemanticTag::EstablishPrimitiveLocal,
        OperationSemanticTag::PrimitiveScalarRead,
        OperationSemanticTag::StructuralCaseMembership,
        OperationSemanticTag::WriteOnlyPrimitiveStore,
        OperationSemanticTag::StructuralScalarFieldStore,
        OperationSemanticTag::StructuralByteSequenceFieldStore,
        OperationSemanticTag::StructuralByteViewFieldStore,
        OperationSemanticTag::ByteSequenceStore,
        OperationSemanticTag::StructuralByteSequenceFieldLength,
        OperationSemanticTag::StructuralByteSequenceFieldByteStore,
        OperationSemanticTag::StructuralByteSequenceFieldRead,
        OperationSemanticTag::EstablishScalarCase,
        OperationSemanticTag::EstablishStructuralCase,
        OperationSemanticTag::EstablishScalarArray,
        OperationSemanticTag::EstablishByteSequenceLiteral,
        OperationSemanticTag::ByteSequenceLength,
        OperationSemanticTag::ByteSequenceRead,
        OperationSemanticTag::ByteSequenceWrite,
        OperationSemanticTag::ByteSequenceSubslice,
        OperationSemanticTag::ByteSequencePredicate,
        OperationSemanticTag::ByteSequenceEqual,
        OperationSemanticTag::EstablishElementView,
        OperationSemanticTag::EstablishMutableElementView,
        OperationSemanticTag::ElementViewLength,
        OperationSemanticTag::ElementViewRead,
        OperationSemanticTag::ElementViewSubslice,
        OperationSemanticTag::ElementViewWrite,
        OperationSemanticTag::BooleanStructuralField,
        OperationSemanticTag::ScalarStructuralField,
        OperationSemanticTag::PortWrite,
        OperationSemanticTag::EstablishTrivialAffineLocal,
        OperationSemanticTag::EstablishStructuralArray,
        OperationSemanticTag::EstablishRecord,
    ] {
        let row = exact_structural_effect_semantic_row_in(tag, rows)?
            .expect("the requested tag belongs to the structural/effect cohort");
        validate_structural_effect_schema(tag, row.schema)?;
    }
    Ok(())
}

fn validate_structural_effect_schema(
    tag: OperationSemanticTag,
    schema: StructuralEffectLeafSchema,
) -> Result<(), OperationSemanticError> {
    let action_tag = match schema.action {
        StructuralEffectAction::EstablishPlacedReference => {
            OperationSemanticTag::EstablishPlacedReference
        }
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
        StructuralEffectAction::ReadByteSequenceFieldByte => {
            OperationSemanticTag::StructuralByteSequenceFieldRead
        }
        StructuralEffectAction::StoreByteSequence => OperationSemanticTag::ByteSequenceStore,
        StructuralEffectAction::StoreByteSequenceField => {
            OperationSemanticTag::StructuralByteSequenceFieldStore
        }
        StructuralEffectAction::StoreByteViewField => {
            OperationSemanticTag::StructuralByteViewFieldStore
        }
        StructuralEffectAction::EstablishByteSequenceSubslice => {
            OperationSemanticTag::ByteSequenceSubslice
        }
        StructuralEffectAction::EvaluateByteSequencePredicate => {
            OperationSemanticTag::ByteSequencePredicate
        }
        StructuralEffectAction::EvaluateByteSequenceEqual => {
            OperationSemanticTag::ByteSequenceEqual
        }
        StructuralEffectAction::EstablishElementView => OperationSemanticTag::EstablishElementView,
        StructuralEffectAction::EstablishMutableElementView => {
            OperationSemanticTag::EstablishMutableElementView
        }
        StructuralEffectAction::ReadElementViewLength => OperationSemanticTag::ElementViewLength,
        StructuralEffectAction::ReadElementView => OperationSemanticTag::ElementViewRead,
        StructuralEffectAction::WriteElementView => OperationSemanticTag::ElementViewWrite,
        StructuralEffectAction::EstablishElementViewSubslice => {
            OperationSemanticTag::ElementViewSubslice
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
        StructuralEffectAction::ReadScalarField => OperationSemanticTag::ScalarStructuralField,
        StructuralEffectAction::EmitPortWrite => OperationSemanticTag::PortWrite,
        StructuralEffectAction::EstablishAffinePlace => {
            OperationSemanticTag::EstablishTrivialAffineLocal
        }
        StructuralEffectAction::EstablishStructuralArray => {
            OperationSemanticTag::EstablishStructuralArray
        }
        StructuralEffectAction::EstablishRecord => OperationSemanticTag::EstablishRecord,
        StructuralEffectAction::EstablishScalarCase => OperationSemanticTag::EstablishScalarCase,
        StructuralEffectAction::EstablishStructuralCase => {
            OperationSemanticTag::EstablishStructuralCase
        }
        StructuralEffectAction::EstablishScalarArray => OperationSemanticTag::EstablishScalarArray,
    };
    let expected_goal = match schema.action {
        StructuralEffectAction::StoreByteSequence => {
            StructuralEffectGoalShape::ByteLengthWithinOwnerCapacity
        }
        StructuralEffectAction::StoreByteSequenceField => {
            StructuralEffectGoalShape::ByteLengthWithinFieldCapacity
        }
        StructuralEffectAction::EstablishByteSequenceSubslice => {
            StructuralEffectGoalShape::ByteRangeInBounds
        }
        StructuralEffectAction::ReadByteSequence
        | StructuralEffectAction::WriteByteSequence
        | StructuralEffectAction::StoreByteSequenceFieldByte
        | StructuralEffectAction::ReadByteSequenceFieldByte => {
            StructuralEffectGoalShape::ByteIndexInBounds
        }
        StructuralEffectAction::ReadElementView | StructuralEffectAction::WriteElementView => {
            StructuralEffectGoalShape::ElementIndexInBounds
        }
        StructuralEffectAction::EstablishElementViewSubslice => {
            StructuralEffectGoalShape::ElementRangeInBounds
        }
        _ => StructuralEffectGoalShape::None,
    };
    let valid =
        action_tag == tag
            && schema.goal == expected_goal
            && match schema.action {
                StructuralEffectAction::EstablishPlacedReference
                | StructuralEffectAction::EstablishReference => {
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
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::ReleasesReferenceCarrier
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
                        && schema.custody == StructuralEffectCustody::ExactReadableSumProjection
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::EvaluateByteSequencePredicate
                | StructuralEffectAction::EvaluateByteSequenceEqual => {
                    schema.result == StructuralEffectResultShape::Boolean
                        && schema.custody == StructuralEffectCustody::ExactImmutableByteView
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::ReadByteSequenceFieldLength
                | StructuralEffectAction::StoreByteSequenceFieldByte
                | StructuralEffectAction::ReadByteSequenceFieldByte => {
                    schema.result
                        == match schema.action {
                            StructuralEffectAction::ReadByteSequenceFieldLength => {
                                StructuralEffectResultShape::ByteCount
                            }
                            StructuralEffectAction::ReadByteSequenceFieldByte => {
                                StructuralEffectResultShape::Byte
                            }
                            _ => StructuralEffectResultShape::Unit,
                        }
                        && schema.custody
                            == StructuralEffectCustody::ExactStructuralByteSequenceField
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::StoreByteSequence => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody == StructuralEffectCustody::ExactBoundedByteOwner
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::StoreByteSequenceField
                | StructuralEffectAction::StoreByteViewField => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody
                            == StructuralEffectCustody::ExactStructuralByteSequenceField
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::EstablishByteSequenceSubslice => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactImmutableByteView
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresViewAndEstablishesBorrowedView,
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
                StructuralEffectAction::EstablishElementView
                | StructuralEffectAction::EstablishMutableElementView => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactElementViewSource
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresViewAndEstablishesBorrowedView,
                StructuralEffectAction::ReadElementViewLength => {
                    schema.result == StructuralEffectResultShape::ElementCount
                        && schema.custody == StructuralEffectCustody::ExactElementViewMetadata
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::ReadElementView => {
                    schema.result == StructuralEffectResultShape::Scalar
                        && schema.custody == StructuralEffectCustody::ExactImmutableElementView
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::WriteElementView => {
                    schema.result == StructuralEffectResultShape::Unit
                        && schema.custody == StructuralEffectCustody::ExactMutableElementView
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier
                            == StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
                }
                StructuralEffectAction::EstablishElementViewSubslice => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactImmutableElementView
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresViewAndEstablishesBorrowedView,
                StructuralEffectAction::StorePrimitive => schema.result
                    == StructuralEffectResultShape::Unit
                    && schema.custody == StructuralEffectCustody::ExactWriteOnlyPrimitiveRoot
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::RequiresAndKeepsWriteOnlyPrimitivePlace,
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
                StructuralEffectAction::ReadScalarField => {
                    schema.result == StructuralEffectResultShape::Scalar
                        && schema.custody == StructuralEffectCustody::ExactLiveScalarField
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
                StructuralEffectAction::EstablishStructuralArray => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactStructuralArray
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::TransfersOwnedChildrenAndAddsOwnedPlace,
                StructuralEffectAction::EstablishRecord => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactRecord
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::TransfersOwnedChildrenAndAddsOwnedPlace,
                StructuralEffectAction::EstablishScalarCase => {
                    schema.result == StructuralEffectResultShape::Structural
                        && schema.custody == StructuralEffectCustody::ExactScalarCase
                        && schema.external_effect == StructuralEffectExternalEffect::None
                        && schema.frontier == StructuralEffectFrontierPolicy::AddsOwnedPlace
                }
                StructuralEffectAction::EstablishStructuralCase => schema.result
                    == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactStructuralCase
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier
                        == StructuralEffectFrontierPolicy::TransfersOwnedChildrenAndAddsOwnedPlace,
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
        StructuralEffectResultShape::ByteCount | StructuralEffectResultShape::ElementCount => {
            operation.result.scalar_ref().is_some_and(|result| {
                matches!(result.scalar_type, ScalarType::Integer(integer) if integer == IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"))
            })
        }
        StructuralEffectResultShape::Boolean => operation
            .result
            .scalar_ref()
            .is_some_and(|result| result.scalar_type == ScalarType::Boolean),
        StructuralEffectResultShape::Integer => operation
            .result
            .scalar_ref()
            .is_some_and(|result| matches!(result.scalar_type, ScalarType::Integer(_))),
        StructuralEffectResultShape::Structural => operation.result.structural().is_some(),
        StructuralEffectResultShape::Unit => matches!(operation.result, OperationResult::Unit)
    };
    valid
        .then_some(())
        .ok_or(OperationSemanticError::StructuralEffectResultShapeMismatch(
            tag,
        ))
}

/// Shared subslice extent equation: when `observation` is the read of a
/// subslice `producer` output over the same source, the measured extent is
/// `end - start`. The closures extract the per-family variant fields.
pub(crate) fn subslice_extent_equation(
    observation: &StructuralEffectObservation,
    producer: &StructuralEffectObservation,
    read: impl Fn(&StructuralEffectObservation) -> Option<(&terminal_psi::StructuralArgument, ValueId)>,
    subslice: impl Fn(&StructuralEffectObservation) -> Option<(PlaceId, ValueId, ValueId)>,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let Some((source, result)) = read(observation) else {
        return Ok(None);
    };
    let Some((destination, start, end)) = subslice(producer) else {
        return Ok(None);
    };
    if source.place != destination || !source.path.is_empty() {
        return Ok(None);
    }
    u64_extent_difference_proposition(result, end, start)
}
