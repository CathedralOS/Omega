//! Exact structural/effect leaf schemas and their local observations.

use psi_core::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, PlaceId, Proposition, ScalarTerm,
    ScalarType, ServiceId, StructuralCaseSubject, StructuralFieldId, ValueId,
};
use psi_terminal::{Operation, OperationKind, OperationResult};

use super::{OperationSemanticError, OperationSemanticTag};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectResultShape {
    Boolean,
    Integer,
    Structural,
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectCustody {
    ExactWriteOnlyPrimitiveRoot,
    ExactStructuralScalarField,
    ExactByteSequenceLiteral,
    ExactLiveBooleanField,
    ExactLiveIntegerField,
    ExactPublishedService,
    ExactEmptyAffineLocal,
    ExactAffineScalarRecord,
    ExactPayloadlessCase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralEffectAction {
    StorePrimitive,
    StoreScalarField,
    EstablishByteSequencePlace,
    ReadBooleanField,
    ReadIntegerField,
    EmitPortWrite,
    EstablishAffinePlace,
    EstablishAffineScalarRecord,
    EstablishPayloadlessCase,
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
pub enum StructuralEffectFrontierPolicy {
    RequiresAndKeepsWriteOnlyPrimitivePlace,
    RequiresAndKeepsStructuralPlace,
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
        frontier,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralEffectSemanticRow {
    tag: OperationSemanticTag,
    schema: StructuralEffectLeafSchema,
}

impl StructuralEffectSemanticRow {
    pub const ALL: [Self; 9] = [
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
            tag: OperationSemanticTag::EstablishPayloadlessCase,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Structural,
                StructuralEffectCustody::ExactPayloadlessCase,
                StructuralEffectAction::EstablishPayloadlessCase,
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
            tag: OperationSemanticTag::EstablishAffineScalarRecord,
            schema: structural_effect_leaf(
                StructuralEffectResultShape::Structural,
                StructuralEffectCustody::ExactAffineScalarRecord,
                StructuralEffectAction::EstablishAffineScalarRecord,
                StructuralEffectExternalEffect::None,
                StructuralEffectFrontierPolicy::AddsAffinePlace,
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
        OperationSemanticTag::WriteOnlyPrimitiveStore
            | OperationSemanticTag::StructuralScalarFieldStore
            | OperationSemanticTag::EstablishPayloadlessCase
            | OperationSemanticTag::EstablishByteSequenceLiteral
            | OperationSemanticTag::BooleanStructuralField
            | OperationSemanticTag::IntegerStructuralField
            | OperationSemanticTag::PortWrite
            | OperationSemanticTag::EstablishTrivialAffineLocal
            | OperationSemanticTag::EstablishAffineScalarRecord
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
        OperationSemanticTag::WriteOnlyPrimitiveStore,
        OperationSemanticTag::StructuralScalarFieldStore,
        OperationSemanticTag::EstablishPayloadlessCase,
        OperationSemanticTag::EstablishByteSequenceLiteral,
        OperationSemanticTag::BooleanStructuralField,
        OperationSemanticTag::IntegerStructuralField,
        OperationSemanticTag::PortWrite,
        OperationSemanticTag::EstablishTrivialAffineLocal,
        OperationSemanticTag::EstablishAffineScalarRecord,
    ] {
        let row = exact_structural_effect_semantic_row_in(tag, rows)?
            .expect("the requested tag belongs to the structural/effect cohort");
        validate_structural_effect_schema(tag, row.schema)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralEffectObservation {
    PrimitiveStored {
        destination: PlaceId,
        value: psi_core::ValueId,
    },
    ScalarFieldStored {
        destination: PlaceId,
        path: Vec<psi_terminal::StructuralPathSegment>,
        field: StructuralFieldId,
        value: ValueId,
    },
    ByteSequencePlaceEstablished {
        destination: PlaceId,
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
    AffineScalarRecordEstablished {
        destination: PlaceId,
        equation: Proposition,
    },
    PayloadlessCaseEstablished(Proposition),
}

impl StructuralEffectObservation {
    pub fn local_equation(&self) -> Option<&Proposition> {
        match self {
            Self::BooleanFieldEquation(proposition)
            | Self::PayloadlessCaseEstablished(proposition)
            | Self::AffineScalarRecordEstablished {
                equation: proposition,
                ..
            } => Some(proposition),
            Self::PrimitiveStored { .. }
            | Self::ScalarFieldStored { .. }
            | Self::ByteSequencePlaceEstablished { .. }
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
        StructuralEffectAction::EstablishAffineScalarRecord => {
            OperationSemanticTag::EstablishAffineScalarRecord
        }
        StructuralEffectAction::EstablishPayloadlessCase => {
            OperationSemanticTag::EstablishPayloadlessCase
        }
    };
    let valid = action_tag == tag
        && match schema.action {
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
            StructuralEffectAction::EstablishAffineScalarRecord => {
                schema.result == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactAffineScalarRecord
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier == StructuralEffectFrontierPolicy::AddsAffinePlace
            }
            StructuralEffectAction::EstablishPayloadlessCase => {
                schema.result == StructuralEffectResultShape::Structural
                    && schema.custody == StructuralEffectCustody::ExactPayloadlessCase
                    && schema.external_effect == StructuralEffectExternalEffect::None
                    && schema.frontier == StructuralEffectFrontierPolicy::AddsOwnedPlace
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
            StructuralEffectAction::StorePrimitive,
            OperationKind::WriteOnlyPrimitiveStore { destination, value },
        ) => StructuralEffectObservation::PrimitiveStored {
            destination: *destination,
            value: *value,
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
            StructuralEffectAction::EstablishPayloadlessCase,
            OperationKind::EstablishPayloadlessCase { result_case },
        ) => {
            let result = operation
                .result
                .structural()
                .expect("validated payloadless-case structural result");
            StructuralEffectObservation::PayloadlessCaseEstablished(
                Proposition::StructuralCaseMembership {
                    subject: StructuralCaseSubject::new(result.place, Vec::new()),
                    case: *result_case,
                },
            )
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
        (
            StructuralEffectAction::EstablishAffineScalarRecord,
            OperationKind::EstablishAffineScalarRecord { field, value },
        ) => {
            let result = operation
                .result
                .structural()
                .expect("validated affine scalar-record structural result");
            let integer_type = IntegerType::new(IntegerSign::Signed, 64)
                .expect("signed i64 is a valid fixed integer type");
            let field_term = ScalarTerm::integer_field_path(
                result.place,
                vec![CanonicalStructuralPathSegment::Field(*field)],
                integer_type,
            );
            let literal_term = ScalarTerm::integer(integer_type, *value)
                .map_err(OperationSemanticError::InvalidProposition)?;
            StructuralEffectObservation::AffineScalarRecordEstablished {
                destination: result.place,
                equation: Proposition::Equal(field_term, literal_term),
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

    use psi_core::{OperationId, StructuralFieldId, ValueId};
    use psi_terminal::{OperationResult, ValueDeclaration};

    use super::*;

    #[test]
    fn inventory_is_exact_unique_and_keeps_axes_separate() {
        assert_eq!(StructuralEffectSemanticRow::ALL.len(), 9);
        assert_eq!(
            StructuralEffectSemanticRow::ALL
                .iter()
                .map(|row| row.tag())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                OperationSemanticTag::WriteOnlyPrimitiveStore,
                OperationSemanticTag::StructuralScalarFieldStore,
                OperationSemanticTag::EstablishPayloadlessCase,
                OperationSemanticTag::EstablishByteSequenceLiteral,
                OperationSemanticTag::BooleanStructuralField,
                OperationSemanticTag::IntegerStructuralField,
                OperationSemanticTag::PortWrite,
                OperationSemanticTag::EstablishTrivialAffineLocal,
                OperationSemanticTag::EstablishAffineScalarRecord,
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
    fn primitive_store_observation_binds_destination_and_value_without_a_fact() {
        let operation = Operation {
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
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
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
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
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
                    id: OperationId::new(2).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
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
