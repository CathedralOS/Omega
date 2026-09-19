//! Operands of integer arithmetic: the exact and checked forms, and the
//! wrapping and saturating add, subtract and multiply.

use super::super::{BTreeMap, BTreeSet, ModuleError, OperationKind, ScalarType, ValueId};
use super::{ArithmeticOperandKind, require_defined};

pub(super) fn validate_exact_integer_add(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ExactIntegerAdd { left, right, .. } = operation.kind.clone() else {
        unreachable!("dispatched validate_exact_integer_add")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let actual_left = value_types[&left];
    let actual_right = value_types[&right];
    if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        || actual_left != expected
        || actual_right != expected
    {
        return Err(ModuleError::ExactIntegerAddOperandTypeMismatch {
            operation: operation.id,
            expected,
            actual_left,
            actual_right,
        });
    }
    Ok(())
}

pub(super) fn validate_exact_integer_subtract(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ExactIntegerSubtract { left, right, .. } = operation.kind.clone() else {
        unreachable!("dispatched validate_exact_integer_subtract")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let actual_left = value_types[&left];
    let actual_right = value_types[&right];
    if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        || actual_left != expected
        || actual_right != expected
    {
        return Err(ModuleError::ExactIntegerSubtractOperandTypeMismatch {
            operation: operation.id,
            expected,
            actual_left,
            actual_right,
        });
    }
    Ok(())
}

pub(super) fn validate_exact_integer_multiply(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ExactIntegerMultiply { left, right, .. } = operation.kind.clone() else {
        unreachable!("dispatched validate_exact_integer_multiply")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let actual_left = value_types[&left];
    let actual_right = value_types[&right];
    if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        || actual_left != expected
        || actual_right != expected
    {
        return Err(ModuleError::ExactIntegerMultiplyOperandTypeMismatch {
            operation: operation.id,
            expected,
            actual_left,
            actual_right,
        });
    }
    Ok(())
}

pub(super) fn validate_exact_integer_divide(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ExactIntegerDivide { left, right, .. } = operation.kind.clone() else {
        unreachable!("dispatched validate_exact_integer_divide")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let actual_left = value_types[&left];
    let actual_right = value_types[&right];
    if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        || actual_left != expected
        || actual_right != expected
    {
        return Err(ModuleError::ExactIntegerDivideOperandTypeMismatch {
            operation: operation.id,
            expected,
            actual_left,
            actual_right,
        });
    }
    Ok(())
}

pub(super) fn validate_exact_integer_remainder(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ExactIntegerRemainder { left, right, .. } = operation.kind.clone() else {
        unreachable!("dispatched validate_exact_integer_remainder")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let actual_left = value_types[&left];
    let actual_right = value_types[&right];
    if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        || actual_left != expected
        || actual_right != expected
    {
        return Err(ModuleError::ExactIntegerRemainderOperandTypeMismatch {
            operation: operation.id,
            expected,
            actual_left,
            actual_right,
        });
    }
    Ok(())
}

pub(super) fn validate_wrapping_integer_divide(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::WrappingIntegerDivide { left, right, .. } = operation.kind.clone() else {
        unreachable!("dispatched validate_wrapping_integer_divide")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let actual_left = value_types[&left];
    let actual_right = value_types[&right];
    if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        || actual_left != expected
        || actual_right != expected
    {
        return Err(ModuleError::WrappingIntegerDivideOperandTypeMismatch {
            operation: operation.id,
            expected,
            actual_left,
            actual_right,
        });
    }
    Ok(())
}

pub(super) fn validate_wrapping_integer_remainder(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::WrappingIntegerRemainder { left, right, .. } = operation.kind.clone() else {
        unreachable!("dispatched validate_wrapping_integer_remainder")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let actual_left = value_types[&left];
    let actual_right = value_types[&right];
    if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        || actual_left != expected
        || actual_right != expected
    {
        return Err(ModuleError::WrappingIntegerRemainderOperandTypeMismatch {
            operation: operation.id,
            expected,
            actual_left,
            actual_right,
        });
    }
    Ok(())
}

pub(super) fn validate_saturating_integer_divide(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::SaturatingIntegerDivide { left, right, .. } = operation.kind.clone() else {
        unreachable!("dispatched validate_saturating_integer_divide")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let actual_left = value_types[&left];
    let actual_right = value_types[&right];
    if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        || actual_left != expected
        || actual_right != expected
    {
        return Err(ModuleError::SaturatingIntegerDivideOperandTypeMismatch {
            operation: operation.id,
            expected,
            actual_left,
            actual_right,
        });
    }
    Ok(())
}

pub(super) fn validate_saturating_integer_remainder(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::SaturatingIntegerRemainder { left, right, .. } = operation.kind.clone()
    else {
        unreachable!("dispatched validate_saturating_integer_remainder")
    };
    require_defined(left, value_types, defined)?;
    require_defined(right, value_types, defined)?;
    let expected = operation.result.expect_scalar().scalar_type;
    let actual_left = value_types[&left];
    let actual_right = value_types[&right];
    if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
        || actual_left != expected
        || actual_right != expected
    {
        return Err(ModuleError::SaturatingIntegerRemainderOperandTypeMismatch {
            operation: operation.id,
            expected,
            actual_left,
            actual_right,
        });
    }
    Ok(())
}

/// The wrapping and saturating add, subtract and multiply operands: both
/// must carry the integer result type. Every other kind reaching here has
/// no scalar operands to check.
pub(super) fn validate_binary_arithmetic(
    operation: &terminal_psi::Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let Some((left, right, arithmetic)) = (match operation.kind.clone() {
        OperationKind::WrappingIntegerAdd { left, right } => {
            Some((left, right, ArithmeticOperandKind::WrappingAdd))
        }
        OperationKind::SaturatingIntegerAdd { left, right } => {
            Some((left, right, ArithmeticOperandKind::SaturatingAdd))
        }
        OperationKind::WrappingIntegerSubtract { left, right } => {
            Some((left, right, ArithmeticOperandKind::WrappingSubtract))
        }
        OperationKind::SaturatingIntegerSubtract { left, right } => {
            Some((left, right, ArithmeticOperandKind::SaturatingSubtract))
        }
        OperationKind::WrappingIntegerMultiply { left, right } => {
            Some((left, right, ArithmeticOperandKind::WrappingMultiply))
        }
        OperationKind::SaturatingIntegerMultiply { left, right } => {
            Some((left, right, ArithmeticOperandKind::SaturatingMultiply))
        }
        OperationKind::IntegerConstant { .. }
        | OperationKind::BooleanConstant { .. }
        | OperationKind::IeeeFloatConstant { .. }
        | OperationKind::IeeeFloatCompare { .. }
        | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::StructuralCaseMembership { .. }
        | OperationKind::IntegerStructuralField { .. }
        | OperationKind::MoveStructuralField { .. }
        | OperationKind::StoreStructuralField { .. }
        | OperationKind::ByteSequenceLength { .. }
        | OperationKind::ByteSequenceRead { .. }
        | OperationKind::ByteSequenceWrite { .. }
        | OperationKind::ByteSequenceSubslice { .. }
        | OperationKind::BooleanNot { .. }
        | OperationKind::BooleanEqual { .. }
        | OperationKind::IntegerEqual { .. }
        | OperationKind::IntegerLessThan { .. }
        | OperationKind::IntegerLessOrEqual { .. }
        | OperationKind::IntegerBitwiseNot { .. }
        | OperationKind::IntegerWiden { .. }
        | OperationKind::IntegerExactCast { .. }
        | OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. }
        | OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. }
        | OperationKind::ExactIntegerShiftLeft { .. }
        | OperationKind::ExactIntegerShiftRight { .. }
        | OperationKind::ExactIntegerAdd { .. }
        | OperationKind::ExactIntegerSubtract { .. }
        | OperationKind::ExactIntegerMultiply { .. } => None,
        OperationKind::ExactIntegerDivide { .. } => None,
        OperationKind::ExactIntegerRemainder { .. } => None,
        OperationKind::WrappingIntegerDivide { .. } => None,
        OperationKind::WrappingIntegerRemainder { .. } => None,
        OperationKind::SaturatingIntegerDivide { .. } => None,
        OperationKind::SaturatingIntegerRemainder { .. } => None,
        OperationKind::Call { .. }
        | OperationKind::EstablishReference { .. }
        | OperationKind::ReleaseReference { .. }
        | OperationKind::EstablishPrimitiveLocal { .. }
        | OperationKind::PrimitiveScalarRead { .. }
        | OperationKind::WriteOnlyPrimitiveStore { .. }
        | OperationKind::WriteOnlyIndexedPrimitiveStore { .. }
        | OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::StructuralByteSequenceFieldStore { .. }
        | OperationKind::StructuralByteSequenceFieldLength { .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { .. }
        | OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterUnit { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. }
        | OperationKind::EstablishScalarCase { .. }
        | OperationKind::EstablishScalarArray { .. }
        | OperationKind::BoundaryCall { .. }
        | OperationKind::PortWrite { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. }
        | OperationKind::EstablishRecord { .. }
        | OperationKind::StoreDynamicDescriptor { .. } => None,
    }) else {
        return Ok(());
    };
    let ScalarType::Integer(integer_type) = operation.result.expect_scalar().scalar_type else {
        unreachable!("operation shape validation requires an integer result")
    };
    for operand in [left, right] {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        let expected = ScalarType::Integer(integer_type);
        if actual != expected {
            return Err(match arithmetic {
                ArithmeticOperandKind::SaturatingAdd => {
                    ModuleError::SaturatingIntegerAddOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::WrappingAdd => {
                    ModuleError::WrappingIntegerAddOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::WrappingSubtract => {
                    ModuleError::WrappingIntegerSubtractOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::SaturatingSubtract => {
                    ModuleError::SaturatingIntegerSubtractOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::WrappingMultiply => {
                    ModuleError::WrappingIntegerMultiplyOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
                ArithmeticOperandKind::SaturatingMultiply => {
                    ModuleError::SaturatingIntegerMultiplyOperandTypeMismatch {
                        operation: operation.id,
                        operand,
                        expected,
                        actual,
                    }
                }
            });
        }
    }
    Ok(())
}
