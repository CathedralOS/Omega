//! Validates terminal operation operands against exact SSA value types.

use super::*;

pub(super) fn validate_operation_operands(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &psi_terminal::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    boundary_machines: &[BoundaryMachineDeclaration],
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if let OperationKind::NearestIeeeFloatFusedMultiplyAdd {
        left,
        right,
        addend,
    } = operation.kind
    {
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
        return Ok(());
    }
    if let OperationKind::WriteOnlyPrimitiveStore { destination, value } = operation.kind {
        require_defined(value, value_types, defined)?;
        let structural_type = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == destination)
            .map(|parameter| parameter.structural_type)
            .ok_or(ModuleError::WriteOnlyPrimitiveStoreDestinationMismatch {
                operation: operation.id,
                place: destination,
            })?;
        let expected = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar_type) => Some(scalar_type),
                _ => None,
            })
            .ok_or(
                ModuleError::WriteOnlyPrimitiveStoreRequiresPrimitiveScalar {
                    operation: operation.id,
                    structural_type,
                },
            )?;
        let actual = value_types[&value];
        if actual != expected {
            return Err(ModuleError::WriteOnlyPrimitiveStoreValueTypeMismatch {
                operation: operation.id,
                expected,
                actual,
            });
        }
        return Ok(());
    }
    if let OperationKind::StructuralScalarFieldStore {
        destination,
        ref path,
        field,
        value,
    } = operation.kind
    {
        require_defined(value, value_types, defined)?;
        let expected = super::structural_scalar_fields::structural_scalar_field_store_type(
            module,
            machine,
            operation.id,
            destination,
            path,
            field,
        )?;
        let actual = value_types[&value];
        if actual != expected {
            return Err(ModuleError::StructuralScalarFieldStoreValueTypeMismatch {
                operation: operation.id,
                expected,
                actual,
            });
        }
        return Ok(());
    }
    if let OperationKind::Call {
        callee, arguments, ..
    } = &operation.kind
    {
        let callee = machines
            .get(callee)
            .copied()
            .expect("call target was validated during operation registration");
        validate_call_arguments(
            operation.id,
            arguments,
            &callee
                .parameters
                .iter()
                .map(|parameter| parameter.scalar_type)
                .collect::<Vec<_>>(),
            value_types,
            defined,
            ScalarCallKind::Ordinary,
        )?;
        return Ok(());
    }
    if let OperationKind::CallStructuralScalar {
        callee, arguments, ..
    } = &operation.kind
    {
        let callee = machines
            .get(callee)
            .copied()
            .expect("structural scalar call target was validated during operation registration");
        validate_call_arguments(
            operation.id,
            arguments,
            &callee
                .parameters
                .iter()
                .map(|parameter| parameter.scalar_type)
                .collect::<Vec<_>>(),
            value_types,
            defined,
            ScalarCallKind::Ordinary,
        )?;
        return Ok(());
    }
    if let OperationKind::CallStructuralWithScalarArguments {
        callee, arguments, ..
    } = &operation.kind
    {
        let callee = machines
            .get(callee)
            .copied()
            .expect("mixed structural call target was validated during operation registration");
        validate_call_arguments(
            operation.id,
            arguments,
            &callee
                .parameters
                .iter()
                .map(|parameter| parameter.scalar_type)
                .collect::<Vec<_>>(),
            value_types,
            defined,
            ScalarCallKind::Ordinary,
        )?;
        return Ok(());
    }
    if let OperationKind::BoundaryCall {
        boundary,
        arguments,
        ..
    } = &operation.kind
    {
        let boundary = boundary_machines
            .iter()
            .find(|candidate| candidate.id == *boundary)
            .expect("boundary target was validated during operation registration");
        validate_call_arguments(
            operation.id,
            arguments,
            &boundary.scalar_parameters,
            value_types,
            defined,
            ScalarCallKind::Boundary,
        )?;
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
        | OperationKind::IeeeFloatConstant { .. }
        | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::IntegerStructuralField { .. }
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
        | OperationKind::WriteOnlyPrimitiveStore { .. }
        | OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. }
        | OperationKind::EstablishPayloadlessCase { .. }
        | OperationKind::BoundaryCall { .. }
        | OperationKind::PortWrite { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
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

#[derive(Clone, Copy)]
enum ScalarCallKind {
    Ordinary,
    Boundary,
}

fn validate_call_arguments(
    operation: OperationId,
    arguments: &[ValueId],
    parameter_types: &[ScalarType],
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
    kind: ScalarCallKind,
) -> Result<(), ModuleError> {
    if arguments.len() != parameter_types.len() {
        return Err(match kind {
            ScalarCallKind::Ordinary => ModuleError::CallArgumentArityMismatch {
                operation,
                expected: parameter_types.len(),
                actual: arguments.len(),
            },
            ScalarCallKind::Boundary => ModuleError::BoundaryCallArgumentArityMismatch {
                operation,
                expected: parameter_types.len(),
                actual: arguments.len(),
            },
        });
    }
    for (argument, expected) in arguments.iter().zip(parameter_types) {
        if let Err(error) = require_defined(*argument, value_types, defined) {
            return Err(match (kind, error) {
                (ScalarCallKind::Boundary, ModuleError::UnknownValue(_)) => {
                    ModuleError::UnknownBoundaryCallArgument {
                        operation,
                        argument: *argument,
                    }
                }
                (ScalarCallKind::Boundary, ModuleError::ValueUsedBeforeDefinition(_))
                    if !value_types.contains_key(argument) =>
                {
                    ModuleError::UnknownBoundaryCallArgument {
                        operation,
                        argument: *argument,
                    }
                }
                (ScalarCallKind::Boundary, ModuleError::ValueUsedBeforeDefinition(_)) => {
                    ModuleError::BoundaryCallArgumentUsedBeforeDefinition {
                        operation,
                        argument: *argument,
                    }
                }
                (_, error) => error,
            });
        }
        let actual = value_types[argument];
        if actual != *expected {
            return Err(match kind {
                ScalarCallKind::Ordinary => ModuleError::CallArgumentTypeMismatch {
                    operation,
                    argument: *argument,
                    expected: *expected,
                    actual,
                },
                ScalarCallKind::Boundary => ModuleError::BoundaryCallArgumentTypeMismatch {
                    operation,
                    argument: *argument,
                    expected: *expected,
                    actual,
                },
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
