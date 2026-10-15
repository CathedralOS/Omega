use super::*;
use terminal_psi::StructuralArgument;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralEffectObservation {
    ReferenceEstablished {
        source: terminal_psi::StructuralArgument,
        destination: PlaceId,
    },
    ReferenceReleased {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
    },
    ScalarArrayEstablished {
        destination: PlaceId,
        elements: Vec<ValueId>,
    },
    PrimitiveLocalEstablished {
        destination: PlaceId,
        value: ValueId,
        range_obligation: Option<semantic_vocabulary::ObligationId>,
    },
    /// Read one primitive leaf. A runtime-selected element in `path` names no
    /// single leaf, so the verifier retains an element-value equation only
    /// for a static path.
    PrimitiveRead {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        result: ValueId,
    },
    /// Captures one tag observation without a reusable current-storage fact.
    CaseMembershipObserved {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
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
    /// Read one byte from the exact field's live storage. The result is the
    /// read value; index < length is proven against the field's own length
    /// observation and provenance is artifact validation, not an equation.
    ByteSequenceFieldByteRead {
        source: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: StructuralFieldId,
        index: ValueId,
        length: ValueId,
        result: ValueId,
        obligation: ObligationId,
    },
    ByteSequenceStored {
        destination: PlaceId,
        source: PlaceId,
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
    /// Replace one shared view descriptor slot with the source view: a
    /// record's field, or the whole destination when `field` is absent. The
    /// slot's extent becomes the source's; no bytes move and no goal arises.
    ByteViewFieldStored {
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        field: Option<StructuralFieldId>,
        source: PlaceId,
    },
    /// Store one primitive leaf. A runtime-selected element in `path` names
    /// no single leaf, so the store forgets the facts beneath its root.
    PrimitiveStored {
        destination: PlaceId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        value: semantic_vocabulary::ValueId,
    },
    /// Store one defined scalar into one exact relevant field. The verifier
    /// publishes the local `field == value` equation itself because naming
    /// the leaf requires the independently resolved canonical write path and
    /// the stored value's scalar type.
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
        source: StructuralArgument,
        result: ValueId,
    },
    /// Evaluate one declared predicate over the operand's exact live extent.
    /// Reads stay inside the operand's own dominating length observation, so
    /// no index certificate exists; the result is the predicate's verdict.
    ByteSequencePredicateEvaluated {
        operand: terminal_psi::control_flow::ByteSequenceOperand,
        predicate: terminal_psi::ByteSequencePredicate,
        result: ValueId,
    },
    /// Content equality between two exact live extents: equal lengths and
    /// equal bytes at every live position. Each operand's reads stay inside
    /// its own dominating length observation; neither place is consumed.
    ByteSequenceEqualEvaluated {
        left: terminal_psi::control_flow::ByteSequenceOperand,
        right: terminal_psi::control_flow::ByteSequenceOperand,
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
        source: StructuralArgument,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        destination: PlaceId,
        obligation: ObligationId,
    },
    /// Establish a whole borrowed element view over a contiguous structural
    /// collection; the source argument carries the root and projection path.
    ElementViewEstablished {
        source: terminal_psi::StructuralArgument,
        destination: PlaceId,
        region: terminal_psi::ElementViewRegion,
    },
    ElementViewLengthRead {
        source: terminal_psi::StructuralArgument,
        result: ValueId,
    },
    /// Read one element from the exact immutable element view after
    /// discharging index < length counted in elements.
    ElementViewRead {
        source: PlaceId,
        index: ValueId,
        length: ValueId,
        result: ValueId,
        obligation: ObligationId,
    },
    /// Store one scalar element into the exact mutable element view after
    /// discharging index < length counted in elements.
    ElementViewWrite {
        destination: StructuralArgument,
        path: Vec<terminal_psi::StructuralPathSegment>,
        index: ValueId,
        value: ValueId,
        length: ValueId,
        obligation: ObligationId,
    },
    ElementViewSubslice {
        source: StructuralArgument,
        start: ValueId,
        end: ValueId,
        length: ValueId,
        destination: PlaceId,
        obligation: ObligationId,
    },
    BooleanFieldEquation(Proposition),
    ScalarFieldEquation(Proposition),
    PortWrite {
        service: ServiceId,
        port: u16,
        value: u8,
    },
    AffinePlaceEstablished {
        destination: PlaceId,
    },
    StructuralArrayEstablished {
        destination: PlaceId,
        elements: Vec<terminal_psi::StructuralArgument>,
    },
    RecordEstablished {
        destination: PlaceId,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    StructuralCaseEstablished {
        membership: Proposition,
        destination: PlaceId,
        fields: Vec<terminal_psi::RecordFieldInitializer>,
    },
    ScalarCaseEstablished {
        membership: Proposition,
        fields: Vec<terminal_psi::ScalarCaseField>,
    },
}

impl StructuralEffectObservation {}

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
            OperationKind::EstablishReference { source, .. },
        )
        | (
            StructuralEffectAction::EstablishPlacedReference,
            OperationKind::EstablishPlacedReference { source },
        ) => {
            let result = operation.result.structural().ok_or(
                OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
            )?;
            if !matches!(
                source.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::SharedBorrow
            ) || result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                // Membership belongs below the borrowed boundary. Source
                // transfer validation authenticates these occurrence rows;
                // this operation-shape judgment grants none by itself.
                || result.projected_qualifications.iter().any(|row| {
                    row.path.first() != Some(&terminal_psi::StructuralPathSegment::Referent)
                })
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
        (
            StructuralEffectAction::ReleaseReference,
            OperationKind::ReleaseReference { source, path },
        ) => StructuralEffectObservation::ReferenceReleased {
            source: *source,
            path: path.clone(),
        },
        (
            StructuralEffectAction::EstablishPrimitiveLocal,
            OperationKind::EstablishPrimitiveLocal {
                value,
                range_obligation,
            },
        ) => {
            let result = operation.result.structural().ok_or(
                OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
            )?;
            StructuralEffectObservation::PrimitiveLocalEstablished {
                destination: result.place,
                value: *value,
                range_obligation: *range_obligation,
            }
        }
        (
            StructuralEffectAction::ReadPrimitive,
            OperationKind::PrimitiveScalarRead { source, path },
        ) => {
            let result = operation.result.scalar().ok_or(
                OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
            )?;
            StructuralEffectObservation::PrimitiveRead {
                source: *source,
                path: path.clone(),
                result: result.id,
            }
        }
        (
            StructuralEffectAction::ObserveCaseMembership,
            OperationKind::StructuralCaseMembership { source, path, case },
        ) => StructuralEffectObservation::CaseMembershipObserved {
            source: *source,
            path: path.clone(),
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
            StructuralEffectAction::ReadByteSequenceFieldByte,
            OperationKind::StructuralByteSequenceFieldRead {
                source,
                path,
                field,
                index,
                length,
                obligation,
            },
        ) => StructuralEffectObservation::ByteSequenceFieldByteRead {
            source: *source,
            path: path.clone(),
            field: *field,
            index: *index,
            length: *length,
            result: operation.result.expect_scalar().id,
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
            source: source.clone(),
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
            StructuralEffectAction::EstablishElementView,
            OperationKind::EstablishElementView {
                destination,
                source,
                region,
                ..
            },
        )
        | (
            StructuralEffectAction::EstablishMutableElementView,
            OperationKind::EstablishMutableElementView {
                destination,
                source,
                region,
                ..
            },
        ) => {
            operation.result.structural().ok_or(
                OperationSemanticError::StructuralEffectResultShapeMismatch(tag),
            )?;
            StructuralEffectObservation::ElementViewEstablished {
                source: source.clone(),
                destination: *destination,
                region: *region,
            }
        }
        (
            StructuralEffectAction::ReadElementViewLength,
            OperationKind::ElementViewLength { source },
        ) => StructuralEffectObservation::ElementViewLengthRead {
            source: source.clone(),
            result: operation.result.expect_scalar().id,
        },
        (
            StructuralEffectAction::ReadElementView,
            OperationKind::ElementViewRead {
                source,
                index,
                length,
                obligation,
                ..
            },
        ) => StructuralEffectObservation::ElementViewRead {
            source: *source,
            index: *index,
            length: *length,
            result: operation.result.expect_scalar().id,
            obligation: *obligation,
        },
        (
            StructuralEffectAction::EstablishElementViewSubslice,
            OperationKind::ElementViewSubslice {
                source,
                start,
                end,
                length,
                obligation,
            },
        ) => StructuralEffectObservation::ElementViewSubslice {
            source: source.clone(),
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
            source: source.clone(),
            result: operation.result.expect_scalar().id,
        },
        (
            StructuralEffectAction::EvaluateByteSequencePredicate,
            OperationKind::ByteSequencePredicate { operand, predicate },
        ) => StructuralEffectObservation::ByteSequencePredicateEvaluated {
            operand: operand.clone(),
            predicate: *predicate,
            result: operation.result.expect_scalar().id,
        },
        (
            StructuralEffectAction::EvaluateByteSequenceEqual,
            OperationKind::ByteSequenceEqual { left, right },
        ) => StructuralEffectObservation::ByteSequenceEqualEvaluated {
            left: left.clone(),
            right: right.clone(),
            result: operation.result.expect_scalar().id,
        },
        (
            StructuralEffectAction::WriteElementView,
            OperationKind::ElementViewWrite {
                destination,
                path,
                index,
                value,
                length,
                obligation,
            },
        ) => StructuralEffectObservation::ElementViewWrite {
            destination: destination.clone(),
            path: path.clone(),
            index: *index,
            value: *value,
            length: *length,
            obligation: *obligation,
        },
        (
            StructuralEffectAction::StorePrimitive,
            OperationKind::WriteOnlyPrimitiveStore {
                destination,
                value,
                path,
                ..
            },
        ) => StructuralEffectObservation::PrimitiveStored {
            destination: *destination,
            path: path.clone(),
            value: *value,
        },
        (
            StructuralEffectAction::StoreByteSequence,
            OperationKind::ByteSequenceStore {
                destination,
                source,
                length,
                obligation,
            },
        ) => StructuralEffectObservation::ByteSequenceStored {
            destination: *destination,
            source: *source,
            length: *length,
            obligation: *obligation,
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
            StructuralEffectAction::StoreByteViewField,
            OperationKind::StructuralByteViewFieldStore {
                destination,
                path,
                field,
                source,
                ..
            },
        ) => StructuralEffectObservation::ByteViewFieldStored {
            destination: *destination,
            path: path.clone(),
            field: *field,
            source: *source,
        },
        (
            StructuralEffectAction::StoreScalarField,
            OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                ..
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
            StructuralEffectAction::EstablishStructuralCase,
            OperationKind::EstablishStructuralCase {
                result_case,
                fields,
            },
        ) => {
            let result = operation
                .result
                .structural()
                .expect("validated structural-case structural result");
            StructuralEffectObservation::StructuralCaseEstablished {
                membership: Proposition::StructuralCaseMembership {
                    subject: StructuralCaseSubject::new(result.place, Vec::new()),
                    case: *result_case,
                },
                destination: result.place,
                fields: fields.clone(),
            }
        }
        (
            StructuralEffectAction::ReadScalarField,
            OperationKind::ScalarStructuralField {
                source,
                path,
                field,
            },
        ) => {
            let result = operation
                .result
                .scalar_ref()
                .expect("validated scalar structural-field result");
            let field_path = path
                .iter()
                .copied()
                .chain(std::iter::once(
                    semantic_vocabulary::CanonicalStructuralPathSegment::Field(*field),
                ))
                .collect();
            let field_term = match result.scalar_type {
                ScalarType::Integer(integer) => {
                    ScalarTerm::integer_field_path(*source, field_path, integer)
                }
                ScalarType::IeeeFloat(format) => {
                    ScalarTerm::ieee_float_field_path(*source, field_path, format)
                }
                ScalarType::Boolean => ScalarTerm::boolean_field_path(*source, field_path),
            };
            // Read denotation connects the current place observation to its SSA
            // snapshot. Module validation owns field/carrier and live-custody
            // checks; this equation grants no range or construction authority.
            StructuralEffectObservation::ScalarFieldEquation(Proposition::Equal(
                ScalarTerm::value(result.id, result.scalar_type),
                field_term,
            ))
        }
        (
            StructuralEffectAction::EstablishByteSequencePlace,
            OperationKind::EstablishByteSequenceLiteral { destination, .. },
        ) => StructuralEffectObservation::ByteSequencePlaceEstablished {
            destination: *destination,
        },
        (
            StructuralEffectAction::ReadBooleanField,
            OperationKind::BooleanStructuralField {
                source,
                path,
                field,
            },
        ) => {
            let result = operation
                .result
                .scalar_ref()
                .expect("validated Boolean structural-field result");
            StructuralEffectObservation::BooleanFieldEquation(Proposition::Equal(
                ScalarTerm::value(result.id, result.scalar_type),
                ScalarTerm::boolean_field_path(
                    *source,
                    path.iter()
                        .copied()
                        .chain(std::iter::once(
                            semantic_vocabulary::CanonicalStructuralPathSegment::Field(*field),
                        ))
                        .collect(),
                ),
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
            StructuralEffectAction::EstablishStructuralArray,
            OperationKind::EstablishStructuralArray { elements },
        ) => {
            let result = operation
                .result
                .structural()
                .expect("validated complete array result");
            StructuralEffectObservation::StructuralArrayEstablished {
                destination: result.place,
                elements: elements.clone(),
            }
        }
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

/// The same captured descriptor coordinates across two observations. Each
/// operation owns and independently validates its selector obligation; that
/// evidence identity does not change the already evaluated index value.
/// Access and all other path segments remain exact. This is correspondence,
/// never liveness, bounds, loan or mutation authority.
pub fn same_descriptor_coordinate(
    left: &terminal_psi::StructuralArgument,
    right: &terminal_psi::StructuralArgument,
) -> bool {
    left.place == right.place
        && left.access == right.access
        && left.path.len() == right.path.len()
        && left
            .path
            .iter()
            .zip(&right.path)
            .all(|(left, right)| match (left, right) {
                (
                    terminal_psi::StructuralPathSegment::RuntimeIndex { index: left, .. },
                    terminal_psi::StructuralPathSegment::RuntimeIndex { index: right, .. },
                ) => left == right,
                _ => left == right,
            })
}
