//! Checked boolean and scalar terms.

use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::{
    LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
};
use crate::proofs::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression, LoweringError,
    ScalarTerm, ScalarType, ValueDeclaration, lower_checked_scalar_expression, unsupported,
};

pub(crate) fn checked_boolean_scalar_term(
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    Ok(match expression {
        CheckedBooleanExpression::ErasedParameter { position } => {
            let value = erased.get(*position).ok_or(LoweringError::Unsupported(
                "erased formal position is outside the contract's erased roster",
            ))?;
            if value.scalar_type != ScalarType::Boolean {
                return unsupported("erased formal predicate value has a non-Boolean type");
            }
            ScalarTerm::value(value.id, value.scalar_type)
        }
        CheckedBooleanExpression::Constant(value) => ScalarTerm::boolean(*value),
        CheckedBooleanExpression::StorageRead { .. } => {
            return unsupported(
                "crash predicate requires an established scalar value, not mutable storage",
            );
        }
        CheckedBooleanExpression::Parameter { position }
        | CheckedBooleanExpression::Local { position } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            if value.scalar_type != ScalarType::Boolean {
                return unsupported("crash predicate Boolean value has a non-Boolean type");
            }
            ScalarTerm::value(value.id, value.scalar_type)
        }
        CheckedBooleanExpression::StructuralParameterField { .. } => {
            return unsupported(
                "structural crash predicates require structural signature lowering",
            );
        }
        CheckedBooleanExpression::Not(operand) => {
            ScalarTerm::boolean_not(checked_boolean_scalar_term(operand, values, erased)?)
                .map_err(LoweringError::InvalidCrashPredicate)?
        }
        CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
            checked_boolean_scalar_term(left, values, erased)?,
            checked_boolean_scalar_term(right, values, erased)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate)?,
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            let left = checked_scalar_term(left, values, erased)?;
            let right = checked_scalar_term(right, values, erased)?;
            let ScalarType::Integer(integer_type) = left.scalar_type() else {
                return unsupported("crash comparison operand is not an integer");
            };
            match kind {
                CheckedIntegerComparisonKind::Equal => {
                    ScalarTerm::integer_equal(integer_type, left, right)
                }
                CheckedIntegerComparisonKind::LessThan => {
                    ScalarTerm::integer_less_than(integer_type, left, right)
                }
                CheckedIntegerComparisonKind::LessOrEqual => {
                    ScalarTerm::integer_less_or_equal(integer_type, left, right)
                }
            }
            .map_err(LoweringError::InvalidCrashPredicate)?
        }
        CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => {
            return unsupported("structural equality requires structural signature lowering");
        }
        CheckedBooleanExpression::And { .. } | CheckedBooleanExpression::Or { .. } => {
            return unsupported(
                "short-circuit Boolean crash predicate is not one scalar terminal term",
            );
        }
    })
}

pub(crate) fn checked_scalar_term(
    expression: &CheckedScalarExpression,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let expression = lower_checked_scalar_expression(expression)?;
    lowered_direct_scalar_term(&expression, values, erased)
}

pub(crate) fn lowered_direct_scalar_term(
    expression: &LoweredDirectExpression,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    Ok(match expression {
        LoweredDirectExpression::ErasedParameter {
            position,
            scalar_type,
        } => {
            let value = erased.get(*position).ok_or(LoweringError::Unsupported(
                "erased formal position is outside the contract's erased roster",
            ))?;
            if value.scalar_type != *scalar_type {
                return unsupported("erased formal term type does not match its checked plan");
            }
            ScalarTerm::value(value.id, *scalar_type)
        }
        LoweredDirectExpression::PrimitiveRead { .. } => {
            return unsupported(
                "primitive storage read requires an occurrence-bound crash predicate",
            );
        }
        LoweredDirectExpression::StructuralField { .. } => {
            return unsupported("runtime field read requires an occurrence-bound crash predicate");
        }
        LoweredDirectExpression::ByteSequenceLength { .. }
        | LoweredDirectExpression::ByteSequenceFieldLength { .. }
        | LoweredDirectExpression::ByteSequenceRead { .. } => {
            return unsupported("byte observation has no retained crash predicate term");
        }
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        }
        | LoweredDirectExpression::Local {
            position,
            scalar_type,
        } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            if value.scalar_type != *scalar_type {
                return unsupported("crash predicate value type does not match its checked plan");
            }
            ScalarTerm::value(value.id, *scalar_type)
        }
        LoweredDirectExpression::IntegerLiteral { value, scalar_type } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return unsupported("crash predicate integer literal has a non-integer type");
            };
            ScalarTerm::integer(*integer_type, *value)
                .map_err(LoweringError::InvalidCrashPredicate)?
        }
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return unsupported("crash predicate arithmetic has a non-integer type");
            };
            let left = Box::new(lowered_direct_scalar_term(left, values, erased)?);
            let right = Box::new(lowered_direct_scalar_term(right, values, erased)?);
            match kind {
                LoweredIntegerBinaryKind::BitwiseAnd => ScalarTerm::IntegerBitwiseAnd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::BitwiseOr => ScalarTerm::IntegerBitwiseOr {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::BitwiseXor => ScalarTerm::IntegerBitwiseXor {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingShiftLeft => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::WrappingIntegerShiftLeft {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::WrappingShiftRight => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::WrappingIntegerShiftRight {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::ExactShiftLeft => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::ExactIntegerShiftLeft {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::ExactShiftRight => {
                    let ScalarType::Integer(count_type) = right.scalar_type() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::ExactIntegerShiftRight {
                        value_type: *integer_type,
                        count_type,
                        value: left,
                        count: right,
                    }
                }
                LoweredIntegerBinaryKind::ExactAdd => ScalarTerm::ExactIntegerAdd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactSubtract => ScalarTerm::ExactIntegerSubtract {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactMultiply => ScalarTerm::ExactIntegerMultiply {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactDivide => ScalarTerm::ExactIntegerDivide {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::ExactRemainder => ScalarTerm::ExactIntegerRemainder {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingDivide => ScalarTerm::WrappingIntegerDivide {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingRemainder => {
                    ScalarTerm::WrappingIntegerRemainder {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
                LoweredIntegerBinaryKind::SaturatingDivide => ScalarTerm::SaturatingIntegerDivide {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingRemainder => {
                    ScalarTerm::SaturatingIntegerRemainder {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
                LoweredIntegerBinaryKind::WrappingAdd => ScalarTerm::WrappingIntegerAdd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingAdd => ScalarTerm::SaturatingIntegerAdd {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::WrappingSubtract => ScalarTerm::WrappingIntegerSubtract {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingSubtract => {
                    ScalarTerm::SaturatingIntegerSubtract {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
                LoweredIntegerBinaryKind::WrappingMultiply => ScalarTerm::WrappingIntegerMultiply {
                    scalar_type: *integer_type,
                    left,
                    right,
                },
                LoweredIntegerBinaryKind::SaturatingMultiply => {
                    ScalarTerm::SaturatingIntegerMultiply {
                        scalar_type: *integer_type,
                        left,
                        right,
                    }
                }
            }
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return unsupported("crash predicate bitwise-not has a non-integer type");
            };
            ScalarTerm::IntegerBitwiseNot {
                scalar_type: *integer_type,
                operand: Box::new(lowered_direct_scalar_term(operand, values, erased)?),
            }
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(target_type) = scalar_type else {
                return unsupported("crash predicate widen has a non-integer target");
            };
            let operand = lowered_direct_scalar_term(operand, values, erased)?;
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return unsupported("crash predicate widen has a non-integer operand");
            };
            ScalarTerm::IntegerWiden {
                source_type,
                target_type: *target_type,
                operand: Box::new(operand),
            }
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(target_type) = scalar_type else {
                return unsupported("crash predicate cast has a non-integer target");
            };
            let operand = lowered_direct_scalar_term(operand, values, erased)?;
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return unsupported("crash predicate cast has a non-integer operand");
            };
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: *target_type,
                operand: Box::new(operand),
            }
        }
        LoweredDirectExpression::IeeeFloatLiteral { .. } => {
            return unsupported("generic scalar crash terms do not carry IEEE float literals");
        }
        LoweredDirectExpression::Boolean { expression } => {
            return checked_boolean_scalar_term_from_lowered(expression, values, erased);
        }
    })
}

fn checked_boolean_scalar_term_from_lowered(
    expression: &LoweredBooleanReturnExpression,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. } => {
            unsupported("storage observation requires an occurrence-bound crash predicate")
        }
        LoweredBooleanReturnExpression::Constant { value } => Ok(ScalarTerm::boolean(*value)),
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            (value.scalar_type == ScalarType::Boolean)
                .then(|| ScalarTerm::value(value.id, value.scalar_type))
                .ok_or(LoweringError::Unsupported(
                    "crash predicate Boolean value has a non-Boolean type",
                ))
        }
        LoweredBooleanReturnExpression::StructuralField {
            source,
            path,
            field,
        } => {
            let mut path = path.clone();
            path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                *field,
            ));
            Ok(ScalarTerm::boolean_field_path(*source, path))
        }
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed crash-predicate lowering")
        }
        LoweredBooleanReturnExpression::Not { operand } => ScalarTerm::boolean_not(
            checked_boolean_scalar_term_from_lowered(operand, values, erased)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate),
        LoweredBooleanReturnExpression::Equal { left, right } => ScalarTerm::boolean_equal(
            checked_boolean_scalar_term_from_lowered(left, values, erased)?,
            checked_boolean_scalar_term_from_lowered(right, values, erased)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate),
        LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
            let left = lowered_direct_scalar_term(left, values, erased)?;
            let right = lowered_direct_scalar_term(right, values, erased)?;
            let ScalarType::Integer(integer_type) = left.scalar_type() else {
                return unsupported("crash comparison operand is not an integer");
            };
            match kind {
                LoweredIntegerComparisonKind::Equal => {
                    ScalarTerm::integer_equal(integer_type, left, right)
                }
                LoweredIntegerComparisonKind::LessThan => {
                    ScalarTerm::integer_less_than(integer_type, left, right)
                }
                LoweredIntegerComparisonKind::LessOrEqual => {
                    ScalarTerm::integer_less_or_equal(integer_type, left, right)
                }
            }
            .map_err(LoweringError::InvalidCrashPredicate)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            unsupported("short-circuit Boolean crash predicate is not one scalar terminal term")
        }
    }
}
