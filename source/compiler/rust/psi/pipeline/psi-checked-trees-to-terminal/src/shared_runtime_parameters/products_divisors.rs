//! Exact product, divisor, and divide/remainder runtime-input classifiers.

use super::*;

pub(super) fn shared_exact_multiply_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_multiply = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactMultiply,
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        if !native_fixed_integer_type(*integer_type)
            || chain_type.is_some_and(|chain_type| chain_type != *integer_type)
            || !nonnegative_exact_multiply_landed_literal(*integer_type, right)
        {
            return None;
        }
        chain_type = Some(*integer_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactMultiply,
                ..
            } => {
                saw_nested_multiply = true;
                expression = nested;
            }
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if saw_nested_multiply && *root_type == *integer_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_signed_multiply_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut product = Some((false, 1_u128));
    let mut saw_nested_multiply = false;
    let mut saw_negative = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactMultiply,
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        let factor = signed_exact_multiply_landed_value(*integer_type, right)?;
        if !native_fixed_integer_type(*integer_type)
            || chain_type.is_some_and(|chain_type| chain_type != *integer_type)
        {
            return None;
        }
        product = checked_lowered_signed_product(product, factor);
        saw_negative |= factor < 0;
        chain_type = Some(*integer_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactMultiply,
                ..
            } => {
                saw_nested_multiply = true;
                expression = nested;
            }
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if product.is_some()
                && saw_nested_multiply
                && saw_negative
                && *root_type == *integer_type =>
            {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_then_multiply_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactMultiply,
            scalar_type: ScalarType::Integer(target_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        if !native_fixed_integer_type(*target_type)
            || chain_type.is_some_and(|chain_type| chain_type != *target_type)
            || !nonnegative_exact_multiply_landed_literal(*target_type, right)
        {
            return None;
        }
        chain_type = Some(*target_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactMultiply,
                ..
            } => expression = nested,
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if *cast_target_type == *target_type => {
                let LoweredDirectExpression::Parameter {
                    position,
                    scalar_type: ScalarType::Integer(source_type),
                } = operand.as_ref()
                else {
                    return None;
                };
                return native_fixed_integer_type(*source_type).then(|| {
                    BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)])
                });
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_then_signed_multiply_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut product = Some((false, 1_u128));
    let mut saw_negative = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactMultiply,
            scalar_type: ScalarType::Integer(target_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        let factor = signed_exact_multiply_landed_value(*target_type, right)?;
        if !native_fixed_integer_type(*target_type)
            || chain_type.is_some_and(|chain_type| chain_type != *target_type)
        {
            return None;
        }
        product = checked_lowered_signed_product(product, factor);
        saw_negative |= factor < 0;
        chain_type = Some(*target_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactMultiply,
                ..
            } => expression = nested,
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if product.is_some() && saw_negative && *cast_target_type == *target_type => {
                let LoweredDirectExpression::Parameter {
                    position,
                    scalar_type: ScalarType::Integer(source_type),
                } = operand.as_ref()
                else {
                    return None;
                };
                return native_fixed_integer_type(*source_type).then(|| {
                    BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)])
                });
            }
            _ => return None,
        }
    }
}

pub(super) fn signed_exact_multiply_landed_value(
    integer_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> Option<i128> {
    let LoweredDirectExpression::IntegerLiteral { value, scalar_type } = expression else {
        return None;
    };
    if integer_type.sign() != IntegerSign::Signed
        || *scalar_type != ScalarType::Integer(integer_type)
    {
        return None;
    }
    let IntegerValue::Signed(value) = value else {
        return None;
    };
    Some(*value)
}

pub(super) fn checked_lowered_signed_product(
    product: Option<(bool, u128)>,
    factor: i128,
) -> Option<(bool, u128)> {
    let magnitude = factor.unsigned_abs();
    if magnitude == 0 {
        return Some((false, 0));
    }
    let product = product?;
    if product.1 == 0 {
        return Some((false, 0));
    }
    Some((product.0 ^ (factor < 0), product.1.checked_mul(magnitude)?))
}

pub(super) fn nonnegative_exact_multiply_landed_literal(
    integer_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> bool {
    let LoweredDirectExpression::IntegerLiteral { value, scalar_type } = expression else {
        return false;
    };
    if *scalar_type != ScalarType::Integer(integer_type) {
        return false;
    }
    match (integer_type.sign(), value) {
        (IntegerSign::Unsigned, IntegerValue::Unsigned(_)) => true,
        (IntegerSign::Signed, IntegerValue::Signed(value)) => *value >= 0,
        _ => false,
    }
}

pub(super) fn shared_exact_divide_remainder_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_operation = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        if integer_type.is_address()
            || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
            || chain_type.is_some_and(|chain_type| chain_type != *integer_type)
            || !safe_exact_divide_remainder_landed_literal(*integer_type, right)
        {
            return None;
        }
        chain_type = Some(*integer_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
                ..
            } => {
                saw_nested_operation = true;
                expression = nested;
            }
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if saw_nested_operation && *root_type == *integer_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_runtime_divisor_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut parameters = BTreeSet::new();
    let mut saw_nested_operation = false;
    let mut saw_runtime_divisor = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        if !native_fixed_integer_type(*integer_type)
            || chain_type.is_some_and(|chain_type| chain_type != *integer_type)
        {
            return None;
        }
        chain_type = Some(*integer_type);
        if !safe_exact_divide_remainder_landed_literal(*integer_type, right) {
            let LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(divisor_type),
            } = right.as_ref()
            else {
                return None;
            };
            if *divisor_type != *integer_type {
                return None;
            }
            parameters.insert(SharedBooleanRuntimeInput::IntegerScalar(*position));
            saw_runtime_divisor = true;
        }
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
                ..
            } => {
                saw_nested_operation = true;
                expression = nested;
            }
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if saw_nested_operation && saw_runtime_divisor && *root_type == *integer_type => {
                parameters.insert(SharedBooleanRuntimeInput::IntegerScalar(*position));
                return Some(parameters);
            }
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if saw_runtime_divisor && *cast_target_type == *integer_type => {
                let LoweredDirectExpression::Parameter {
                    position,
                    scalar_type: ScalarType::Integer(source_type),
                } = operand.as_ref()
                else {
                    return None;
                };
                let source_interval = fixed_native_integer_interval(*source_type)?;
                let target_interval = fixed_native_integer_interval(*cast_target_type)?;
                if source_interval.0 >= target_interval.0 && source_interval.1 <= target_interval.1
                {
                    return None;
                }
                parameters.insert(SharedBooleanRuntimeInput::IntegerScalar(*position));
                return Some(parameters);
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_then_divide_remainder_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
            scalar_type: ScalarType::Integer(target_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        if !native_fixed_integer_type(*target_type)
            || chain_type.is_some_and(|chain_type| chain_type != *target_type)
            || !safe_exact_divide_remainder_landed_literal(*target_type, right)
        {
            return None;
        }
        chain_type = Some(*target_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
                ..
            } => expression = nested,
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if *cast_target_type == *target_type => {
                let LoweredDirectExpression::Parameter {
                    position,
                    scalar_type: ScalarType::Integer(source_type),
                } = operand.as_ref()
                else {
                    return None;
                };
                return native_fixed_integer_type(*source_type).then(|| {
                    BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)])
                });
            }
            _ => return None,
        }
    }
}

pub(super) fn safe_exact_divide_remainder_landed_literal(
    integer_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> bool {
    landed_safe_exact_divide_remainder_value(integer_type, expression).is_some()
}
