//! Validates terminal operation operands against exact SSA value types.

use super::*;

pub(super) fn validate_operation_operands(
    operation: &psi_terminal::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if let OperationKind::Call {
        callee, arguments, ..
    } = &operation.kind
    {
        let callee = machines
            .get(callee)
            .copied()
            .expect("call target was validated during operation registration");
        if arguments.len() != callee.parameters.len() {
            return Err(ModuleError::CallArgumentArityMismatch {
                operation: operation.id,
                expected: callee.parameters.len(),
                actual: arguments.len(),
            });
        }
        for (argument, parameter) in arguments.iter().zip(&callee.parameters) {
            require_defined(*argument, value_types, defined)?;
            let actual = value_types[argument];
            if actual != parameter.scalar_type {
                return Err(ModuleError::CallArgumentTypeMismatch {
                    operation: operation.id,
                    argument: *argument,
                    expected: parameter.scalar_type,
                    actual,
                });
            }
        }
        return Ok(());
    }
    if let OperationKind::IntegerExactCast { operand, .. } = operation.kind.clone() {
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
        return Ok(());
    }
    if let OperationKind::IntegerWiden { operand } = operation.kind.clone() {
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
        return Ok(());
    }
    if let OperationKind::IntegerBitwiseNot { operand } = operation.kind.clone() {
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
        return Ok(());
    }
    if let OperationKind::BooleanNot { operand } = operation.kind.clone() {
        require_defined(operand, value_types, defined)?;
        let actual = value_types[&operand];
        if actual != ScalarType::Boolean {
            return Err(ModuleError::BooleanNotOperandTypeMismatch {
                operation: operation.id,
                operand,
                actual,
            });
        }
        return Ok(());
    }
    if let OperationKind::BooleanEqual { left, right } = operation.kind.clone() {
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
        return Ok(());
    }
    if let OperationKind::IntegerEqual { left, right } = operation.kind.clone() {
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
        return Ok(());
    }
    if let OperationKind::IntegerLessThan { left, right }
    | OperationKind::IntegerLessOrEqual { left, right } = operation.kind.clone()
    {
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
        return Ok(());
    }
    if let OperationKind::IntegerBitwiseAnd { left, right }
    | OperationKind::IntegerBitwiseOr { left, right }
    | OperationKind::IntegerBitwiseXor { left, right } = operation.kind.clone()
    {
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
        return Ok(());
    }
    if let OperationKind::WrappingIntegerShiftLeft { value, count }
    | OperationKind::WrappingIntegerShiftRight { value, count } = operation.kind.clone()
    {
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
        return Ok(());
    }
    if let OperationKind::ExactIntegerShiftLeft { value, count, .. }
    | OperationKind::ExactIntegerShiftRight { value, count, .. } = operation.kind.clone()
    {
        require_defined(value, value_types, defined)?;
        require_defined(count, value_types, defined)?;
        let expected_value = operation.result.expect_scalar().scalar_type;
        let actual_value = value_types[&value];
        let actual_count = value_types[&count];
        if !matches!(expected_value, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
            || actual_value != expected_value
            || !matches!(actual_count, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
        {
            return Err(ModuleError::ExactIntegerShiftOperandTypeMismatch {
                operation: operation.id,
                expected_value,
                actual_value,
                actual_count,
            });
        }
        return Ok(());
    }
    if let OperationKind::ExactIntegerAdd { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
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
        return Ok(());
    }
    if let OperationKind::ExactIntegerSubtract { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
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
        return Ok(());
    }
    if let OperationKind::ExactIntegerMultiply { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
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
        return Ok(());
    }
    if let OperationKind::ExactIntegerDivide { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
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
        return Ok(());
    }
    if let OperationKind::ExactIntegerRemainder { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
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
        return Ok(());
    }
    if let OperationKind::WrappingIntegerDivide { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
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
        return Ok(());
    }
    if let OperationKind::WrappingIntegerRemainder { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
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
        return Ok(());
    }
    if let OperationKind::SaturatingIntegerDivide { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
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
        return Ok(());
    }
    if let OperationKind::SaturatingIntegerRemainder { left, right, .. } = operation.kind.clone() {
        require_defined(left, value_types, defined)?;
        require_defined(right, value_types, defined)?;
        let expected = operation.result.expect_scalar().scalar_type;
        let actual_left = value_types[&left];
        let actual_right = value_types[&right];
        if !matches!(expected, ScalarType::Integer(integer) if integer.carrier() == psi_core::IntegerCarrier::Fixed)
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
        return Ok(());
    }
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
        | OperationKind::BooleanStructuralField { .. }
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
        | OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::BoundaryCall { .. }
        | OperationKind::PortWrite { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. } => None,
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

pub(super) fn require_defined(
    value: ValueId,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if !defined.contains(&value) {
        return Err(ModuleError::ValueUsedBeforeDefinition(value));
    }
    if !value_types.contains_key(&value) {
        return Err(ModuleError::UnknownValue(value));
    }
    Ok(())
}

enum ArithmeticOperandKind {
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
}
