//! Lowering checked scalar and boolean expressions.

use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::{
    LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
};
use crate::emission::scalar_types::{
    integer_landing_scalar_type, integer_value, terminal_scalar_type,
};
use crate::expression_preparation::bindings::structural_fields::CasePayloadRead;
use crate::expression_preparation::bindings::view_locals::{self, ViewCarrier, ViewLocalBinding};
use crate::expression_preparation::source_custody;
use crate::expression_preparation::wrapping_cast;
use crate::expression_preparation::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees, IntegerSign, IntegerValue,
    LoweringError, PlaceId, PrimitiveType, ScalarType, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeId, unsupported,
};
use semantic_vocabulary::CanonicalStructuralPathSegment;

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
        &[],
        &[],
        &[],
        &std::collections::BTreeMap::new(),
        &[],
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
        &[],
        &[],
        &[],
        &std::collections::BTreeMap::new(),
        &[],
    )?;
    source_custody::validate_pure(checked, binding, expression.scalar_type())?;
    Ok(expression)
}

fn view_observation_parameter(
    position: u32,
    parameters: &[(u32, StructuralParameterDeclaration)],
    length_only: bool,
) -> Result<&StructuralParameterDeclaration, LoweringError> {
    let parameter = parameters
        .iter()
        .find_map(|(source_position, parameter)| {
            (*source_position == position).then_some(parameter)
        })
        .ok_or(LoweringError::Unsupported(
            "view observation has no exact structural parameter",
        ))?;
    if !(parameter.access == StructuralAccess::SharedBorrow
        || (length_only && parameter.access == StructuralAccess::MutableBorrow))
        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
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

pub(crate) fn lower_checked_scalar_expression_with_parameters(
    expression: &CheckedScalarExpression,
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
    structural_fields: &[crate::expression_preparation::bindings::StructuralScalarFieldBinding],
    structural_cases: &[crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding],
    primitive_storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
    element_views: &std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
    view_locals: &[ViewLocalBinding],
) -> Result<LoweredDirectExpression, LoweringError> {
    match expression {
        CheckedScalarExpression::StructuralParameterByteLength { root, path } => {
            let count_type = terminal_scalar_type(PrimitiveType::U64)?;
            let (source, carrier) = match *root {
                checked_trees::CheckedStorageRoot::Parameter {
                    index: parameter_position,
                } => {
                    if !path.is_empty() {
                        let (source, path, field) =
                            crate::expression_preparation::bindings::structural_fields::resolve_byte_length(
                                structural_fields,
                                parameter_position,
                                path,
                            )?;
                        return Ok(LoweredDirectExpression::ByteSequenceFieldLength {
                            source,
                            path,
                            field,
                            scalar_type: count_type,
                        });
                    }
                    let parameter = view_observation_parameter(
                        parameter_position,
                        structural_parameters,
                        true,
                    )?;
                    (
                        parameter.place,
                        parameter_view_carrier(element_views, parameter.structural_type),
                    )
                }
                checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                    view_local_observation(view_locals, symbol, path)?
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
                source,
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
                        CasePayloadRead::Deferred { source, case, field } => {
                            LoweredDirectExpression::StructuralField {
                                source,
                                path: vec![CanonicalStructuralPathSegment::Case(case)],
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
            // whose carrier may itself cross one literal index.
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
                source,
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
        } => Ok(LoweredDirectExpression::IntegerBinary {
            kind: match kind {
                CheckedIntegerBinaryKind::ExactAdd => LoweredIntegerBinaryKind::ExactAdd,
                CheckedIntegerBinaryKind::ExactSubtract => LoweredIntegerBinaryKind::ExactSubtract,
                CheckedIntegerBinaryKind::ExactMultiply => LoweredIntegerBinaryKind::ExactMultiply,
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
                CheckedIntegerBinaryKind::SaturatingAdd => LoweredIntegerBinaryKind::SaturatingAdd,
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
                // No Terminal Trapping operation family exists yet; refuse
                // rather than silently weaken the policy into Exact.
                CheckedIntegerBinaryKind::TrappingShiftLeft
                | CheckedIntegerBinaryKind::TrappingShiftRight => {
                    return unsupported(
                        "checked trapping operation requires runtime policy realization",
                    );
                }
            },
            scalar_type: terminal_scalar_type(*primitive_type)?,
            left: Box::new(lower_checked_scalar_expression_with_parameters(
                left,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
            )?),
            right: Box::new(lower_checked_scalar_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
            )?),
        }),
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
            )?),
        }),
        CheckedScalarExpression::StructuralParameterIndexedRead {
            root,
            path,
            index,
            primitive_type,
        } => {
            let (source, carrier) = match *root {
                checked_trees::CheckedStorageRoot::Parameter {
                    index: parameter_position,
                } => {
                    if !path.is_empty() {
                        let index = lower_checked_scalar_expression_with_parameters(
                            index,
                            structural_parameters,
                            structural_fields,
                            structural_cases,
                            primitive_storage,
                            element_views,
                            view_locals,
                        )?;
                        if index.scalar_type() != terminal_scalar_type(PrimitiveType::U64)? {
                            return unsupported("indexed field reads require an exact u64 index");
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
                                scalar_type: element_scalar,
                            });
                        }
                        // An indexed read landing on a byte-sequence record
                        // field borrows the field's own dominating length
                        // observation, so bounded-owned and borrowed-view
                        // carriers share one read.
                        if *primitive_type != PrimitiveType::U8 {
                            return unsupported("indexed field reads require a u8 element");
                        }
                        let (source, path, field) =
                            crate::expression_preparation::bindings::structural_fields::resolve_byte_length(
                                structural_fields,
                                parameter_position,
                                path,
                            )?;
                        return Ok(LoweredDirectExpression::ByteSequenceFieldRead {
                            source,
                            path,
                            field,
                            index: Box::new(index),
                            scalar_type: terminal_scalar_type(PrimitiveType::U8)?,
                        });
                    }
                    let parameter = view_observation_parameter(
                        parameter_position,
                        structural_parameters,
                        false,
                    )?;
                    (
                        parameter.place,
                        parameter_view_carrier(element_views, parameter.structural_type),
                    )
                }
                checked_trees::CheckedStorageRoot::ViewLocal { symbol } => {
                    view_local_observation(view_locals, symbol, path)?
                }
            };
            let element_scalar = match carrier {
                ViewCarrier::Bytes => None,
                ViewCarrier::Elements {
                    element: Some(scalar_type),
                } => Some(scalar_type),
                ViewCarrier::Elements { element: None } => {
                    return unsupported("element-view reads of record elements yield no scalar");
                }
            };
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
            )?;
            if index.scalar_type() != terminal_scalar_type(PrimitiveType::U64)? {
                return unsupported("view indexed reads require an exact u64 index");
            }
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
            )?;
            let target = terminal_scalar_type(*primitive_type)?;
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return unsupported("wrapping conversion requires fixed integer carriers");
            };
            wrapping_cast::wrapping_cast(operand, source_type, target)
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
            )?;
            let target = terminal_scalar_type(*primitive_type)?;
            let (ScalarType::Integer(source_type), ScalarType::Integer(target_type)) =
                (operand.scalar_type(), target)
            else {
                return unsupported("saturating conversion requires fixed integer carriers");
            };
            if source_type == target_type {
                return Ok(operand);
            }
            // A widening carrier already clamps at the source range; the
            // widened operand is the saturating image.
            if source_type.can_widen_to(target_type) {
                return Ok(LoweredDirectExpression::IntegerWiden {
                    scalar_type: target,
                    operand: Box::new(operand),
                });
            }
            if source_type.sign() != IntegerSign::Unsigned
                || target_type.sign() != IntegerSign::Unsigned
            {
                return unsupported(
                    "saturating conversion with a signed carrier requires runtime policy realization",
                );
            }
            if target_type.bits() >= source_type.bits() {
                return unsupported("saturating conversion requires an unsigned narrowing carrier");
            }
            let maximum = (1_u128.checked_shl(u32::from(target_type.bits())))
                .and_then(|modulus| modulus.checked_sub(1))
                .ok_or(LoweringError::Unsupported(
                    "saturating conversion bound exceeds u128",
                ))?;
            let modulus = 1_u128.checked_shl(u32::from(target_type.bits())).ok_or(
                LoweringError::Unsupported("saturating conversion modulus exceeds u128"),
            )?;
            // `min(value, target_max)` is `value - (value sat_sub target_max)`
            // on unsigned carriers: saturating subtraction floors at zero, so
            // the clamp needs no branch. The modular remainder then carries
            // the bound that the exact cast obligation consumes, exactly as
            // the wrapping conversion does.
            let saturated = |value: u128| LoweredDirectExpression::IntegerLiteral {
                value: IntegerValue::Unsigned(value),
                scalar_type: ScalarType::Integer(source_type),
            };
            let clamped = LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::WrappingSubtract,
                scalar_type: ScalarType::Integer(source_type),
                left: Box::new(operand.clone()),
                right: Box::new(LoweredDirectExpression::IntegerBinary {
                    kind: LoweredIntegerBinaryKind::SaturatingSubtract,
                    scalar_type: ScalarType::Integer(source_type),
                    left: Box::new(operand),
                    right: Box::new(saturated(maximum)),
                }),
            };
            Ok(LoweredDirectExpression::IntegerExactCast {
                scalar_type: target,
                operand: Box::new(LoweredDirectExpression::IntegerBinary {
                    kind: LoweredIntegerBinaryKind::ExactRemainder,
                    scalar_type: ScalarType::Integer(source_type),
                    left: Box::new(clamped),
                    right: Box::new(saturated(modulus)),
                }),
            })
        }
        CheckedScalarExpression::IntegerTrappingCast { .. } => {
            unsupported("checked trapping conversion requires runtime policy realization")
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
        &[],
        &[],
        &[],
        &std::collections::BTreeMap::new(),
        &[],
    )
}

fn lower_checked_boolean_expression_with_parameters(
    expression: &CheckedBooleanExpression,
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
    structural_fields: &[crate::expression_preparation::bindings::StructuralScalarFieldBinding],
    structural_cases: &[crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding],
    primitive_storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
    element_views: &std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
    view_locals: &[ViewLocalBinding],
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
                source,
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
                        CasePayloadRead::Deferred { source, case, field } => {
                            LoweredBooleanReturnExpression::StructuralField {
                                source,
                                path: vec![CanonicalStructuralPathSegment::Case(case)],
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
                // may itself cross one literal index.
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
                    source,
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
                    | checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_) => {
                        unsupported("case-payload predicates are contract-only")
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            LoweredBooleanReturnExpression::UnresolvedStructuralParameterField {
                parameter_position: *parameter_position,
                path,
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
            )?),
            right: Box::new(lower_checked_boolean_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
            )?),
        },
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
                )?),
                right: Box::new(lower_checked_scalar_expression_with_parameters(
                    right,
                    structural_parameters,
                    structural_fields,
                    structural_cases,
                    primitive_storage,
                    element_views,
                    view_locals,
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
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
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
            )?),
            right: Box::new(lower_checked_boolean_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
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
            )?),
            right: Box::new(lower_checked_boolean_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
                element_views,
                view_locals,
            )?),
        },
    })
}
