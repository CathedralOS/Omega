//! Lowering checked scalar and boolean expressions.

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
use crate::expression_preparation::source_custody;
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
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(state, statement_ordinal, role)
        .ok_or(LoweringError::Unsupported(
            "scalar expression has no unique source-bound checked value plan",
        ))?;
    let expression = lower_checked_scalar_expression(expression)?;
    source_custody::validate_pure(checked, binding, expression.scalar_type())?;
    Ok(expression)
}

pub(crate) fn lower_checked_scalar_expression(
    expression: &CheckedScalarExpression,
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
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(state, statement_ordinal, role)
        .ok_or(LoweringError::Unsupported(
            "scalar expression has no unique source-bound checked value plan",
        ))?;
    let expression = lower_checked_scalar_expression_with_parameters(
        expression,
        structural_parameters,
        &Default::default(),
        &[],
        &[],
        &std::collections::BTreeMap::new(),
        &[],
        &[],
        None,
    )?;
    source_custody::validate_pure(checked, binding, expression.scalar_type())?;
    Ok(expression)
}

/// The whole view parameter at `position`, for a length or element read. An
/// exclusive `&mut [T]` view grants observation as well as mutation, so its
/// lengths and element reads observe the parameter place exactly as a shared
/// view's do; a write-only or owned parameter grants no observation here.
fn view_observation_parameter(
    position: u32,
    parameters: &[(u32, StructuralParameterDeclaration)],
) -> Result<&StructuralParameterDeclaration, LoweringError> {
    let parameter = parameters
        .iter()
        .find_map(|(source_position, parameter)| {
            (*source_position == position).then_some(parameter)
        })
        .ok_or(LoweringError::Unsupported(
            "view observation has no exact structural parameter",
        ))?;
    if !matches!(
        parameter.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
    {
        return unsupported(
            "view observation requires an exact whole view with access for this observation",
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
/// observations (element reads) rooted at it. The local keeps whole owned
/// custody: an established view re-loan or a projected source means the read
/// is no longer the plain owned-place read this root names.
fn owned_local_observation(
    structural_locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    symbol: symbols::SymbolHandle,
) -> Result<PlaceId, LoweringError> {
    let mut matches = structural_locals
        .iter()
        .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol));
    let Some(crate::expression_preparation::bindings::StructuralLocalBinding {
        argument: source,
        ..
    }) = matches.next()
    else {
        return unsupported("owned local observation lost its established place");
    };
    if !symbol.is_valid()
        || matches.next().is_some()
        || source.access != StructuralAccess::Owned
        || !source.path.is_empty()
    {
        return unsupported("owned local observation changed its local custody");
    }
    Ok(source.place)
}

fn lower_indexed_field_observation(
    fields: &crate::expression_preparation::bindings::StructuralFieldBindings,
    root: checked_trees::CheckedStorageRoot,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    element_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    index: LoweredDirectExpression,
    expected: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let (source, path, leaf, scalar_type) =
        crate::expression_preparation::bindings::structural_fields::resolve_indexed_element_field(
            fields,
            root,
            path,
            element_path,
        )?;
    if scalar_type != expected {
        return unsupported("indexed element field read drifted from its field type");
    }
    use crate::emission::primitive_store::IndexedElementLeaf;
    Ok(match leaf {
        IndexedElementLeaf::Flat(element_path) => LoweredDirectExpression::IndexedPrimitiveRead {
            source,
            path,
            index: Box::new(index),
            inner_indexes: Vec::new(),
            element_path: element_path
                .into_iter()
                .map(LoweredElementPathSegment::Static)
                .collect(),
            scalar_type,
        },
        IndexedElementLeaf::Hoist {
            element,
            carrier,
            field,
        } => LoweredDirectExpression::IndexedElementFieldRead {
            source,
            path,
            index: Box::new(index),
            element,
            carrier,
            field,
            scalar_type,
        },
        IndexedElementLeaf::RuntimeTail { .. } => {
            return unsupported("static element path minted a runtime tail");
        }
    })
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
) -> Result<LoweredDirectExpression, LoweringError> {
    match expression {
        CheckedScalarExpression::StructuralParameterByteLength { root, path } => {
            let count_type = terminal_scalar_type(PrimitiveType::U64)?;
            if !path.is_empty() {
                let (source, path, field) =
                    byte_field_observation(structural_fields, scalar_bindings, *root, path)?
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
                        view_observation_parameter(parameter_position, structural_parameters)?;
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
            let source = crate::expression_preparation::bindings::primitive_storage_place(
                primitive_storage,
                *symbol,
                scalar_type,
            )?;
            Ok(LoweredDirectExpression::PrimitiveRead {
                source: source.into(),
                path: Vec::new(),
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
                        CasePayloadRead::Observation { source, path, field } => {
                            LoweredDirectExpression::StructuralField {
                                source,
                                path,
                                field,
                                scalar_type,
                            }
                        }
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
            let (source, path, field) =
                crate::expression_preparation::bindings::structural_fields::resolve(
                    structural_fields,
                    *parameter_position,
                    path,
                    scalar_type,
                )?;
            Ok(LoweredDirectExpression::StructuralField {
                source: source.into(),
                path,
                field,
                scalar_type,
            })
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
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
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
            )?),
        }),
        CheckedScalarExpression::StructuralParameterIndexedRead {
            root,
            path,
            index,
            inner_indexes,
            element_path,
            primitive_type,
        } => {
            if let checked_trees::CheckedStorageRoot::ViewLocal { symbol } = *root
                && let Some(bindings) = scalar_bindings
                && let Some(observation) = bindings.byte_record_observation(
                    symbol,
                    path,
                    index,
                    inner_indexes,
                    element_path,
                    *primitive_type,
                )?
            {
                return Ok(observation);
            }
            if matches!(
                path.last(),
                Some(checked_trees::CheckedStructuralPredicatePathSegment::Field(
                    _
                ))
            ) && inner_indexes.is_empty()
                && element_path.is_empty()
                && *primitive_type == PrimitiveType::U8
                && let Some((source, path, field)) =
                    byte_field_observation(structural_fields, scalar_bindings, *root, path)?
            {
                let index = lower_checked_scalar_expression_with_parameters(
                    index,
                    structural_parameters,
                    structural_fields,
                    structural_cases,
                    primitive_storage,
                    element_views,
                    view_locals,
                    structural_locals,
                    scalar_bindings,
                )?;
                return Ok(LoweredDirectExpression::ByteSequenceFieldRead {
                    source,
                    path,
                    field,
                    index: Box::new(exact_u64_index(index)?),
                    scalar_type: terminal_scalar_type(PrimitiveType::U8)?,
                });
            }
            // Nested selectors read a structural parameter or its static
            // projection; every selector keeps its own
            // `RuntimeIndex` segment and obligation.
            if !inner_indexes.is_empty() {
                let checked_trees::CheckedStorageRoot::Parameter {
                    index: parameter_position,
                } = *root
                else {
                    return unsupported("nested element reads require a parameter root");
                };
                if !element_path.is_empty() {
                    return unsupported("nested element reads require a scalar leaf");
                }
                if inner_indexes.len()
                    > crate::emission::operation_emission::expressions::TERMINAL_NESTED_SELECTOR_LIMIT
                {
                    return unsupported("nested element read exceeds its selector namespace");
                }
                let mut lower_index = |selector: &CheckedScalarExpression| {
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
                    )
                    .and_then(exact_u64_index)
                };
                let index = lower_index(index)?;
                let inner_indexes = inner_indexes
                    .iter()
                    .map(&mut lower_index)
                    .collect::<Result<Vec<_>, _>>()?;
                let (source, path, scalar_type) =
                    crate::expression_preparation::bindings::structural_fields::resolve_nested_indexed_array(
                        structural_fields,
                        parameter_position,
                        path,
                        inner_indexes.len(),
                    )?;
                if scalar_type != terminal_scalar_type(*primitive_type)? {
                    return unsupported("nested element read drifted from its leaf type");
                }
                return Ok(LoweredDirectExpression::IndexedPrimitiveRead {
                    source,
                    path,
                    index: Box::new(index),
                    inner_indexes,
                    element_path: Vec::new(),
                    scalar_type,
                });
            }
            if let checked_trees::CheckedStorageRoot::OwnedLocal { symbol } = *root {
                let source = owned_local_observation(structural_locals, symbol)?;
                let index = lower_checked_scalar_expression_with_parameters(
                    index,
                    structural_parameters,
                    structural_fields,
                    structural_cases,
                    primitive_storage,
                    element_views,
                    view_locals,
                    structural_locals,
                    scalar_bindings,
                )?;
                if matches!(
                    element_path.last(),
                    Some(checked_trees::CheckedStructuralPredicatePathSegment::Field(
                        _
                    ))
                ) {
                    if structural_fields.observation_source(*root)?.0 != source {
                        return unsupported("indexed local field changed its established source");
                    }
                    return lower_indexed_field_observation(
                        structural_fields,
                        *root,
                        path,
                        element_path,
                        exact_u64_index(index)?,
                        terminal_scalar_type(*primitive_type)?,
                    );
                }
                // A raw array uses its declared extent. A bounded byte owner
                // instead observes its live length, which may be less than
                // capacity; the established carrier below selects the rule.
                let path = path
                    .iter()
                    .map(|segment| match segment {
                        checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                            Ok(terminal_psi::StructuralPathSegment::Field(identity.clone()))
                        }
                        checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                            Ok(terminal_psi::StructuralPathSegment::FixedIndex(*index))
                        }
                        _ => {
                            unsupported("an owned local collection path takes static members only")
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                // A path ending at an inline primitive array element keeps
                // its static tail. Declared fields already used the common
                // field resolver above, including their restriction evidence.
                let element_path = element_path
                    .iter()
                    .map(|segment| match segment {
                        checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                            Ok(LoweredElementPathSegment::Static(
                                terminal_psi::StructuralPathSegment::Field(identity.clone()),
                            ))
                        }
                        checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                            Ok(LoweredElementPathSegment::Static(
                                terminal_psi::StructuralPathSegment::FixedIndex(*index),
                            ))
                        }
                        _ => unsupported("an owned local element path takes static members only"),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if view_locals.iter().any(|local| local.symbol == symbol) {
                    let local = crate::expression_preparation::bindings::view_locals::resolve(
                        view_locals,
                        symbol,
                    )?;
                    if local.place != source
                        || local.carrier != ViewCarrier::Bytes
                        || *primitive_type != PrimitiveType::U8
                        || !path.is_empty()
                        || !element_path.is_empty()
                    {
                        return unsupported("owned byte read lost its established carrier");
                    }
                    // A bounded owner has a capacity and a live length. Read
                    // through the ordinary byte operation so the obligation
                    // uses that current length, never the fixed-array capacity.
                    return Ok(LoweredDirectExpression::ByteSequenceRead {
                        source,
                        index: Box::new(exact_u64_index(index)?),
                        scalar_type: terminal_scalar_type(PrimitiveType::U8)?,
                    });
                }
                return Ok(LoweredDirectExpression::IndexedPrimitiveRead {
                    source,
                    path,
                    index: Box::new(exact_u64_index(index)?),
                    inner_indexes: Vec::new(),
                    element_path,
                    scalar_type: terminal_scalar_type(*primitive_type)?,
                });
            }
            let (source, carrier) = match *root {
                checked_trees::CheckedStorageRoot::Parameter {
                    index: parameter_position,
                } => {
                    // A whole owned array has an empty carrier path, but
                    // still gets its extent and element type from the same
                    // declaration as an array field. It is not a borrowed
                    // view and must not acquire view-observation authority.
                    let owned_parameter =
                        structural_parameters.iter().any(|(position, parameter)| {
                            *position == parameter_position
                                && parameter.access == StructuralAccess::Owned
                        });
                    // A whole element view carries its runtime extent even
                    // when the selected element has fields. Field selection
                    // does not turn that descriptor into a fixed array.
                    let whole_element_view = path.is_empty()
                        && structural_parameters.iter().any(|(position, parameter)| {
                            *position == parameter_position
                                && element_views.contains_key(&parameter.structural_type)
                        });
                    // A borrowed whole fixed array of non-byte elements
                    // (`numbers: &[i32; 6]`) reads through its declared
                    // extent exactly as an owned array parameter does; only
                    // a byte array keeps its bounded byte-view carrier.
                    let borrowed_fixed_array = path.is_empty()
                        && !whole_element_view
                        && !owned_parameter
                        && matches!(
                            crate::expression_preparation::bindings::structural_fields::resolve_indexed_array(
                                structural_fields,
                                parameter_position,
                                path,
                            ),
                            Ok(Some((_, _, scalar_type)))
                                if scalar_type != terminal_scalar_type(PrimitiveType::U8)?
                        );
                    if !path.is_empty()
                        || (!whole_element_view
                            && (!element_path.is_empty()
                                || owned_parameter
                                || borrowed_fixed_array))
                    {
                        let index = lower_checked_scalar_expression_with_parameters(
                            index,
                            structural_parameters,
                            structural_fields,
                            structural_cases,
                            primitive_storage,
                            element_views,
                            view_locals,
                            structural_locals,
                            scalar_bindings,
                        )?;
                        let index = exact_u64_index(index)?;
                        // A scalar field of a record element (`cells[i].hp`)
                        // follows the selector with the element path; the
                        // array's declared extent still bounds the selector.
                        if element_path.iter().any(|segment| {
                            matches!(
                                segment,
                                checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                                    _
                                )
                            )
                        }) {
                            // `cells[i].data[j]`: the element path carries
                            // its own runtime selector beside the member
                            // steps. Each `RuntimeIndex` payload lowers to
                            // a direct expression through the same field
                            // bindings the carrier paths use.
                            let (source, path, leaf, scalar_type) =
                                crate::expression_preparation::bindings::structural_fields::resolve_indexed_element_runtime_path(
                                    structural_fields,
                                    parameter_position,
                                    path,
                                    element_path,
                                )?;
                            if scalar_type != terminal_scalar_type(*primitive_type)? {
                                return unsupported(
                                    "indexed element read drifted from its element type",
                                );
                            }
                            use crate::emission::primitive_store::IndexedElementLeaf;
                            let IndexedElementLeaf::RuntimeTail { element_path, .. } = leaf else {
                                return unsupported("indexed element runtime path did not lower");
                            };
                            let mut lowered_path = Vec::with_capacity(element_path.len());
                            for segment in element_path {
                                let segment = match segment {
                                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                                        index,
                                    ) => LoweredElementPathSegment::RuntimeIndex {
                                        selector: exact_u64_index(element_path_runtime_selector(
                                            structural_fields,
                                            scalar_bindings,
                                            &index,
                                        )?)?,
                                        extent: None,
                                    },
                                    checked_trees::CheckedUnitStructuralPathSegment::Field(
                                        identity,
                                    ) => LoweredElementPathSegment::Static(
                                        terminal_psi::StructuralPathSegment::Field(identity),
                                    ),
                                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                                        index,
                                    ) => LoweredElementPathSegment::Static(
                                        terminal_psi::StructuralPathSegment::FixedIndex(index),
                                    ),
                                    _ => {
                                        return unsupported(
                                            "indexed element runtime path has an unsupported segment",
                                        );
                                    }
                                };
                                lowered_path.push(segment);
                            }
                            return Ok(LoweredDirectExpression::IndexedPrimitiveRead {
                                source,
                                path,
                                index: Box::new(index),
                                inner_indexes: Vec::new(),
                                element_path: lowered_path,
                                scalar_type,
                            });
                        }
                        if !element_path.is_empty() {
                            return lower_indexed_field_observation(
                                structural_fields,
                                *root,
                                path,
                                element_path,
                                index,
                                terminal_scalar_type(*primitive_type)?,
                            );
                        }
                        // A path ending at a fixed array declares its extent
                        // on the type, so the element read needs no length
                        // observation.
                        if let Some((source, path, element_scalar)) =
                            crate::expression_preparation::bindings::structural_fields::resolve_indexed_array(
                                structural_fields,
                                parameter_position,
                                path,
                            )?
                        {
                            if element_scalar != terminal_scalar_type(*primitive_type)? {
                                return unsupported(
                                    "indexed field read drifted from its element type",
                                );
                            }
                            return Ok(LoweredDirectExpression::IndexedPrimitiveRead {
                                source,
                                path,
                                index: Box::new(index),
                                inner_indexes: Vec::new(),
                                element_path: Vec::new(),
                                scalar_type: element_scalar,
                            });
                        }
                        return unsupported("indexed field read has no scalar collection carrier");
                    }
                    let parameter =
                        view_observation_parameter(parameter_position, structural_parameters)?;
                    (
                        parameter.place,
                        parameter_view_carrier(element_views, parameter.structural_type),
                    )
                }
                checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                    view_local_observation(view_locals, symbol, path)?
                }
                checked_trees::CheckedStorageRoot::OwnedLocal { .. } => {
                    return unsupported("owned local element read left the view-carrier match");
                }
            };
            let element_scalar = match carrier {
                ViewCarrier::Bytes => None,
                ViewCarrier::Elements {
                    element: Some(scalar_type),
                } => Some(scalar_type),
                // A record element yields a scalar only at a field leaf: the
                // read carries the element path, and the verifier resolves
                // that leaf against the view's element type.
                ViewCarrier::Elements { element: None } => {
                    if element_path.is_empty() {
                        return unsupported(
                            "element-view reads of record elements yield no scalar",
                        );
                    }
                    let path = element_path
                        .iter()
                        .map(|segment| match segment {
                            checked_trees::CheckedStructuralPredicatePathSegment::Field(
                                identity,
                            ) => Ok(terminal_psi::StructuralPathSegment::Field(identity.clone())),
                            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(
                                index,
                            ) => Ok(terminal_psi::StructuralPathSegment::FixedIndex(*index)),
                            checked_trees::CheckedStructuralPredicatePathSegment::Case(_) => {
                                unsupported("an element field path selects no case")
                            }
                            // A runtime selector inside a view local's
                            // element path stays out of this read's scope:
                            // element-view paths carry no runtime steps.
                            checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                                _,
                            ) => unsupported("an element field path selects no runtime element"),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let index = lower_checked_scalar_expression_with_parameters(
                        index,
                        structural_parameters,
                        structural_fields,
                        structural_cases,
                        primitive_storage,
                        element_views,
                        view_locals,
                        structural_locals,
                        scalar_bindings,
                    )?;
                    return Ok(LoweredDirectExpression::ElementViewRead {
                        source,
                        index: Box::new(exact_u64_index(index)?),
                        path,
                        scalar_type: terminal_scalar_type(*primitive_type)?,
                    });
                }
            };
            if !element_path.is_empty() {
                return unsupported("a scalar element has no field path");
            }
            if element_scalar.is_none() && *primitive_type != PrimitiveType::U8 {
                return unsupported("indexed reads require a whole byte-view parameter");
            }
            let index = lower_checked_scalar_expression_with_parameters(
                index,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
                structural_locals,
                scalar_bindings,
            )?;
            let index = exact_u64_index(index)?;
            match element_scalar {
                Some(scalar_type) => {
                    if scalar_type != terminal_scalar_type(*primitive_type)? {
                        return unsupported(
                            "element-view indexed read drifted from its element type",
                        );
                    }
                    Ok(LoweredDirectExpression::ElementViewRead {
                        source,
                        index: Box::new(index),
                        path: Vec::new(),
                        scalar_type,
                    })
                }
                None => Ok(LoweredDirectExpression::ByteSequenceRead {
                    source,
                    index: Box::new(index),
                    scalar_type: terminal_scalar_type(PrimitiveType::U8)?,
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
            )?;
            crate::expression_preparation::trapping_cast::trapping_cast(
                operand,
                terminal_scalar_type(*primitive_type)?,
            )
        }
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            ..
        } => Ok(LoweredDirectExpression::IntegerExactCast {
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
            )?),
        }),
    }
}

#[cfg(test)]
pub(crate) fn lower_checked_boolean_expression(
    expression: &CheckedBooleanExpression,
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    lower_checked_boolean_expression_with_parameters(
        expression,
        &[],
        &Default::default(),
        &[],
        &[],
        &std::collections::BTreeMap::new(),
        &[],
        &[],
        None,
    )
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
            let source = crate::expression_preparation::bindings::primitive_storage_place(
                primitive_storage,
                *symbol,
                ScalarType::Boolean,
            )?;
            LoweredBooleanReturnExpression::PrimitiveRead {
                source: source.into(),
                path: Vec::new(),
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
                        CasePayloadRead::Observation { source, path, field } => {
                            LoweredBooleanReturnExpression::StructuralField {
                                source,
                                path,
                                field,
                            }
                        }
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
                let (source, path, field) =
                    crate::expression_preparation::bindings::structural_fields::resolve(
                        structural_fields,
                        *parameter_position,
                        path,
                        ScalarType::Boolean,
                    )?;
                return Ok(LoweredBooleanReturnExpression::StructuralField {
                    source: source.into(),
                    path,
                    field,
                });
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
                let (source, path, field) =
                    byte_field_observation(structural_fields, scalar_bindings, *root, path)?
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
                } => view_observation_parameter(parameter_position, structural_parameters)?.place,
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
            )?),
        },
    })
}

fn byte_field_observation(
    fields: &crate::expression_preparation::bindings::StructuralFieldBindings,
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
    let (source, structural_type) = fields.observation_source(root)?;
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
            Ok(LoweredElementPathSegment::RuntimeIndex { selector, extent })
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
            let (source, path, field, scalar_type) =
                crate::expression_preparation::bindings::structural_fields::resolve_typed(
                    structural_fields,
                    field.parameter_position,
                    &field.path,
                )?;
            if !matches!(scalar_type, ScalarType::Integer(integer) if !integer.is_address()) {
                return unsupported("element path runtime selector is not an integer read");
            }
            Ok(LoweredDirectExpression::StructuralField {
                source: source.into(),
                path,
                field,
                scalar_type,
            })
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
        }),
        _ => unsupported("indexed reads require an integer index"),
    }
}
