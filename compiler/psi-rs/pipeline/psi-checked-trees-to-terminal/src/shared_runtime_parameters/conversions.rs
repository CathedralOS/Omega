//! Exact-cast and mixed conversion-spine runtime-input classifiers.

use super::*;

pub(super) fn shared_roundtrip_exact_cast_runtime_parameters(
    target_type: ScalarType,
    operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut operand = operand;
    let mut saw_widen = false;
    while let LoweredDirectExpression::IntegerWiden { operand: inner, .. } = operand {
        saw_widen = true;
        operand = inner;
    }
    let LoweredDirectExpression::Parameter {
        position,
        scalar_type,
    } = operand
    else {
        return None;
    };
    (saw_widen && *scalar_type == target_type)
        .then(|| BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)]))
}

pub(super) fn shared_exact_cast_chain_runtime_parameters(
    target_type: ScalarType,
    mut operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_type else {
        return None;
    };
    fixed_native_integer_interval(target_type)?;
    let mut expected_target = target_type;
    let mut followed_nested_cast = false;
    loop {
        match operand {
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target),
                operand: cast_operand,
            } if partial_fixed_native_integer_cast(*cast_target, expected_target) => {
                expected_target = *cast_target;
                operand = cast_operand;
                followed_nested_cast = true;
            }
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if followed_nested_cast
                && partial_fixed_native_integer_cast(*root_type, expected_target) =>
            {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn partial_fixed_native_integer_cast(source: IntegerType, target: IntegerType) -> bool {
    fixed_native_integer_interval(source).is_some()
        && fixed_native_integer_interval(target).is_some()
        && source != target
        && source.can_exact_cast_to(target)
        && !source.can_widen_to(target)
}

pub(super) fn shared_exact_cast_chain_then_computed_suffix_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_parameters(
        expression,
        shared_exact_cast_chain_suffix_root_runtime_parameters,
    )
}

pub(super) fn shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_parameters(
        expression,
        shared_exact_computed_prefix_cast_chain_suffix_root_runtime_parameters,
    )
}

pub(super) fn shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_parameters(
        expression,
        shared_exact_computed_prefix_widen_chain_suffix_root_runtime_parameters,
    )
}

pub(super) fn shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_parameters(
        expression,
        shared_exact_computed_prefix_mixed_conversion_chain_suffix_root_runtime_parameters,
    )
}

type SharedExactComputedSuffixRootRuntimeParameters =
    fn(IntegerType, &LoweredDirectExpression) -> Option<BTreeSet<SharedBooleanRuntimeInput>>;

pub(super) fn shared_exact_computed_suffix_runtime_parameters(
    expression: &LoweredDirectExpression,
    root_runtime_parameters: SharedExactComputedSuffixRootRuntimeParameters,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_cast_chain_then_affine_runtime_parameters(expression, root_runtime_parameters)
        .or_else(|| {
            shared_exact_cast_chain_then_signed_product_runtime_parameters(
                expression,
                root_runtime_parameters,
            )
        })
        .or_else(|| {
            shared_exact_cast_chain_then_shift_runtime_parameters(
                expression,
                root_runtime_parameters,
            )
        })
        .or_else(|| {
            shared_exact_cast_chain_then_divide_remainder_runtime_parameters(
                expression,
                root_runtime_parameters,
            )
        })
}

pub(super) fn shared_exact_cast_chain_suffix_root_runtime_parameters(
    target_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let LoweredDirectExpression::IntegerExactCast {
        scalar_type: ScalarType::Integer(cast_target_type),
        operand,
    } = expression
    else {
        return None;
    };
    (*cast_target_type == target_type).then_some(())?;
    shared_exact_cast_chain_runtime_parameters(ScalarType::Integer(target_type), operand)
}

pub(super) fn shared_exact_computed_prefix_cast_chain_suffix_root_runtime_parameters(
    target_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let LoweredDirectExpression::IntegerExactCast {
        scalar_type: ScalarType::Integer(cast_target_type),
        operand,
    } = expression
    else {
        return None;
    };
    (*cast_target_type == target_type).then_some(())?;
    shared_exact_computed_prefix_cast_chain_runtime_parameters(
        ScalarType::Integer(target_type),
        operand,
    )
}

pub(super) fn shared_exact_computed_prefix_widen_chain_suffix_root_runtime_parameters(
    target_type: IntegerType,
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut expected_target = target_type;
    let mut saw_widen = false;
    while let LoweredDirectExpression::IntegerWiden {
        scalar_type: ScalarType::Integer(widen_target),
        operand,
    } = expression
    {
        let ScalarType::Integer(source_type) = operand.scalar_type() else {
            return None;
        };
        if *widen_target != expected_target || !source_type.can_widen_to(*widen_target) {
            return None;
        }
        expected_target = source_type;
        expression = operand;
        saw_widen = true;
    }
    saw_widen.then_some(())?;
    shared_exact_computed_suffix_runtime_parameters(
        expression,
        shared_exact_direct_parameter_suffix_root_runtime_parameters,
    )
}

pub(super) fn shared_exact_computed_prefix_mixed_conversion_chain_suffix_root_runtime_parameters(
    target_type: IntegerType,
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut expected_target = target_type;
    let mut saw_widen = false;
    let mut saw_cast = false;
    loop {
        match expression {
            LoweredDirectExpression::IntegerWiden {
                scalar_type: ScalarType::Integer(conversion_target),
                operand,
            } => {
                let ScalarType::Integer(source_type) = operand.scalar_type() else {
                    return None;
                };
                if *conversion_target != expected_target
                    || !source_type.can_widen_to(*conversion_target)
                {
                    return None;
                }
                expected_target = source_type;
                expression = operand;
                saw_widen = true;
            }
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(conversion_target),
                operand,
            } => {
                let ScalarType::Integer(source_type) = operand.scalar_type() else {
                    return None;
                };
                if *conversion_target != expected_target
                    || !partial_fixed_native_integer_cast(source_type, *conversion_target)
                {
                    return None;
                }
                expected_target = source_type;
                expression = operand;
                saw_cast = true;
            }
            _ => break,
        }
    }
    (saw_widen && saw_cast).then_some(())?;
    shared_exact_computed_suffix_runtime_parameters(
        expression,
        shared_exact_direct_parameter_suffix_root_runtime_parameters,
    )
}

pub(super) fn shared_exact_direct_parameter_suffix_root_runtime_parameters(
    target_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let LoweredDirectExpression::Parameter {
        position,
        scalar_type: ScalarType::Integer(parameter_type),
    } = expression
    else {
        return None;
    };
    (*parameter_type == target_type)
        .then(|| BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)]))
}

pub(super) fn shared_exact_cast_chain_then_affine_runtime_parameters(
    mut expression: &LoweredDirectExpression,
    root_runtime_parameters: SharedExactComputedSuffixRootRuntimeParameters,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind:
                kind @ (LoweredIntegerBinaryKind::ExactAdd
                | LoweredIntegerBinaryKind::ExactSubtract
                | LoweredIntegerBinaryKind::ExactMultiply),
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        if !native_fixed_integer_type(*integer_type)
            || chain_type.is_some_and(|chain_type| chain_type != *integer_type)
            || match kind {
                LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract => {
                    !exact_offset_landed_literal(*integer_type, right)
                }
                LoweredIntegerBinaryKind::ExactMultiply => {
                    !nonnegative_exact_multiply_landed_literal(*integer_type, right)
                }
                _ => unreachable!("matched one exact affine operation"),
            }
        {
            return None;
        }
        chain_type = Some(*integer_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactAdd
                    | LoweredIntegerBinaryKind::ExactSubtract
                    | LoweredIntegerBinaryKind::ExactMultiply,
                ..
            } => expression = nested,
            root => {
                return root_runtime_parameters(*integer_type, root);
            }
        }
    }
}

pub(super) fn shared_exact_cast_chain_then_signed_product_runtime_parameters(
    mut expression: &LoweredDirectExpression,
    root_runtime_parameters: SharedExactComputedSuffixRootRuntimeParameters,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut product = Some((false, 1_u128));
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
            } => expression = nested,
            root if product.is_some() && saw_negative => {
                return root_runtime_parameters(*integer_type, root);
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_chain_then_shift_runtime_parameters(
    mut expression: &LoweredDirectExpression,
    root_runtime_parameters: SharedExactComputedSuffixRootRuntimeParameters,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind:
                LoweredIntegerBinaryKind::ExactShiftLeft | LoweredIntegerBinaryKind::ExactShiftRight,
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        if !native_fixed_integer_type(*integer_type)
            || chain_type.is_some_and(|chain_type| chain_type != *integer_type)
            || landed_exact_shift_literal_count(*integer_type, right).is_none()
        {
            return None;
        }
        chain_type = Some(*integer_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactShiftLeft | LoweredIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            root => {
                return root_runtime_parameters(*integer_type, root);
            }
        }
    }
}

pub(super) fn shared_exact_cast_chain_then_divide_remainder_runtime_parameters(
    mut expression: &LoweredDirectExpression,
    root_runtime_parameters: SharedExactComputedSuffixRootRuntimeParameters,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
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
            || landed_safe_exact_divide_remainder_value(*integer_type, right).is_none()
        {
            return None;
        }
        chain_type = Some(*integer_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
                ..
            } => expression = nested,
            root => {
                return root_runtime_parameters(*integer_type, root);
            }
        }
    }
}

pub(super) fn shared_exact_computed_prefix_cast_chain_runtime_parameters(
    target_scalar_type: ScalarType,
    mut operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_scalar_type else {
        return None;
    };
    fixed_native_integer_interval(target_type)?;
    let mut expected_target = target_type;
    let mut followed_nested_cast = false;
    while let LoweredDirectExpression::IntegerExactCast {
        scalar_type: ScalarType::Integer(cast_target),
        operand: cast_operand,
    } = operand
    {
        if !partial_fixed_native_integer_cast(*cast_target, expected_target) {
            return None;
        }
        expected_target = *cast_target;
        operand = cast_operand;
        followed_nested_cast = true;
    }
    followed_nested_cast.then_some(())?;
    let ScalarType::Integer(source_type) = operand.scalar_type() else {
        return None;
    };
    partial_fixed_native_integer_cast(source_type, expected_target).then_some(())?;
    let expected_target = ScalarType::Integer(expected_target);
    shared_exact_divide_remainder_chain_cast_runtime_parameters(expected_target, operand)
        .or_else(|| {
            shared_exact_mixed_shift_chain_cast_runtime_parameters(expected_target, operand)
        })
        .or_else(|| {
            shared_exact_shift_right_chain_cast_runtime_parameters(expected_target, operand)
        })
        .or_else(|| shared_exact_shift_left_chain_cast_runtime_parameters(expected_target, operand))
        .or_else(|| shared_exact_affine_chain_cast_runtime_parameters(expected_target, operand))
        .or_else(|| shared_exact_multiply_chain_cast_runtime_parameters(expected_target, operand))
        .or_else(|| {
            shared_exact_signed_multiply_chain_cast_runtime_parameters(expected_target, operand)
        })
        .or_else(|| shared_exact_offset_chain_cast_runtime_parameters(expected_target, operand))
}

#[derive(Clone, Copy)]
pub(super) enum ExactDivideRemainderTransfer {
    Divide,
    Remainder,
}

pub(super) fn shared_exact_divide_remainder_chain_cast_runtime_parameters(
    target_scalar_type: ScalarType,
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_scalar_type else {
        return None;
    };
    let target_interval = fixed_native_integer_interval(target_type)?;
    let mut source_type = None;
    let mut transfers = Vec::new();
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind:
                kind
                @ (LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder),
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        if source_type.is_some_and(|source_type| source_type != *integer_type) {
            return None;
        }
        source_type = Some(*integer_type);
        let divisor = landed_safe_exact_divide_remainder_value(*integer_type, right)?;
        transfers.push((
            match kind {
                LoweredIntegerBinaryKind::ExactDivide => ExactDivideRemainderTransfer::Divide,
                LoweredIntegerBinaryKind::ExactRemainder => ExactDivideRemainderTransfer::Remainder,
                _ => unreachable!("matched one exact divide/remainder operation"),
            },
            divisor,
        ));
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
                ..
            } => expression = nested,
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == *integer_type => {
                let source_interval = fixed_native_integer_interval(*root_type)?;
                if source_interval.0 >= target_interval.0 && source_interval.1 <= target_interval.1
                {
                    return None;
                }
                let final_interval = transfers.into_iter().rev().try_fold(
                    source_interval,
                    |interval, (transfer, divisor)| {
                        exact_divide_remainder_interval_transfer(interval, transfer, divisor)
                    },
                )?;
                return (final_interval.0 >= target_interval.0
                    && final_interval.1 <= target_interval.1)
                    .then(|| {
                        BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)])
                    });
            }
            _ => return None,
        }
    }
}

pub(super) fn fixed_native_integer_interval(integer_type: IntegerType) -> Option<(i128, i128)> {
    if !native_fixed_integer_type(integer_type) {
        return None;
    }
    let minimum = match integer_type.minimum_value() {
        IntegerValue::Signed(value) => value,
        IntegerValue::Unsigned(value) => i128::try_from(value).ok()?,
    };
    let maximum = match integer_type.maximum_value() {
        IntegerValue::Signed(value) => value,
        IntegerValue::Unsigned(value) => i128::try_from(value).ok()?,
    };
    Some((minimum, maximum))
}

pub(super) fn exact_divide_remainder_interval_transfer(
    (minimum, maximum): (i128, i128),
    transfer: ExactDivideRemainderTransfer,
    divisor: i128,
) -> Option<(i128, i128)> {
    if divisor == 0 || divisor == -1 {
        return None;
    }
    match transfer {
        ExactDivideRemainderTransfer::Divide if divisor > 0 => {
            Some((minimum / divisor, maximum / divisor))
        }
        ExactDivideRemainderTransfer::Divide => Some((maximum / divisor, minimum / divisor)),
        ExactDivideRemainderTransfer::Remainder => {
            let magnitude = divisor.checked_abs()?;
            let remainder_maximum = magnitude.checked_sub(1)?;
            if minimum >= 0 {
                Some((0, maximum.min(remainder_maximum)))
            } else if maximum <= 0 {
                Some((minimum.max(-remainder_maximum), 0))
            } else {
                Some((
                    minimum.max(-remainder_maximum),
                    maximum.min(remainder_maximum),
                ))
            }
        }
    }
}

pub(super) fn landed_safe_exact_divide_remainder_value(
    integer_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> Option<i128> {
    let LoweredDirectExpression::IntegerLiteral { value, scalar_type } = expression else {
        return None;
    };
    if *scalar_type != ScalarType::Integer(integer_type) {
        return None;
    }
    let value = match (integer_type.sign(), value) {
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => i128::try_from(*value).ok()?,
        (IntegerSign::Signed, IntegerValue::Signed(value)) => *value,
        _ => return None,
    };
    (value != 0 && value != -1).then_some(value)
}

pub(super) fn shared_exact_offset_chain_cast_runtime_parameters(
    target_type: ScalarType,
    mut operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_type else {
        return None;
    };
    if !native_fixed_integer_type(target_type) {
        return None;
    }
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract,
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = operand
        else {
            return None;
        };
        if !native_fixed_integer_type(*integer_type)
            || chain_type.is_some_and(|chain_type| chain_type != *integer_type)
            || !exact_offset_landed_literal(*integer_type, right)
        {
            return None;
        }
        chain_type = Some(*integer_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract,
                ..
            } => operand = nested,
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == *integer_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_multiply_chain_cast_runtime_parameters(
    target_type: ScalarType,
    mut operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_type else {
        return None;
    };
    if !native_fixed_integer_type(target_type) {
        return None;
    }
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactMultiply,
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = operand
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
            } => operand = nested,
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == *integer_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_signed_multiply_chain_cast_runtime_parameters(
    target_type: ScalarType,
    mut operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_type else {
        return None;
    };
    if !native_fixed_integer_type(target_type) {
        return None;
    }
    let mut chain_type = None;
    let mut product = Some((false, 1_u128));
    let mut saw_negative = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactMultiply,
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = operand
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
            } => operand = nested,
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if product.is_some() && saw_negative && *root_type == *integer_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_affine_chain_cast_runtime_parameters(
    target_type: ScalarType,
    operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_type else {
        return None;
    };
    if !native_fixed_integer_type(target_type) {
        return None;
    }
    shared_exact_affine_chain_runtime_parameters(operand)
}

pub(super) fn shared_exact_shift_left_chain_cast_runtime_parameters(
    target_type: ScalarType,
    mut operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_type else {
        return None;
    };
    if !native_fixed_integer_type(target_type) {
        return None;
    }
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactShiftLeft,
            scalar_type: ScalarType::Integer(value_type),
            left,
            right,
        } = operand
        else {
            return None;
        };
        if !native_fixed_integer_type(*value_type)
            || chain_type.is_some_and(|chain_type| chain_type != *value_type)
            || !safe_exact_shift_landed_literal_count(*value_type, right)
        {
            return None;
        }
        chain_type = Some(*value_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactShiftLeft,
                ..
            } => operand = nested,
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == *value_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_mixed_shift_chain_cast_runtime_parameters(
    target_type: ScalarType,
    operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_type else {
        return None;
    };
    if !native_fixed_integer_type(target_type) {
        return None;
    }
    shared_exact_mixed_shift_chain_runtime_parameters(operand)
}

pub(super) fn shared_exact_shift_right_chain_cast_runtime_parameters(
    target_type: ScalarType,
    mut operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_type else {
        return None;
    };
    if !native_fixed_integer_type(target_type) {
        return None;
    }
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactShiftRight,
            scalar_type: ScalarType::Integer(value_type),
            left,
            right,
        } = operand
        else {
            return None;
        };
        if !native_fixed_integer_type(*value_type)
            || chain_type.is_some_and(|chain_type| chain_type != *value_type)
            || !safe_exact_shift_landed_literal_count(*value_type, right)
        {
            return None;
        }
        chain_type = Some(*value_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactShiftRight,
                ..
            } => operand = nested,
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == *value_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}
