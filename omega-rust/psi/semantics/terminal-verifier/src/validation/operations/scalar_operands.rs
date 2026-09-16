//! Operands of casts, widening, Boolean and integer comparisons, bitwise
//! logic and shifts.

use super::super::{BTreeMap, BTreeSet, ModuleError, OperationKind, ScalarType, ValueId};
use super::require_defined;

pub(super) fn validate_ieee_float_compare(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::IeeeFloatCompare { left, right, .. } = operation.kind else {
        unreachable!("dispatched validate_ieee_float_compare")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let left_type = value_types[&left];
    let right_type = value_types[&right];
    if !matches!(left_type, ScalarType::IeeeFloat(_)) || left_type != right_type {
        return Err(ModuleError::IeeeFloatComparisonOperandTypeMismatch {
            operation: operation.id,
            left: left_type,
            right: right_type,
        });
    }
    Ok(())
}

pub(super) fn validate_nearest_ieee_float_fused_multiply_add(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::NearestIeeeFloatFusedMultiplyAdd {
        left,
        right,
        addend,
    } = operation.kind
    else {
        unreachable!("dispatched validate_nearest_ieee_float_fused_multiply_add")
    };
    let expected = operation.result.expect_scalar().scalar_type;
    for operand in [left, right, addend] {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        if actual != expected {
            return Err(ModuleError::IeeeFloatFusedMultiplyAddOperandTypeMismatch {
                operation: operation.id,
                operand,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_integer_exact_cast(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::IntegerExactCast { operand, .. } = operation.kind.clone() else {
        unreachable!("dispatched validate_integer_exact_cast")
    };
    require_defined(operand, value_types, defined)?;
    let actual = value_types[&operand];
    let expected = operation.result.expect_scalar().scalar_type;
    let (ScalarType::Integer(source), ScalarType::Integer(target)) = (actual, expected) else {
        return Err(ModuleError::IntegerExactCastOperandTypeMismatch {
            operation: operation.id,
            source: actual,
            target: expected,
        });
    };
    if !source.can_exact_cast_to(target) || source.can_widen_to(target) || source == target {
        return Err(ModuleError::IntegerExactCastOperandTypeMismatch {
            operation: operation.id,
            source: actual,
            target: expected,
        });
    }
    Ok(())
}

pub(super) fn validate_integer_widen(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::IntegerWiden { operand } = operation.kind.clone() else {
        unreachable!("dispatched validate_integer_widen")
    };
    require_defined(operand, value_types, defined)?;
    let actual = value_types[&operand];
    let expected = operation.result.expect_scalar().scalar_type;
    let (ScalarType::Integer(source), ScalarType::Integer(target)) = (actual, expected) else {
        return Err(ModuleError::IntegerWidenOperandTypeMismatch {
            operation: operation.id,
            source: actual,
            target: expected,
        });
    };
    if !source.can_widen_to(target) {
        return Err(ModuleError::IntegerWidenOperandTypeMismatch {
            operation: operation.id,
            source: actual,
            target: expected,
        });
    }
    Ok(())
}

pub(super) fn validate_integer_bitwise_not(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::IntegerBitwiseNot { operand } = operation.kind.clone() else {
        unreachable!("dispatched validate_integer_bitwise_not")
    };
    require_defined(operand, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let actual = value_types[&operand];
    if !matches!(expected, ScalarType::Integer(_)) || actual != expected {
        return Err(ModuleError::IntegerBitwiseNotOperandTypeMismatch {
            operation: operation.id,
            expected,
            actual,
        });
    }
    Ok(())
}

pub(super) fn validate_boolean_not(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::BooleanNot { operand } = operation.kind.clone() else {
        unreachable!("dispatched validate_boolean_not")
    };
    require_defined(operand, value_types, defined)?;
    let actual = value_types[&operand];
    if actual != ScalarType::Boolean {
        return Err(ModuleError::BooleanNotOperandTypeMismatch {
            operation: operation.id,
            operand,
            actual,
        });
    }
    Ok(())
}

pub(super) fn validate_boolean_equal(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::BooleanEqual { left, right } = operation.kind.clone() else {
        unreachable!("dispatched validate_boolean_equal")
    };
    for operand in [left, right] {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        if actual != ScalarType::Boolean {
            return Err(ModuleError::BooleanEqualOperandTypeMismatch {
                operation: operation.id,
                operand,
                actual,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_integer_equal(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::IntegerEqual { left, right } = operation.kind.clone() else {
        unreachable!("dispatched validate_integer_equal")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let left_type = value_types[&left];
    let right_type = value_types[&right];
    if !matches!(left_type, ScalarType::Integer(_)) || right_type != left_type {
        return Err(ModuleError::IntegerEqualOperandTypeMismatch {
            operation: operation.id,
            left: left_type,
            right: right_type,
        });
    }
    Ok(())
}

pub(super) fn validate_integer_comparison(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let (OperationKind::IntegerLessThan { left, right }
    | OperationKind::IntegerLessOrEqual { left, right }) = operation.kind.clone()
    else {
        unreachable!("dispatched validate_integer_comparison")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let left_type = value_types[&left];
    let right_type = value_types[&right];
    if !matches!(left_type, ScalarType::Integer(_)) || right_type != left_type {
        return Err(ModuleError::IntegerOrderingOperandTypeMismatch {
            operation: operation.id,
            left: left_type,
            right: right_type,
        });
    }
    Ok(())
}

pub(super) fn validate_integer_bitwise(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let (OperationKind::IntegerBitwiseAnd { left, right }
    | OperationKind::IntegerBitwiseOr { left, right }
    | OperationKind::IntegerBitwiseXor { left, right }) = operation.kind.clone()
    else {
        unreachable!("dispatched validate_integer_bitwise")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let left_type = value_types[&left];
    let right_type = value_types[&right];
    if !matches!(expected, ScalarType::Integer(_))
        || left_type != expected
        || right_type != expected
    {
        return Err(ModuleError::IntegerBitwiseOperandTypeMismatch {
            operation: operation.id,
            expected,
            left: left_type,
            right: right_type,
        });
    }
    Ok(())
}

pub(super) fn validate_wrapping_shift(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let (OperationKind::WrappingIntegerShiftLeft { value, count }
    | OperationKind::WrappingIntegerShiftRight { value, count }) = operation.kind.clone()
    else {
        unreachable!("dispatched validate_wrapping_shift")
    };
    require_defined(value, value_types, defined)?;
    require_defined(count, value_types, defined)?;
    let expected_value = operation.result.expect_scalar().scalar_type;
    let actual_value = value_types[&value];
    let actual_count = value_types[&count];
    if !matches!(expected_value, ScalarType::Integer(_))
        || actual_value != expected_value
        || !matches!(actual_count, ScalarType::Integer(_))
    {
        return Err(ModuleError::WrappingIntegerShiftOperandTypeMismatch {
            operation: operation.id,
            expected_value,
            actual_value,
            actual_count,
        });
    }
    Ok(())
}

pub(super) fn validate_exact_shift(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let (OperationKind::ExactIntegerShiftLeft { value, count, .. }
    | OperationKind::ExactIntegerShiftRight { value, count, .. }) = operation.kind.clone()
    else {
        unreachable!("dispatched validate_exact_shift")
    };
    require_defined(value, value_types, defined)?;
    require_defined(count, value_types, defined)?;
    let expected_value = operation.result.expect_scalar().scalar_type;
    let actual_value = value_types[&value];
    let actual_count = value_types[&count];
    if !matches!(expected_value, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        || actual_value != expected_value
        || !matches!(actual_count, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
    {
        return Err(ModuleError::ExactIntegerShiftOperandTypeMismatch {
            operation: operation.id,
            expected_value,
            actual_value,
            actual_count,
        });
    }
    Ok(())
}
