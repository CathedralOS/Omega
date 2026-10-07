//! Lowering checked scalar and boolean expressions.
//!
//! Bounds decisions join each retained selector's exact authored occurrence
//! while preparing that read, regardless of its surrounding expression. The
//! row root is not a selector identity: projections, comparisons and nested
//! indexing contain independent reads. Binding rebases preserve the selected
//! storage and value; generated selectors have no authored occurrence. In
//! particular, a representation view's byte expansion cannot inherit this
//! decision for the additional offsets it creates.

use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, LoweredElementPathSegment,
};
use crate::emission::operation_emission::integer::{
    LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
};
use crate::emission::scalar_types::{
    integer_landing_scalar_type, integer_value, terminal_scalar_type,
};
use crate::expression_preparation::bindings::structural_fields::CasePayloadRead;
use crate::expression_preparation::bindings::view_locals::{self, ViewCarrier, ViewLocalBinding};
use crate::expression_preparation::saturating_cast;
use crate::expression_preparation::wrapping_cast;
use crate::expression_preparation::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees, LoweringError, PlaceId,
    PrimitiveType, ScalarType, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeId, unsupported,
};

pub(crate) fn lower_checked_scalar_expression_at(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
    role: CheckedScalarExpressionRole,
) -> Result<LoweredDirectExpression, LoweringError> {
    let (_, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(state, statement_ordinal, role)
        .ok_or(LoweringError::Unsupported(
            "scalar expression has no unique source-bound checked value plan",
        ))?;
    lower_checked_scalar_expression(expression, &checked.facts.proven_scalar_indexes)
}

pub(crate) fn lower_checked_scalar_expression(
    expression: &CheckedScalarExpression,
    proven_indexes: &[checked_trees::expression::ExpressionHandle],
) -> Result<LoweredDirectExpression, LoweringError> {
    lower_checked_scalar_expression_with_parameters(
        expression,
        &[],
        &Default::default(),
        &[],
        &[],
        &std::collections::BTreeMap::new(),
        &[],
        &[],
        None,
        proven_indexes,
    )
}

/// `lower_checked_scalar_expression_at` carrying the callee's authored
/// structural parameter namespace, so a whole byte-view length observation
/// resolves its exact retained source place instead of failing closed.
pub(crate) fn lower_checked_scalar_expression_at_with_parameters(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
    role: CheckedScalarExpressionRole,
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
) -> Result<LoweredDirectExpression, LoweringError> {
    let (_, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(state, statement_ordinal, role)
        .ok_or(LoweringError::Unsupported(
            "scalar expression has no unique source-bound checked value plan",
        ))?;
    let lowered = lower_checked_scalar_expression_with_parameters(
        expression,
        structural_parameters,
        &Default::default(),
        &[],
        &[],
        &std::collections::BTreeMap::new(),
        &[],
        &[],
        None,
        &checked.facts.proven_scalar_indexes,
    )?;
    Ok(lowered)
}

/// The exact whole sequence parameter for a live length, element or predicate
/// observation. An owned bounded sequence reads its own storage; shared and
/// mutable loans read their retained referent. A write-only loan supplies no
/// observation. Carrier selection remains with the checked read and catalog.
fn readable_sequence_parameter(
    position: u32,
    parameters: &[(u32, StructuralParameterDeclaration)],
) -> Result<&StructuralParameterDeclaration, LoweringError> {
    let parameter = parameters
        .iter()
        .find_map(|(source_position, parameter)| {
            (*source_position == position).then_some(parameter)
        })
        .ok_or(LoweringError::Unsupported(
            "sequence observation has no exact structural parameter",
        ))?;
    if !matches!(
        parameter.access,
        StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
    {
        return unsupported(
            "sequence observation requires readable access to an unrestricted whole carrier",
        );
    }
    Ok(parameter)
}

/// The view family of a whole view parameter: an element view when its type
/// is one, a byte view otherwise.
fn parameter_view_carrier(
    element_views: &std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
    structural_type: StructuralTypeId,
) -> ViewCarrier {
    match element_views.get(&structural_type) {
        Some(element) => ViewCarrier::Elements { element: *element },
        None => ViewCarrier::Bytes,
    }
}

/// A view local is observed whole, at the place its establishment published.
fn view_local_observation(
    view_locals: &[ViewLocalBinding],
    symbol: symbols::SymbolHandle,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<(PlaceId, ViewCarrier), LoweringError> {
    if !path.is_empty() {
        return unsupported("a view local is observed whole, never through a projection");
    }
    let local = view_locals::resolve(view_locals, symbol)?;
    Ok((local.place, local.carrier))
}

/// The place an owned structural local's establishment published, for scalar
/// observations (element reads) rooted at it.
fn owned_local_observation(
    structural_locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    symbol: symbols::SymbolHandle,
) -> Result<PlaceId, LoweringError> {
    let Some(crate::expression_preparation::bindings::StructuralLocalBinding {
        argument: source,
        ..
    }) = structural_locals
        .iter()
        .find(|local| local.root == facts::PlaceRoot::Symbol(symbol))
    else {
        return unsupported("owned local observation lost its established place");
    };
    Ok(source.place)
}

/// Range checking publishes these handles in arena-position/generation order.
/// Consume that ordering without another index allocation or source walk.
fn selector_bound_is_decided(
    selector: &checked_trees::CheckedScalarSelector,
    proven_indexes: &[checked_trees::expression::ExpressionHandle],
) -> bool {
    selector.occurrence.is_valid()
        && proven_indexes
            .binary_search_by_key(
                &checked_trees::operators::handle_position(selector.occurrence),
                |occurrence| checked_trees::operators::handle_position(*occurrence),
            )
            .is_ok()
}

pub(crate) fn lower_checked_scalar_expression_with_parameters(
    expression: &CheckedScalarExpression,
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
    structural_fields: &crate::expression_preparation::bindings::StructuralFieldBindings,
    structural_cases: &[crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding],
    primitive_storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
    element_views: &std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
    view_locals: &[ViewLocalBinding],
    structural_locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    scalar_bindings: Option<&crate::expression_preparation::bindings::ScalarBindings>,
    proven_indexes: &[checked_trees::expression::ExpressionHandle],
) -> Result<LoweredDirectExpression, LoweringError> {
    match expression {
        CheckedScalarExpression::StructuralParameterByteLength { root, path } => {
            let count_type = terminal_scalar_type(PrimitiveType::U64)?;
            if !path.is_empty() {
                let (source, path, field) = byte_field_observation(
                    structural_fields,
                    structural_locals,
                    scalar_bindings,
                    *root,
                    path,
                )?
                .ok_or(LoweringError::Unsupported(
                    "field length requires a live byte or element descriptor",
                ))?;
                return Ok(LoweredDirectExpression::ByteSequenceFieldLength {
                    source,
                    path,
                    field,
                    scalar_type: count_type,
                });
            }
            let (source, carrier) = match *root {
                checked_trees::CheckedStorageRoot::Parameter {
                    index: parameter_position,
                } => {
                    let parameter =
                        readable_sequence_parameter(parameter_position, structural_parameters)?;
                    (
                        parameter.place,
                        parameter_view_carrier(element_views, parameter.structural_type),
                    )
                }
                checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                    view_local_observation(view_locals, symbol, path)?
                }
                checked_trees::CheckedStorageRoot::OwnedLocal { symbol } => {
                    let source = owned_local_observation(structural_locals, symbol)?;
                    let local = crate::expression_preparation::bindings::view_locals::resolve(
                        view_locals,
                        symbol,
                    )?;
                    if !path.is_empty()
                        || local.place != source
                        || local.carrier != ViewCarrier::Bytes
                    {
                        return unsupported("owned byte length lost its established carrier");
                    }
                    (source, ViewCarrier::Bytes)
                }
            };
            Ok(match carrier {
                ViewCarrier::Bytes => LoweredDirectExpression::ByteSequenceLength {
                    source,
                    scalar_type: count_type,
                },
                ViewCarrier::Elements { .. } => LoweredDirectExpression::ElementViewLength {
                    source,
                    scalar_type: count_type,
                },
            })
        }
        CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type,
        } => {
            let scalar_type = terminal_scalar_type(*primitive_type)?;
            if let Some(bindings) = scalar_bindings
                && let Some(observation) =
                    crate::expression_preparation::bindings::primitive_reads::captured_observation(
                        bindings,
                        *symbol,
                        scalar_type,
                    )?
            {
                return Ok(observation);
            }
            let (source, path) = crate::expression_preparation::bindings::primitive_storage_read(
                primitive_storage,
                structural_locals,
                structural_fields.declarations(),
                *symbol,
                scalar_type,
            )?;
            Ok(LoweredDirectExpression::PrimitiveRead {
                source: source.into(),
                path,
                scalar_type,
            })
        }
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => Ok(LoweredDirectExpression::Parameter {
            position: *position,
            scalar_type: terminal_scalar_type(*primitive_type)?,
        }),
        CheckedScalarExpression::ErasedParameter {
            position,
            primitive_type,
        } => Ok(LoweredDirectExpression::ErasedParameter {
            position: *position,
            scalar_type: terminal_scalar_type(*primitive_type)?,
        }),
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => Ok(LoweredDirectExpression::Local {
            position: *position,
            scalar_type: terminal_scalar_type(*primitive_type)?,
        }),
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => {
            let scalar_type = terminal_scalar_type(*primitive_type)?;
            // Float fields are storage observations, not integer field
            // operations or proof-only IEEE equality. Preserve the canonical
            // leaf path so replay checks the same root, access and carrier.
            if matches!(scalar_type, ScalarType::IeeeFloat(_)) {
                let (source, path) =
                    crate::expression_preparation::bindings::structural_fields::resolve_primitive(
                        structural_fields,
                        *parameter_position,
                        path,
                        scalar_type,
                    )?;
                return Ok(LoweredDirectExpression::PrimitiveRead {
                    source,
                    path,
                    scalar_type,
                });
            }
            if !matches!(scalar_type, ScalarType::Integer(_)) {
                return unsupported("runtime scalar field observation requires an integer field");
            }
            if matches!(
                path.first(),
                Some(checked_trees::CheckedStructuralPredicatePathSegment::Case(
                    _
                ))
            ) {
                return Ok(
                    match crate::expression_preparation::bindings::structural_fields::resolve_case_payload(
                        structural_fields,
                        *parameter_position,
                        path,
                        scalar_type,
                    )? {
                        CasePayloadRead::Observation { expression } => expression,
                        CasePayloadRead::Established { position } => {
                            LoweredDirectExpression::Parameter {
                                position,
                                scalar_type,
                            }
                        }
                    },
                );
            }
            // A path ending at the index selects an inline primitive
            // element; one ending at a field is a record-field observation
            // whose carrier may itself cross literal fixed-array indices.
            if matches!(
                path.last(),
                Some(checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_))
            ) {
                let (source, path) =
                    crate::expression_preparation::bindings::structural_fields::resolve_primitive(
                        structural_fields,
                        *parameter_position,
                        path,
                        scalar_type,
                    )?;
                return Ok(LoweredDirectExpression::PrimitiveRead {
                    source,
                    path,
                    scalar_type,
                });
            }
            crate::expression_preparation::bindings::structural_fields::resolve(
                structural_fields,
                *parameter_position,
                path,
                scalar_type,
            )
        }
        CheckedScalarExpression::IntegerLiteral { literal } => {
            let scalar_type = integer_landing_scalar_type(literal)?;
            Ok(LoweredDirectExpression::IntegerLiteral {
                value: integer_value(literal, scalar_type)?,
                scalar_type,
            })
        }
        CheckedScalarExpression::IeeeFloatLiteral { value } => {
            Ok(LoweredDirectExpression::IeeeFloatLiteral { value: *value })
        }
        CheckedScalarExpression::ImplementationSelection {
            implementation,
            interface_identity,
        } => Ok(LoweredDirectExpression::ImplementationSelection {
            implementation: *implementation,
            interface_identity: interface_identity.clone(),
            scalar_type: crate::emission::scalar_types::selector_scalar_type(),
        }),
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
            formation_decided,
        } => {
            let scalar_type = terminal_scalar_type(*primitive_type)?;
            let left = lower_checked_scalar_expression_with_parameters(
                left,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?;
            let right = lower_checked_scalar_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?;
            Ok(LoweredDirectExpression::IntegerBinary {
                kind: match kind {
                    CheckedIntegerBinaryKind::ExactAdd => LoweredIntegerBinaryKind::ExactAdd,
                    CheckedIntegerBinaryKind::ExactSubtract => {
                        LoweredIntegerBinaryKind::ExactSubtract
                    }
                    CheckedIntegerBinaryKind::ExactMultiply => {
                        LoweredIntegerBinaryKind::ExactMultiply
                    }
                    CheckedIntegerBinaryKind::ExactDivide => LoweredIntegerBinaryKind::ExactDivide,
                    CheckedIntegerBinaryKind::ExactRemainder => {
                        LoweredIntegerBinaryKind::ExactRemainder
                    }
                    CheckedIntegerBinaryKind::WrappingDivide => {
                        LoweredIntegerBinaryKind::WrappingDivide
                    }
                    CheckedIntegerBinaryKind::WrappingRemainder => {
                        LoweredIntegerBinaryKind::WrappingRemainder
                    }
                    CheckedIntegerBinaryKind::SaturatingDivide => {
                        LoweredIntegerBinaryKind::SaturatingDivide
                    }
                    CheckedIntegerBinaryKind::SaturatingRemainder => {
                        LoweredIntegerBinaryKind::SaturatingRemainder
                    }
                    CheckedIntegerBinaryKind::WrappingAdd => LoweredIntegerBinaryKind::WrappingAdd,
                    CheckedIntegerBinaryKind::SaturatingAdd => {
                        LoweredIntegerBinaryKind::SaturatingAdd
                    }
                    CheckedIntegerBinaryKind::WrappingSubtract => {
                        LoweredIntegerBinaryKind::WrappingSubtract
                    }
                    CheckedIntegerBinaryKind::SaturatingSubtract => {
                        LoweredIntegerBinaryKind::SaturatingSubtract
                    }
                    CheckedIntegerBinaryKind::WrappingMultiply => {
                        LoweredIntegerBinaryKind::WrappingMultiply
                    }
                    CheckedIntegerBinaryKind::SaturatingMultiply => {
                        LoweredIntegerBinaryKind::SaturatingMultiply
                    }
                    CheckedIntegerBinaryKind::BitwiseAnd => LoweredIntegerBinaryKind::BitwiseAnd,
                    CheckedIntegerBinaryKind::BitwiseOr => LoweredIntegerBinaryKind::BitwiseOr,
                    CheckedIntegerBinaryKind::BitwiseXor => LoweredIntegerBinaryKind::BitwiseXor,
                    CheckedIntegerBinaryKind::WrappingShiftLeft => {
                        LoweredIntegerBinaryKind::WrappingShiftLeft
                    }
                    CheckedIntegerBinaryKind::WrappingShiftRight => {
                        LoweredIntegerBinaryKind::WrappingShiftRight
                    }
                    CheckedIntegerBinaryKind::ExactShiftLeft => {
                        LoweredIntegerBinaryKind::ExactShiftLeft
                    }
                    CheckedIntegerBinaryKind::ExactShiftRight => {
                        LoweredIntegerBinaryKind::ExactShiftRight
                    }
                    CheckedIntegerBinaryKind::SaturatingShiftRight => {
                        LoweredIntegerBinaryKind::ExactShiftRight
                    }
                    CheckedIntegerBinaryKind::SaturatingShiftLeft => {
                        return super::saturating_shift::left(scalar_type, left, right);
                    }
                    // Each Trapping primitive keeps its own Terminal operation and
                    // operation-level crash site; it is never weakened into the
                    // Exact, Wrapping, or Saturating sibling.
                    CheckedIntegerBinaryKind::TrappingShiftLeft => {
                        LoweredIntegerBinaryKind::TrappingShiftLeft
                    }
                    CheckedIntegerBinaryKind::TrappingShiftRight => {
                        LoweredIntegerBinaryKind::TrappingShiftRight
                    }
                    CheckedIntegerBinaryKind::TrappingAdd => LoweredIntegerBinaryKind::TrappingAdd,
                    CheckedIntegerBinaryKind::TrappingSubtract => {
                        LoweredIntegerBinaryKind::TrappingSubtract
                    }
                    CheckedIntegerBinaryKind::TrappingMultiply => {
                        LoweredIntegerBinaryKind::TrappingMultiply
                    }
                    CheckedIntegerBinaryKind::TrappingDivide => {
                        LoweredIntegerBinaryKind::TrappingDivide
                    }
                    CheckedIntegerBinaryKind::TrappingRemainder => {
                        LoweredIntegerBinaryKind::TrappingRemainder
                    }
                },
                scalar_type,
                left: Box::new(left),
                right: Box::new(right),
                formation_decided: *formation_decided,
            })
        }
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => Ok(LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression_with_parameters(
                operand,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
        }),
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => Ok(LoweredDirectExpression::IntegerWiden {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression_with_parameters(
                operand,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
        }),
        CheckedScalarExpression::StructuralParameterIndexedRead {
            root,
            path,
            index,
            element_path,
            primitive_type,
        } => {
            let bound_decided = selector_bound_is_decided(index, proven_indexes);
            if let checked_trees::CheckedStorageRoot::ViewLocal { symbol } = *root
                && let Some(bindings) = scalar_bindings
                && let Some(observation) = bindings.byte_record_observation(
                    proven_indexes,
                    symbol,
                    path,
                    &index.expression,
                    element_path,
                    *primitive_type,
                )?
            {
                return Ok(observation);
            }

            let lower_selector = |selector: &CheckedScalarExpression| {
                lower_checked_scalar_expression_with_parameters(
                    selector,
                    structural_parameters,
                    structural_fields,
                    structural_cases,
                    primitive_storage,
                    element_views,
                    view_locals,
                    structural_locals,
                    scalar_bindings,
                    proven_indexes,
                )
                .and_then(exact_u64_index)
            };
            let index = Box::new(lower_selector(&index.expression)?);
            // Bounded bytes read their live length, never capacity. A projected
            // byte field also retains its declaration and containing selectors.
            if element_path.is_empty()
                && *primitive_type == PrimitiveType::U8
                && matches!(
                    path.last(),
                    Some(checked_trees::CheckedStructuralPredicatePathSegment::Field(
                        _
                    ))
                )
                && let Some((source, path, field)) = byte_field_observation(
                    structural_fields,
                    structural_locals,
                    scalar_bindings,
                    *root,
                    path,
                )?
            {
                return Ok(LoweredDirectExpression::ByteSequenceFieldRead {
                    source,
                    path,
                    field,
                    index,
                    scalar_type: terminal_scalar_type(*primitive_type)?,
                    bound_decided,
                });
            }
            let whole_carrier = if path.is_empty() {
                match *root {
                    checked_trees::CheckedStorageRoot::Parameter { index: position } => {
                        let parameter = structural_parameters
                            .iter()
                            .find(|(candidate, _)| *candidate == position)
                            .map(|(_, parameter)| parameter)
                            .ok_or(LoweringError::Unsupported(
                                "indexed read lost its parameter home",
                            ))?;
                        if element_views.contains_key(&parameter.structural_type)
                            || structural_fields.declarations().iter().any(|row| {
                                row.id == parameter.structural_type
                                    && matches!(
                                        row.shape,
                                        terminal_psi::StructuralTypeShape::ByteSequence(_)
                                    )
                            })
                        {
                            Some((
                                parameter.place,
                                parameter_view_carrier(element_views, parameter.structural_type),
                            ))
                        } else {
                            None
                        }
                    }
                    checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                        Some(view_local_observation(view_locals, symbol, path)?)
                    }
                    checked_trees::CheckedStorageRoot::OwnedLocal { symbol } => {
                        if let Some(local) = view_locals.iter().find(|local| local.symbol == symbol)
                        {
                            let source = owned_local_observation(structural_locals, symbol)?;
                            if local.place != source || local.carrier != ViewCarrier::Bytes {
                                return unsupported(
                                    "owned byte read changed its established carrier",
                                );
                            }
                            Some((source, ViewCarrier::Bytes))
                        } else {
                            None
                        }
                    }
                }
            } else {
                None
            };
            if let Some((source, ViewCarrier::Bytes)) = whole_carrier {
                if !element_path.is_empty() || *primitive_type != PrimitiveType::U8 {
                    return unsupported("a byte read requires a primitive byte endpoint");
                }
                return Ok(LoweredDirectExpression::ByteSequenceRead {
                    source,
                    index,
                    scalar_type: terminal_scalar_type(*primitive_type)?,
                    bound_decided,
                });
            }
            // Geometry needs selector positions, not their expressions or proof
            // identities. Scratch markers associate each declaration walk step
            // with the read's ordered, evaluated selector, never emitted authority.
            if element_path
                .iter()
                .filter(|step| {
                    matches!(
                        step,
                        checked_trees::CheckedScalarReadProjection::RuntimeIndex(_)
                    )
                })
                .count()
                > crate::emission::operation_emission::expressions::TERMINAL_NESTED_SELECTOR_LIMIT
            {
                return unsupported("indexed read exceeds its selector namespace");
            }
            let mut selectors = Vec::new();
            let geometry = element_path
                .iter()
                .map(|step| match step {
                    checked_trees::CheckedScalarReadProjection::Static(segment) => segment.clone(),
                    checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                        let depth = selectors.len() as u32;
                        selectors.push(selector);
                        checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                            checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
                        )
                    }
                })
                .collect::<Vec<_>>();
            // Views publish descriptor homes in the view registry; ordinary
            // storage publishes homes in the field registry. Both supply the
            // same declaration-driven projection walk, not different paths.
            let (source, structural_type) = match *root {
                checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                    let local = view_locals::resolve(view_locals, symbol)?;
                    (local.place, local.structural_type)
                }
                _ => structural_fields.observation_source(*root, structural_locals)?,
            };
            let (source, path, explicit_path, scalar_type) =
                crate::expression_preparation::bindings::structural_fields::resolve_indexed_element(
                    structural_fields, source, structural_type, path, &geometry)?;
            if scalar_type != terminal_scalar_type(*primitive_type)? {
                return unsupported("indexed read changes its declared scalar carrier");
            }
            let element_path = explicit_path
                .into_iter()
                .map(|step| match step {
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
                    ) => {
                        let selector =
                            selectors
                                .get(depth as usize)
                                .ok_or(LoweringError::Unsupported(
                                    "indexed read lost its selector position",
                                ))?;
                        Ok(LoweredElementPathSegment::RuntimeIndex {
                            selector: lower_selector(&selector.expression)?,
                            extent: None,
                            bound_decided: selector_bound_is_decided(selector, proven_indexes),
                        })
                    }
                    checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                        Ok(LoweredElementPathSegment::Static(
                            terminal_psi::StructuralPathSegment::Field(identity),
                        ))
                    }
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                        Ok(LoweredElementPathSegment::Static(
                            terminal_psi::StructuralPathSegment::FixedIndex(index),
                        ))
                    }
                    checked_trees::CheckedUnitStructuralPathSegment::Referent => {
                        Ok(LoweredElementPathSegment::Static(
                            terminal_psi::StructuralPathSegment::Referent,
                        ))
                    }
                    _ => unsupported("indexed read has an unsupported projection"),
                })
                .collect::<Result<Vec<_>, _>>()?;
            match whole_carrier {
                Some((view_source, ViewCarrier::Elements { .. })) => {
                    if view_source != source || !path.is_empty() {
                        return unsupported("element read changed its live descriptor");
                    }
                    Ok(LoweredDirectExpression::ElementViewRead {
                        source,
                        index,
                        // The outer selector keeps the descriptor's live extent;
                        // descendants keep their own declaration-driven bounds.
                        path: element_path,
                        scalar_type,
                        bound_decided,
                    })
                }
                _ => Ok(LoweredDirectExpression::IndexedPrimitiveRead {
                    source,
                    path,
                    index,
                    element_path,
                    scalar_type,
                    bound_decided,
                }),
            }
        }
        CheckedScalarExpression::IntegerWrappingCast {
            primitive_type,
            operand,
            ..
        } => {
            let operand = lower_checked_scalar_expression_with_parameters(
                operand,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?;
            let target = terminal_scalar_type(*primitive_type)?;
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return unsupported("wrapping conversion requires fixed integer carriers");
            };
            wrapping_cast::wrapping_cast(operand, source_type, target)
        }
        // The source place is lowered at its declared carrier -- every
        // declared-type check of that read still applies -- and the view
        // replays the carrier relation instead of retagging the read.
        CheckedScalarExpression::RepresentationView {
            primitive_type,
            operand,
        } => {
            let operand = lower_checked_scalar_expression_with_parameters(
                operand,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?;
            crate::expression_preparation::representation_view::replay(
                operand,
                terminal_scalar_type(*primitive_type)?,
            )
        }
        CheckedScalarExpression::IntegerSaturatingCast {
            primitive_type,
            operand,
        } => {
            let operand = lower_checked_scalar_expression_with_parameters(
                operand,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?;
            let target = terminal_scalar_type(*primitive_type)?;
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return unsupported("saturating conversion requires fixed integer carriers");
            };
            saturating_cast::saturating_cast(operand, source_type, target)
        }
        CheckedScalarExpression::IntegerTrappingCast {
            primitive_type,
            operand,
        } => {
            let operand = lower_checked_scalar_expression_with_parameters(
                operand,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?;
            crate::expression_preparation::trapping_cast::trapping_cast(
                operand,
                terminal_scalar_type(*primitive_type)?,
            )
        }
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            formation_decided,
            ..
        } => Ok(LoweredDirectExpression::IntegerExactCast {
            formation_decided: *formation_decided,
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression_with_parameters(
                operand,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
        }),
        CheckedScalarExpression::Boolean(expression) => Ok(LoweredDirectExpression::Boolean {
            expression: Box::new(lower_checked_boolean_expression_with_parameters(
                expression,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
        }),
    }
}

fn lower_checked_boolean_expression_with_parameters(
    expression: &CheckedBooleanExpression,
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
    structural_fields: &crate::expression_preparation::bindings::StructuralFieldBindings,
    structural_cases: &[crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding],
    primitive_storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
    element_views: &std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
    view_locals: &[ViewLocalBinding],
    structural_locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    scalar_bindings: Option<&crate::expression_preparation::bindings::ScalarBindings>,
    proven_indexes: &[checked_trees::expression::ExpressionHandle],
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    Ok(match expression {
        CheckedBooleanExpression::Constant(value) => {
            LoweredBooleanReturnExpression::Constant { value: *value }
        }
        CheckedBooleanExpression::Parameter { position } => {
            LoweredBooleanReturnExpression::Parameter {
                position: *position,
            }
        }
        CheckedBooleanExpression::StorageRead { symbol } => {
            if let Some(bindings) = scalar_bindings
                && let Some(observation) =
                    crate::expression_preparation::bindings::primitive_reads::captured_observation(
                        bindings,
                        *symbol,
                        ScalarType::Boolean,
                    )?
            {
                return Ok(LoweredBooleanReturnExpression::ScalarObservation {
                    operand: Box::new(observation),
                });
            }
            let (source, path) = crate::expression_preparation::bindings::primitive_storage_read(
                primitive_storage,
                structural_locals,
                structural_fields.declarations(),
                *symbol,
                ScalarType::Boolean,
            )?;
            LoweredBooleanReturnExpression::PrimitiveRead {
                source: source.into(),
                path,
            }
        }
        CheckedBooleanExpression::Local { position } => LoweredBooleanReturnExpression::Local {
            position: *position,
        },
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => {
            if !structural_fields.is_empty()
                && matches!(
                    path.first(),
                    Some(checked_trees::CheckedStructuralPredicatePathSegment::Case(
                        _
                    ))
                )
            {
                return Ok(
                    match crate::expression_preparation::bindings::structural_fields::resolve_case_payload(
                        structural_fields,
                        *parameter_position,
                        path,
                        ScalarType::Boolean,
                    )? {
                        CasePayloadRead::Observation { expression } =>
                            crate::expression_preparation::bindings::structural_fields::boolean_observation(expression)?,
                        CasePayloadRead::Established { position } => {
                            LoweredBooleanReturnExpression::Parameter { position }
                        }
                    },
                );
            }
            if !structural_fields.is_empty() {
                // Index-terminal paths read an inline primitive element;
                // field-terminal paths observe a record field whose carrier
                // may itself cross literal fixed-array indices.
                if matches!(
                    path.last(),
                    Some(checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_))
                ) {
                    let (source, path) =
                        crate::expression_preparation::bindings::structural_fields::resolve_primitive(
                            structural_fields,
                            *parameter_position,
                            path,
                            ScalarType::Boolean,
                        )?;
                    return Ok(LoweredBooleanReturnExpression::PrimitiveRead { source, path });
                }
                let observation =
                    crate::expression_preparation::bindings::structural_fields::resolve(
                        structural_fields,
                        *parameter_position,
                        path,
                        ScalarType::Boolean,
                    )?;
                return crate::expression_preparation::bindings::structural_fields::boolean_observation(observation);
            }
            let path = path
                .iter()
                .map(|segment| match segment {
                    checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                        Ok(identity.clone())
                    }
                    checked_trees::CheckedStructuralPredicatePathSegment::Case(_)
                    | checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_)
                    | checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(_) => {
                        unsupported("case-payload predicates are contract-only")
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            LoweredBooleanReturnExpression::UnresolvedStructuralParameterField {
                parameter_position: *parameter_position,
                path,
            }
        }
        CheckedBooleanExpression::ByteSequencePredicate {
            predicate,
            root,
            path,
        } => {
            if !path.is_empty() {
                let (source, path, field) = byte_field_observation(
                    structural_fields,
                    structural_locals,
                    scalar_bindings,
                    *root,
                    path,
                )?
                .ok_or(LoweringError::Unsupported(
                    "byte predicate requires a live byte descriptor",
                ))?;
                return Ok(LoweredBooleanReturnExpression::ByteSequencePredicate {
                    predicate: *predicate,
                    source,
                    path,
                    field: Some(field),
                });
            }
            let source = match *root {
                checked_trees::CheckedStorageRoot::Parameter {
                    index: parameter_position,
                } => readable_sequence_parameter(parameter_position, structural_parameters)?.place,
                checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                    view_local_observation(view_locals, symbol, path)?.0
                }
                checked_trees::CheckedStorageRoot::OwnedLocal { symbol } => {
                    let source = owned_local_observation(structural_locals, symbol)?;
                    let local = crate::expression_preparation::bindings::view_locals::resolve(
                        view_locals,
                        symbol,
                    )?;
                    if local.place != source || local.carrier != ViewCarrier::Bytes {
                        return unsupported(
                            "byte predicate operand lost its established byte carrier",
                        );
                    }
                    source
                }
            };
            LoweredBooleanReturnExpression::ByteSequencePredicate {
                predicate: *predicate,
                source,
                path: Vec::new(),
                field: None,
            }
        }
        CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
            let operand = |field: &checked_trees::CheckedStructuralParameterField| {
                byte_field_observation(
                    structural_fields,
                    structural_locals,
                    scalar_bindings,
                    checked_trees::CheckedStorageRoot::Parameter {
                        index: field.parameter_position,
                    },
                    &field.path,
                )?
                .map(|(source, path, field)| {
                    crate::emission::operation_emission::boolean::LoweredByteSequenceOperand {
                        source,
                        path,
                        field: Some(field),
                    }
                })
                .ok_or(LoweringError::Unsupported(
                    "byte sequence equality requires a live byte descriptor",
                ))
            };
            LoweredBooleanReturnExpression::ByteSequenceEqual {
                left: operand(left)?,
                right: operand(right)?,
            }
        }
        CheckedBooleanExpression::Not(operand) => LoweredBooleanReturnExpression::Not {
            operand: Box::new(lower_checked_boolean_expression_with_parameters(
                operand,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
        },
        CheckedBooleanExpression::Equal { left, right } => LoweredBooleanReturnExpression::Equal {
            left: Box::new(lower_checked_boolean_expression_with_parameters(
                left,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
            right: Box::new(lower_checked_boolean_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
        },
        CheckedBooleanExpression::IntegerMathComparison { .. } => {
            return unsupported("mathematical contract predicate has no executable Boolean value");
        }
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            LoweredBooleanReturnExpression::IntegerComparison {
                kind: match kind {
                    CheckedIntegerComparisonKind::Equal => LoweredIntegerComparisonKind::Equal,
                    CheckedIntegerComparisonKind::LessThan => {
                        LoweredIntegerComparisonKind::LessThan
                    }
                    CheckedIntegerComparisonKind::LessOrEqual => {
                        LoweredIntegerComparisonKind::LessOrEqual
                    }
                },
                left: Box::new(lower_checked_scalar_expression_with_parameters(
                    left,
                    structural_parameters,
                    structural_fields,
                    structural_cases,
                    primitive_storage,
                    element_views,
                    view_locals,
                    structural_locals,
                    scalar_bindings,
                    proven_indexes,
                )?),
                right: Box::new(lower_checked_scalar_expression_with_parameters(
                    right,
                    structural_parameters,
                    structural_fields,
                    structural_cases,
                    primitive_storage,
                    element_views,
                    view_locals,
                    structural_locals,
                    scalar_bindings,
                    proven_indexes,
                )?),
            }
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            let (source, path, case) =
                crate::expression_preparation::bindings::structural_cases::resolve(
                    structural_cases,
                    subject,
                    case,
                )?;
            LoweredBooleanReturnExpression::StructuralCaseMembership { source, path, case }
        }
        CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
        | CheckedBooleanExpression::ErasedParameter { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. } => {
            return unsupported("structural equality is contract-only terminal vocabulary");
        }
        CheckedBooleanExpression::And { left, right } => LoweredBooleanReturnExpression::And {
            left: Box::new(lower_checked_boolean_expression_with_parameters(
                left,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
            right: Box::new(lower_checked_boolean_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
        },
        CheckedBooleanExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(lower_checked_boolean_expression_with_parameters(
                left,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
            right: Box::new(lower_checked_boolean_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
                proven_indexes,
            )?),
        },
    })
}

fn byte_field_observation(
    fields: &crate::expression_preparation::bindings::StructuralFieldBindings,
    structural_locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    bindings: Option<&crate::expression_preparation::bindings::ScalarBindings>,
    root: checked_trees::CheckedStorageRoot,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<
    Option<(
        PlaceId,
        Vec<LoweredElementPathSegment>,
        semantic_vocabulary::StructuralFieldId,
    )>,
    LoweringError,
> {
    use crate::emission::runtime_elements::ProjectionStep;
    let (source, structural_type) = fields.observation_source(root, structural_locals)?;
    let Some((source, steps, field)) =
        crate::expression_preparation::bindings::structural_fields::resolve_byte_projection(
            source,
            structural_type,
            fields.declarations(),
            path,
        )?
    else {
        return Ok(None);
    };
    let view_prefix = fields
        .declarations()
        .iter()
        .find(|row| row.id == structural_type)
        .and_then(|row| match row.shape {
            terminal_psi::StructuralTypeShape::ElementView { .. } => Some(0),
            terminal_psi::StructuralTypeShape::Reference { referent, .. }
                if fields.declarations().iter().any(|row| {
                    row.id == referent
                        && matches!(
                            row.shape,
                            terminal_psi::StructuralTypeShape::ElementView { .. }
                        )
                }) =>
            {
                Some(1)
            }
            _ => None,
        });
    let path = steps
        .into_iter()
        .enumerate()
        .map(|(ordinal, step)| {
            let selector = match step {
                ProjectionStep::Static(segment) => {
                    return Ok(LoweredElementPathSegment::Static(segment));
                }
                ProjectionStep::RuntimeIndexLiteral { index } => {
                    LoweredDirectExpression::IntegerLiteral {
                        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(index)),
                        scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                    }
                }
                step => {
                    let selector = match step {
                        ProjectionStep::RuntimeIndexParameter { position } => {
                            checked_trees::CheckedRuntimeIndex::Parameter { position }
                        }
                        ProjectionStep::RuntimeIndexLocal { position } => {
                            checked_trees::CheckedRuntimeIndex::Local { position }
                        }
                        ProjectionStep::RuntimeIndexField(field) => {
                            checked_trees::CheckedRuntimeIndex::StructuralField(field)
                        }
                        _ => {
                            return unsupported(
                                "field observation cannot borrow an assignment selector",
                            );
                        }
                    };
                    exact_u64_index(element_path_runtime_selector(fields, bindings, &selector)?)?
                }
            };
            let extent = (view_prefix == Some(ordinal)).then(|| {
                LoweredDirectExpression::ElementViewLength {
                    source,
                    scalar_type: selector.scalar_type(),
                }
            });
            Ok(LoweredElementPathSegment::RuntimeIndex {
                selector,
                extent,
                bound_decided: false,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if path
        .iter()
        .filter(|segment| matches!(segment, LoweredElementPathSegment::RuntimeIndex { .. }))
        .count()
        > crate::emission::operation_emission::expressions::TERMINAL_NESTED_SELECTOR_LIMIT
    {
        return unsupported("field observation exceeds the runtime selector namespace");
    }
    Ok(Some((source, path, field)))
}

/// Terminal element reads take a `u64` position. Any integer carrier may index
/// once its bounds are proven: a narrower unsigned index widens totally, and a
/// signed index lands through an exact cast whose Terminal obligation proves it
/// nonnegative, so it never silently becomes unsigned.
/// The direct expression one element-path runtime selector evaluates to:
/// `cells[i].data[j]`'s `j` rides the same structural field bindings a
/// carrier path's `CheckedRuntimeIndex` spells. Parameter selectors resolve
/// through the typed authored namespace before emission.
fn element_path_runtime_selector(
    structural_fields: &crate::expression_preparation::bindings::StructuralFieldBindings,
    scalar_bindings: Option<&crate::expression_preparation::bindings::ScalarBindings>,
    index: &checked_trees::CheckedRuntimeIndex,
) -> Result<LoweredDirectExpression, LoweringError> {
    match index {
        checked_trees::CheckedRuntimeIndex::Parameter { position } => scalar_bindings
            .ok_or(LoweringError::Unsupported(
                "element selector has no typed scalar namespace",
            ))?
            .runtime_index_parameter(*position),
        checked_trees::CheckedRuntimeIndex::Local { position } => scalar_bindings
            .ok_or(LoweringError::Unsupported(
                "element selector has no typed scalar namespace",
            ))?
            .runtime_index_local(*position),
        checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
            let observation =
                crate::expression_preparation::bindings::structural_fields::resolve_typed(
                    structural_fields,
                    field.parameter_position,
                    &field.path,
                )?;
            if !matches!(observation.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
            {
                return unsupported("element path runtime selector is not an integer read");
            }
            Ok(observation)
        }
        _ => unsupported("element path runtime selector is not a parameter or field read"),
    }
}

pub(crate) fn exact_u64_index(
    index: LoweredDirectExpression,
) -> Result<LoweredDirectExpression, LoweringError> {
    let u64_type = terminal_scalar_type(PrimitiveType::U64)?;
    match index.scalar_type() {
        scalar_type if scalar_type == u64_type => Ok(index),
        ScalarType::Integer(integer)
            if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned =>
        {
            Ok(LoweredDirectExpression::IntegerWiden {
                scalar_type: u64_type,
                operand: Box::new(index),
            })
        }
        ScalarType::Integer(_) => Ok(LoweredDirectExpression::IntegerExactCast {
            scalar_type: u64_type,
            operand: Box::new(index),
            formation_decided: false,
        }),
        _ => unsupported("indexed reads require an integer index"),
    }
}
