//! Multiplicative and divisor-chain sufficient forms.

use super::*;

pub(super) fn shared_exact_multiply_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_multiply = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactMultiply,
            primitive_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type)
            || !nonnegative_exact_multiply_literal(*primitive_type, right)
        {
            return None;
        }
        chain_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactMultiply,
                ..
            } => {
                saw_nested_multiply = true;
                expression = nested;
            }
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if saw_nested_multiply
                && *root_type == *primitive_type
                && *position < scalar_parameter_count =>
            {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_signed_multiply_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut product = Some((false, 1_u128));
    let mut saw_nested_multiply = false;
    let mut saw_negative = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactMultiply,
            primitive_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        let factor = signed_exact_multiply_literal_value(*primitive_type, right)?;
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type) {
            return None;
        }
        product = checked_signed_product(product, factor);
        saw_negative |= factor < 0;
        chain_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactMultiply,
                ..
            } => {
                saw_nested_multiply = true;
                expression = nested;
            }
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if product.is_some()
                && saw_nested_multiply
                && saw_negative
                && *root_type == *primitive_type
                && *position < scalar_parameter_count =>
            {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_then_multiply_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactMultiply,
            primitive_type: target_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *target_type)
            || !nonnegative_exact_multiply_literal(*target_type, right)
        {
            return None;
        }
        chain_type = Some(*target_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactMultiply,
                ..
            } => expression = nested,
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if *cast_target_type == *target_type => {
                let CheckedScalarExpression::Parameter {
                    position,
                    primitive_type: source_type,
                } = operand.as_ref()
                else {
                    return None;
                };
                return (matches!(
                    source_type,
                    PrimitiveType::I8
                        | PrimitiveType::I16
                        | PrimitiveType::I32
                        | PrimitiveType::I64
                        | PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                ) && *position < scalar_parameter_count)
                    .then(|| {
                        BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)])
                    });
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_then_signed_multiply_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut product = Some((false, 1_u128));
    let mut saw_negative = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactMultiply,
            primitive_type: target_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        let factor = signed_exact_multiply_literal_value(*target_type, right)?;
        if chain_type.is_some_and(|chain_type| chain_type != *target_type) {
            return None;
        }
        product = checked_signed_product(product, factor);
        saw_negative |= factor < 0;
        chain_type = Some(*target_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactMultiply,
                ..
            } => expression = nested,
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if product.is_some() && saw_negative && *cast_target_type == *target_type => {
                let CheckedScalarExpression::Parameter {
                    position,
                    primitive_type: source_type,
                } = operand.as_ref()
                else {
                    return None;
                };
                return (matches!(
                    source_type,
                    PrimitiveType::I8
                        | PrimitiveType::I16
                        | PrimitiveType::I32
                        | PrimitiveType::I64
                        | PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                ) && *position < scalar_parameter_count)
                    .then(|| {
                        BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)])
                    });
            }
            _ => return None,
        }
    }
}

pub(super) fn signed_exact_multiply_literal_value(
    primitive_type: PrimitiveType,
    expression: &CheckedScalarExpression,
) -> Option<i64> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    match (
        primitive_type,
        literal.landing().map(|landing| landing.landed_type),
    ) {
        (PrimitiveType::I8, Some(numerics::literals::LandedIntegerType::I8))
        | (PrimitiveType::I16, Some(numerics::literals::LandedIntegerType::I16))
        | (PrimitiveType::I32, Some(numerics::literals::LandedIntegerType::I32))
        | (PrimitiveType::I64, Some(numerics::literals::LandedIntegerType::I64)) => {
            literal.value_i64()
        }
        _ => None,
    }
}

pub(super) fn checked_signed_product(
    product: Option<(bool, u128)>,
    factor: i64,
) -> Option<(bool, u128)> {
    let magnitude = u128::from(factor.unsigned_abs());
    if magnitude == 0 {
        return Some((false, 0));
    }
    let product = product?;
    if product.1 == 0 {
        return Some((false, 0));
    }
    Some((product.0 ^ (factor < 0), product.1.checked_mul(magnitude)?))
}

#[cfg(test)]
pub(crate) fn exact_signed_multiply_chain_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_signed_multiply_chain_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_signed_multiply_chain_cast_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_signed_multiply_chain_cast_runtime_inputs(
        target_type,
        expression,
        scalar_parameter_count,
    )?
    .into_iter()
    .map(|input| match input {
        SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
        _ => None,
    })
    .collect()
}

#[cfg(test)]
pub(crate) fn exact_cast_then_signed_multiply_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_then_signed_multiply_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn nonnegative_exact_multiply_literal(
    primitive_type: PrimitiveType,
    expression: &CheckedScalarExpression,
) -> bool {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return false;
    };
    match (
        primitive_type,
        literal.landing().map(|landing| landing.landed_type),
    ) {
        (PrimitiveType::I8, Some(numerics::literals::LandedIntegerType::I8))
        | (PrimitiveType::I16, Some(numerics::literals::LandedIntegerType::I16))
        | (PrimitiveType::I32, Some(numerics::literals::LandedIntegerType::I32))
        | (PrimitiveType::I64, Some(numerics::literals::LandedIntegerType::I64)) => {
            literal.value_i64().is_some_and(|value| value >= 0)
        }
        (PrimitiveType::U8, Some(numerics::literals::LandedIntegerType::U8))
        | (PrimitiveType::U16, Some(numerics::literals::LandedIntegerType::U16))
        | (PrimitiveType::U32, Some(numerics::literals::LandedIntegerType::U32))
        | (PrimitiveType::U64, Some(numerics::literals::LandedIntegerType::U64)) => {
            literal.value_u64().is_some()
        }
        _ => false,
    }
}

#[cfg(test)]
pub(crate) fn exact_cast_then_multiply_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_then_multiply_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn shared_exact_divide_remainder_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_operation = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder,
            primitive_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type)
            || !safe_exact_divide_remainder_literal(*primitive_type, right)
        {
            return None;
        }
        chain_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder,
                ..
            } => {
                saw_nested_operation = true;
                expression = nested;
            }
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if saw_nested_operation
                && *root_type == *primitive_type
                && *position < scalar_parameter_count =>
            {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_runtime_divisor_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut inputs = BTreeSet::new();
    let mut saw_nested_operation = false;
    let mut saw_runtime_divisor = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder,
            primitive_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if fixed_native_primitive_interval(*primitive_type).is_none()
            || chain_type.is_some_and(|chain_type| chain_type != *primitive_type)
        {
            return None;
        }
        chain_type = Some(*primitive_type);
        if !safe_exact_divide_remainder_literal(*primitive_type, right) {
            let CheckedScalarExpression::Parameter {
                position,
                primitive_type: divisor_type,
            } = right.as_ref()
            else {
                return None;
            };
            if *divisor_type != *primitive_type || *position >= scalar_parameter_count {
                return None;
            }
            inputs.insert(SharedBooleanRuntimeInput::IntegerScalar(*position));
            saw_runtime_divisor = true;
        }
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder,
                ..
            } => {
                saw_nested_operation = true;
                expression = nested;
            }
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if saw_nested_operation
                && saw_runtime_divisor
                && *root_type == *primitive_type
                && *position < scalar_parameter_count =>
            {
                inputs.insert(SharedBooleanRuntimeInput::IntegerScalar(*position));
                return Some(inputs);
            }
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if saw_runtime_divisor && *cast_target_type == *primitive_type => {
                let CheckedScalarExpression::Parameter {
                    position,
                    primitive_type: source_type,
                } = operand.as_ref()
                else {
                    return None;
                };
                let source_interval = fixed_native_primitive_interval(*source_type)?;
                let target_interval = fixed_native_primitive_interval(*cast_target_type)?;
                if (source_interval.0 >= target_interval.0
                    && source_interval.1 <= target_interval.1)
                    || *position >= scalar_parameter_count
                {
                    return None;
                }
                inputs.insert(SharedBooleanRuntimeInput::IntegerScalar(*position));
                return Some(inputs);
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
pub(crate) fn exact_runtime_divisor_chain_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_runtime_divisor_chain_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn shared_exact_cast_then_divide_remainder_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder,
            primitive_type: target_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *target_type)
            || !safe_exact_divide_remainder_literal(*target_type, right)
        {
            return None;
        }
        chain_type = Some(*target_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder,
                ..
            } => expression = nested,
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if *cast_target_type == *target_type => {
                let CheckedScalarExpression::Parameter {
                    position,
                    primitive_type: source_type,
                } = operand.as_ref()
                else {
                    return None;
                };
                return (matches!(
                    source_type,
                    PrimitiveType::I8
                        | PrimitiveType::I16
                        | PrimitiveType::I32
                        | PrimitiveType::I64
                        | PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                ) && *position < scalar_parameter_count)
                    .then(|| {
                        BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)])
                    });
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
pub(crate) fn exact_cast_then_divide_remainder_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_then_divide_remainder_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn safe_exact_divide_remainder_literal(
    primitive_type: PrimitiveType,
    expression: &CheckedScalarExpression,
) -> bool {
    landed_safe_exact_divide_remainder_literal(primitive_type, expression).is_some()
}
