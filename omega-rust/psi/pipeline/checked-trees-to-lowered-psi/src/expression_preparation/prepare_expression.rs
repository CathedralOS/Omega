//! Lowering checked scalar and boolean expressions.

use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::{
    LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
};
use crate::emission::scalar_types::{
    integer_landing_scalar_type, integer_value, terminal_scalar_type,
};
use crate::expression_preparation::source_custody;
use crate::expression_preparation::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees, IntegerSign, IntegerValue,
    LoweringError, PlaceId, PrimitiveType, ScalarType, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, unsupported,
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
    lower_checked_scalar_expression_with_parameters(expression, &[], &[], &[], &[])
}

fn byte_observation_parameter(
    position: u32,
    parameters: &[(u32, StructuralParameterDeclaration)],
    length_only: bool,
) -> Result<PlaceId, LoweringError> {
    let parameter = parameters
        .iter()
        .find_map(|(source_position, parameter)| {
            (*source_position == position).then_some(parameter)
        })
        .ok_or(LoweringError::Unsupported(
            "byte observation has no exact structural parameter",
        ))?;
    if !(parameter.access == StructuralAccess::SharedBorrow
        || (length_only && parameter.access == StructuralAccess::MutableBorrow))
        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
    {
        return unsupported(
            "byte observation requires an exact whole view with access for this observation",
        );
    }
    Ok(parameter.place)
}

pub(crate) fn lower_checked_scalar_expression_with_parameters(
    expression: &CheckedScalarExpression,
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
    structural_fields: &[crate::expression_preparation::bindings::StructuralScalarFieldBinding],
    structural_cases: &[crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding],
    primitive_storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
) -> Result<LoweredDirectExpression, LoweringError> {
    match expression {
        CheckedScalarExpression::StructuralParameterByteLength {
            parameter_position,
            path,
        } => {
            if !path.is_empty() {
                let (source, path, field) =
                    crate::expression_preparation::bindings::structural_fields::resolve_byte_length(
                        structural_fields,
                        *parameter_position,
                        path,
                    )?;
                return Ok(LoweredDirectExpression::ByteSequenceFieldLength {
                    source,
                    path,
                    field,
                    scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                });
            }
            Ok(LoweredDirectExpression::ByteSequenceLength {
                source: byte_observation_parameter(
                    *parameter_position,
                    structural_parameters,
                    true,
                )?,
                scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
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
            if path.iter().any(|segment| {
                matches!(
                    segment,
                    checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_)
                )
            }) {
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
            },
            scalar_type: terminal_scalar_type(*primitive_type)?,
            left: Box::new(lower_checked_scalar_expression_with_parameters(
                left,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
            )?),
            right: Box::new(lower_checked_scalar_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
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
            )?),
        }),
        CheckedScalarExpression::StructuralParameterIndexedRead {
            parameter_position,
            path,
            index,
            primitive_type,
        } => {
            if !path.is_empty() || *primitive_type != PrimitiveType::U8 {
                return unsupported("indexed reads require a whole byte-view parameter");
            }
            let source =
                byte_observation_parameter(*parameter_position, structural_parameters, false)?;
            let index = lower_checked_scalar_expression_with_parameters(
                index,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
            )?;
            if index.scalar_type() != terminal_scalar_type(PrimitiveType::U64)? {
                return unsupported("byte-view indexed reads require an exact u64 index");
            }
            Ok(LoweredDirectExpression::ByteSequenceRead {
                source,
                index: Box::new(index),
                scalar_type: terminal_scalar_type(PrimitiveType::U8)?,
            })
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
            )?;
            let target = terminal_scalar_type(*primitive_type)?;
            let (ScalarType::Integer(source_type), ScalarType::Integer(target_type)) =
                (operand.scalar_type(), target)
            else {
                return unsupported("wrapping conversion requires fixed integer carriers");
            };
            if source_type == target_type {
                return Ok(operand);
            }
            // A widening carrier already contains every source value, so the
            // modular image is the widened operand itself; this holds for
            // signed sources and signed targets without a signed modular
            // operator.
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
                    "signed wrapping conversion requires runtime policy realization",
                );
            }
            if target_type.bits() >= source_type.bits() {
                return unsupported("wrapping conversion requires an unsigned narrowing carrier");
            }
            let modulus = 1_u128.checked_shl(u32::from(target_type.bits())).ok_or(
                LoweringError::Unsupported("wrapping conversion modulus exceeds u128"),
            )?;
            // Unsigned remainder has exactly the destination's modular image.
            // Existing remainder and cast obligations independently prove the bound.
            Ok(LoweredDirectExpression::IntegerExactCast {
                scalar_type: target,
                operand: Box::new(LoweredDirectExpression::IntegerBinary {
                    kind: LoweredIntegerBinaryKind::ExactRemainder,
                    scalar_type: ScalarType::Integer(source_type),
                    left: Box::new(operand),
                    right: Box::new(LoweredDirectExpression::IntegerLiteral {
                        value: IntegerValue::Unsigned(modulus),
                        scalar_type: ScalarType::Integer(source_type),
                    }),
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
            )?),
        }),
        CheckedScalarExpression::Boolean(expression) => Ok(LoweredDirectExpression::Boolean {
            expression: Box::new(lower_checked_boolean_expression_with_parameters(
                expression,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
            )?),
        }),
    }
}

#[cfg(test)]
pub(crate) fn lower_checked_boolean_expression(
    expression: &CheckedBooleanExpression,
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    lower_checked_boolean_expression_with_parameters(expression, &[], &[], &[], &[])
}

fn lower_checked_boolean_expression_with_parameters(
    expression: &CheckedBooleanExpression,
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
    structural_fields: &[crate::expression_preparation::bindings::StructuralScalarFieldBinding],
    structural_cases: &[crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding],
    primitive_storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
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
            if !structural_fields.is_empty() {
                if path.iter().any(|segment| {
                    matches!(
                        segment,
                        checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_)
                    )
                }) {
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
            )?),
        },
        CheckedBooleanExpression::Equal { left, right } => LoweredBooleanReturnExpression::Equal {
            left: Box::new(lower_checked_boolean_expression_with_parameters(
                left,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
            )?),
            right: Box::new(lower_checked_boolean_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
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
                )?),
                right: Box::new(lower_checked_scalar_expression_with_parameters(
                    right,
                    structural_parameters,
                    structural_fields,
                    structural_cases,
                    primitive_storage,
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
            )?),
            right: Box::new(lower_checked_boolean_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
            )?),
        },
        CheckedBooleanExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(lower_checked_boolean_expression_with_parameters(
                left,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
            )?),
            right: Box::new(lower_checked_boolean_expression_with_parameters(
                right,
                structural_parameters,
                structural_fields,
                structural_cases,
                primitive_storage,
            )?),
        },
    })
}
