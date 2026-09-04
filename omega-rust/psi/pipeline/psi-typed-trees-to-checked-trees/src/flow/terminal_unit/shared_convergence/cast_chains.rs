//! Exact-cast chains and cast-boundary sufficient forms.

use super::*;

pub(super) fn shared_roundtrip_exact_cast_runtime_inputs(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut operand = operand;
    let mut saw_widen = false;
    while let CheckedScalarExpression::IntegerWiden { operand: inner, .. } = operand {
        saw_widen = true;
        operand = inner;
    }
    let CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    } = operand
    else {
        return None;
    };
    (saw_widen && *primitive_type == target_type && *position < scalar_parameter_count)
        .then(|| BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)]))
}

pub(super) fn shared_exact_cast_chain_runtime_inputs(
    target_type: PrimitiveType,
    mut operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    fixed_native_primitive_interval(target_type)?;
    let mut expected_target = target_type;
    let mut followed_nested_cast = false;
    loop {
        match operand {
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target,
                operand: cast_operand,
                ..
            } if partial_fixed_native_primitive_cast(*cast_target, expected_target) => {
                expected_target = *cast_target;
                operand = cast_operand;
                followed_nested_cast = true;
            }
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if followed_nested_cast
                && partial_fixed_native_primitive_cast(*root_type, expected_target)
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

pub(super) fn partial_fixed_native_primitive_cast(
    source: PrimitiveType,
    target: PrimitiveType,
) -> bool {
    let Some(source_interval) = fixed_native_primitive_interval(source) else {
        return false;
    };
    let Some(target_interval) = fixed_native_primitive_interval(target) else {
        return false;
    };
    source != target
        && !(target_interval.0 <= source_interval.0 && source_interval.1 <= target_interval.1)
}

pub(super) fn shared_exact_cast_chain_then_computed_suffix_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_inputs(
        expression,
        scalar_parameter_count,
        shared_exact_cast_chain_suffix_root_runtime_inputs,
    )
}

pub(super) fn shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_inputs(
        expression,
        scalar_parameter_count,
        shared_exact_computed_prefix_cast_chain_suffix_root_runtime_inputs,
    )
}

pub(super) fn shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_inputs(
        expression,
        scalar_parameter_count,
        shared_exact_computed_prefix_widen_chain_suffix_root_runtime_inputs,
    )
}

pub(super) fn shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_inputs(
        expression,
        scalar_parameter_count,
        shared_exact_computed_prefix_mixed_conversion_chain_suffix_root_runtime_inputs,
    )
}

type SharedExactComputedSuffixRootRuntimeInputs = fn(
    PrimitiveType,
    &CheckedScalarExpression,
    usize,
)
    -> Option<BTreeSet<SharedBooleanRuntimeInput>>;

pub(super) fn shared_exact_computed_suffix_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    root_runtime_inputs: SharedExactComputedSuffixRootRuntimeInputs,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_cast_chain_then_affine_runtime_inputs(
        expression,
        scalar_parameter_count,
        root_runtime_inputs,
    )
    .or_else(|| {
        shared_exact_cast_chain_then_signed_product_runtime_inputs(
            expression,
            scalar_parameter_count,
            root_runtime_inputs,
        )
    })
    .or_else(|| {
        shared_exact_cast_chain_then_shift_runtime_inputs(
            expression,
            scalar_parameter_count,
            root_runtime_inputs,
        )
    })
    .or_else(|| {
        shared_exact_cast_chain_then_divide_remainder_runtime_inputs(
            expression,
            scalar_parameter_count,
            root_runtime_inputs,
        )
    })
}

pub(super) fn shared_exact_cast_chain_suffix_root_runtime_inputs(
    target_type: PrimitiveType,
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let CheckedScalarExpression::IntegerExactCast {
        primitive_type: cast_target_type,
        operand,
        ..
    } = expression
    else {
        return None;
    };
    (*cast_target_type == target_type).then_some(())?;
    shared_exact_cast_chain_runtime_inputs(target_type, operand, scalar_parameter_count)
}

pub(super) fn shared_exact_computed_prefix_cast_chain_suffix_root_runtime_inputs(
    target_type: PrimitiveType,
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let CheckedScalarExpression::IntegerExactCast {
        primitive_type: cast_target_type,
        operand,
        ..
    } = expression
    else {
        return None;
    };
    (*cast_target_type == target_type).then_some(())?;
    shared_exact_computed_prefix_cast_chain_runtime_inputs(
        target_type,
        operand,
        scalar_parameter_count,
    )
}

pub(super) fn shared_exact_computed_prefix_widen_chain_suffix_root_runtime_inputs(
    target_type: PrimitiveType,
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut expected_target = target_type;
    let mut saw_widen = false;
    while let CheckedScalarExpression::IntegerWiden {
        primitive_type: widen_target,
        operand,
        ..
    } = expression
    {
        if *widen_target != expected_target {
            return None;
        }
        let source_type = crate::values::scalar_expression_type(operand)?;
        if !strict_fixed_native_primitive_widen(source_type, *widen_target) {
            return None;
        }
        expected_target = source_type;
        expression = operand;
        saw_widen = true;
    }
    saw_widen.then_some(())?;
    shared_exact_computed_suffix_runtime_inputs(
        expression,
        scalar_parameter_count,
        shared_exact_direct_parameter_suffix_root_runtime_inputs,
    )
}

pub(super) fn shared_exact_computed_prefix_mixed_conversion_chain_suffix_root_runtime_inputs(
    target_type: PrimitiveType,
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut expected_target = target_type;
    let mut saw_widen = false;
    let mut saw_cast = false;
    loop {
        match expression {
            CheckedScalarExpression::IntegerWiden {
                primitive_type: conversion_target,
                operand,
            } => {
                let source_type = crate::values::scalar_expression_type(operand)?;
                if *conversion_target != expected_target
                    || !strict_fixed_native_primitive_widen(source_type, *conversion_target)
                {
                    return None;
                }
                expected_target = source_type;
                expression = operand;
                saw_widen = true;
            }
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: conversion_target,
                operand,
                ..
            } => {
                let source_type = crate::values::scalar_expression_type(operand)?;
                if *conversion_target != expected_target
                    || !partial_fixed_native_primitive_cast(source_type, *conversion_target)
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
    shared_exact_computed_suffix_runtime_inputs(
        expression,
        scalar_parameter_count,
        shared_exact_direct_parameter_suffix_root_runtime_inputs,
    )
}

pub(super) fn shared_exact_direct_parameter_suffix_root_runtime_inputs(
    target_type: PrimitiveType,
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let CheckedScalarExpression::Parameter {
        position,
        primitive_type,
    } = expression
    else {
        return None;
    };
    (*primitive_type == target_type && *position < scalar_parameter_count)
        .then(|| BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)]))
}

pub(super) fn strict_fixed_native_primitive_widen(
    source: PrimitiveType,
    target: PrimitiveType,
) -> bool {
    let Some(source_interval) = fixed_native_primitive_interval(source) else {
        return false;
    };
    let Some(target_interval) = fixed_native_primitive_interval(target) else {
        return false;
    };
    source != target
        && target_interval.0 <= source_interval.0
        && source_interval.1 <= target_interval.1
}

pub(super) fn shared_exact_cast_chain_then_affine_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    root_runtime_inputs: SharedExactComputedSuffixRootRuntimeInputs,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind:
                kind @ (CheckedIntegerBinaryKind::ExactAdd
                | CheckedIntegerBinaryKind::ExactSubtract
                | CheckedIntegerBinaryKind::ExactMultiply),
            primitive_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type)
            || match kind {
                CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract => {
                    !exact_offset_landed_literal(*primitive_type, right)
                }
                CheckedIntegerBinaryKind::ExactMultiply => {
                    !nonnegative_exact_multiply_literal(*primitive_type, right)
                }
                _ => unreachable!("matched one exact affine operation"),
            }
        {
            return None;
        }
        chain_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactAdd
                    | CheckedIntegerBinaryKind::ExactSubtract
                    | CheckedIntegerBinaryKind::ExactMultiply,
                ..
            } => expression = nested,
            root => {
                return root_runtime_inputs(*primitive_type, root, scalar_parameter_count);
            }
        }
    }
}

pub(super) fn shared_exact_cast_chain_then_signed_product_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    root_runtime_inputs: SharedExactComputedSuffixRootRuntimeInputs,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut product = Some((false, 1_u128));
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
            } => expression = nested,
            root if product.is_some() && saw_negative => {
                return root_runtime_inputs(*primitive_type, root, scalar_parameter_count);
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_chain_then_shift_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    root_runtime_inputs: SharedExactComputedSuffixRootRuntimeInputs,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind:
                CheckedIntegerBinaryKind::ExactShiftLeft | CheckedIntegerBinaryKind::ExactShiftRight,
            primitive_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type)
            || landed_exact_shift_literal_count(*primitive_type, right).is_none()
        {
            return None;
        }
        chain_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactShiftLeft | CheckedIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            root => {
                return root_runtime_inputs(*primitive_type, root, scalar_parameter_count);
            }
        }
    }
}

pub(super) fn shared_exact_cast_chain_then_divide_remainder_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    root_runtime_inputs: SharedExactComputedSuffixRootRuntimeInputs,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
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
            } => expression = nested,
            root => {
                return root_runtime_inputs(*primitive_type, root, scalar_parameter_count);
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_chain_then_computed_suffix_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_computed_prefix_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_inputs(
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
pub(crate) fn exact_computed_prefix_widen_chain_then_computed_suffix_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_inputs(
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
pub(crate) fn exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_inputs(
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

pub(super) fn shared_exact_computed_prefix_cast_chain_runtime_inputs(
    target_type: PrimitiveType,
    mut operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    fixed_native_primitive_interval(target_type)?;
    let mut expected_target = target_type;
    let mut followed_nested_cast = false;
    while let CheckedScalarExpression::IntegerExactCast {
        primitive_type: cast_target,
        operand: cast_operand,
        ..
    } = operand
    {
        if !partial_fixed_native_primitive_cast(*cast_target, expected_target) {
            return None;
        }
        expected_target = *cast_target;
        operand = cast_operand;
        followed_nested_cast = true;
    }
    followed_nested_cast.then_some(())?;
    let source_type = crate::values::scalar_expression_type(operand)?;
    partial_fixed_native_primitive_cast(source_type, expected_target).then_some(())?;
    shared_exact_divide_remainder_chain_cast_runtime_inputs(
        expected_target,
        operand,
        scalar_parameter_count,
    )
    .or_else(|| {
        shared_exact_mixed_shift_chain_cast_runtime_inputs(
            expected_target,
            operand,
            scalar_parameter_count,
        )
    })
    .or_else(|| {
        shared_exact_shift_right_chain_cast_runtime_inputs(
            expected_target,
            operand,
            scalar_parameter_count,
        )
    })
    .or_else(|| {
        shared_exact_shift_left_chain_cast_runtime_inputs(
            expected_target,
            operand,
            scalar_parameter_count,
        )
    })
    .or_else(|| {
        shared_exact_affine_chain_cast_runtime_inputs(
            expected_target,
            operand,
            scalar_parameter_count,
        )
    })
    .or_else(|| {
        shared_exact_multiply_chain_cast_runtime_inputs(
            expected_target,
            operand,
            scalar_parameter_count,
        )
    })
    .or_else(|| {
        shared_exact_signed_multiply_chain_cast_runtime_inputs(
            expected_target,
            operand,
            scalar_parameter_count,
        )
    })
    .or_else(|| {
        shared_exact_offset_chain_cast_runtime_inputs(
            expected_target,
            operand,
            scalar_parameter_count,
        )
    })
}

#[cfg(test)]
pub(crate) fn exact_computed_prefix_cast_chain_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_computed_prefix_cast_chain_runtime_inputs(
        target_type,
        operand,
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
pub(crate) fn exact_cast_chain_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_chain_runtime_inputs(target_type, operand, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[derive(Clone, Copy)]
pub(super) enum ExactDivideRemainderTransfer {
    Divide,
    Remainder,
}

pub(super) fn shared_exact_divide_remainder_chain_cast_runtime_inputs(
    target_type: PrimitiveType,
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let target_interval = fixed_native_primitive_interval(target_type)?;
    let mut source_type = None;
    let mut transfers = Vec::new();
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind:
                kind
                @ (CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder),
            primitive_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if source_type.is_some_and(|source_type| source_type != *primitive_type) {
            return None;
        }
        source_type = Some(*primitive_type);
        let divisor = landed_safe_exact_divide_remainder_literal(*primitive_type, right)?;
        transfers.push((
            match kind {
                CheckedIntegerBinaryKind::ExactDivide => ExactDivideRemainderTransfer::Divide,
                CheckedIntegerBinaryKind::ExactRemainder => ExactDivideRemainderTransfer::Remainder,
                _ => unreachable!("matched one exact divide/remainder operation"),
            },
            divisor,
        ));
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder,
                ..
            } => expression = nested,
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if *root_type == *primitive_type && *position < scalar_parameter_count => {
                let source_interval = fixed_native_primitive_interval(*root_type)?;
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

pub(super) fn fixed_native_primitive_interval(
    primitive_type: PrimitiveType,
) -> Option<(i128, i128)> {
    match primitive_type {
        PrimitiveType::I8 => Some((i128::from(i8::MIN), i128::from(i8::MAX))),
        PrimitiveType::I16 => Some((i128::from(i16::MIN), i128::from(i16::MAX))),
        PrimitiveType::I32 => Some((i128::from(i32::MIN), i128::from(i32::MAX))),
        PrimitiveType::I64 => Some((i128::from(i64::MIN), i128::from(i64::MAX))),
        PrimitiveType::U8 => Some((0, i128::from(u8::MAX))),
        PrimitiveType::U16 => Some((0, i128::from(u16::MAX))),
        PrimitiveType::U32 => Some((0, i128::from(u32::MAX))),
        PrimitiveType::U64 => Some((0, i128::from(u64::MAX))),
        _ => None,
    }
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

pub(super) fn landed_safe_exact_divide_remainder_literal(
    primitive_type: PrimitiveType,
    expression: &CheckedScalarExpression,
) -> Option<i128> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    let value = match (
        primitive_type,
        literal.landing().map(|landing| landing.landed_type),
    ) {
        (PrimitiveType::I8, Some(psi_numerics::literals::LandedIntegerType::I8))
        | (PrimitiveType::I16, Some(psi_numerics::literals::LandedIntegerType::I16))
        | (PrimitiveType::I32, Some(psi_numerics::literals::LandedIntegerType::I32))
        | (PrimitiveType::I64, Some(psi_numerics::literals::LandedIntegerType::I64)) => {
            i128::from(literal.value_i64()?)
        }
        (PrimitiveType::U8, Some(psi_numerics::literals::LandedIntegerType::U8))
        | (PrimitiveType::U16, Some(psi_numerics::literals::LandedIntegerType::U16))
        | (PrimitiveType::U32, Some(psi_numerics::literals::LandedIntegerType::U32))
        | (PrimitiveType::U64, Some(psi_numerics::literals::LandedIntegerType::U64)) => {
            i128::from(literal.value_u64()?)
        }
        _ => return None,
    };
    (value != 0 && value != -1).then_some(value)
}

#[cfg(test)]
pub(crate) fn exact_divide_remainder_chain_cast_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_divide_remainder_chain_cast_runtime_inputs(
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

pub(super) fn shared_exact_offset_chain_cast_runtime_inputs(
    target_type: PrimitiveType,
    mut operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    if !matches!(
        target_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract,
            primitive_type,
            left,
            right,
        } = operand
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type)
            || !exact_offset_landed_literal(*primitive_type, right)
        {
            return None;
        }
        chain_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract,
                ..
            } => operand = nested,
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if *root_type == *primitive_type && *position < scalar_parameter_count => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_multiply_chain_cast_runtime_inputs(
    target_type: PrimitiveType,
    mut operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    if !matches!(
        target_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactMultiply,
            primitive_type,
            left,
            right,
        } = operand
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
            } => operand = nested,
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if *root_type == *primitive_type && *position < scalar_parameter_count => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_signed_multiply_chain_cast_runtime_inputs(
    target_type: PrimitiveType,
    mut operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    if !matches!(
        target_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    let mut chain_type = None;
    let mut product = Some((false, 1_u128));
    let mut saw_negative = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactMultiply,
            primitive_type,
            left,
            right,
        } = operand
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
            } => operand = nested,
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if product.is_some()
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

pub(super) fn shared_exact_affine_chain_cast_runtime_inputs(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    if !matches!(
        target_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    shared_exact_affine_chain_runtime_inputs(operand, scalar_parameter_count)
}

pub(super) fn shared_exact_shift_left_chain_cast_runtime_inputs(
    target_type: PrimitiveType,
    mut operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    if !matches!(
        target_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactShiftLeft,
            primitive_type,
            left,
            right,
        } = operand
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type)
            || !safe_exact_shift_literal_count(*primitive_type, right)
        {
            return None;
        }
        chain_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactShiftLeft,
                ..
            } => operand = nested,
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if *root_type == *primitive_type && *position < scalar_parameter_count => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_mixed_shift_chain_cast_runtime_inputs(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    if !matches!(
        target_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    shared_exact_mixed_shift_chain_runtime_inputs(operand, scalar_parameter_count)
}

pub(super) fn shared_exact_shift_right_chain_cast_runtime_inputs(
    target_type: PrimitiveType,
    mut operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    if !matches!(
        target_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactShiftRight,
            primitive_type,
            left,
            right,
        } = operand
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type)
            || !safe_exact_shift_literal_count(*primitive_type, right)
        {
            return None;
        }
        chain_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactShiftRight,
                ..
            } => operand = nested,
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if *root_type == *primitive_type && *position < scalar_parameter_count => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_then_offset_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract,
            primitive_type: target_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *target_type)
            || !exact_offset_landed_literal(*target_type, right)
        {
            return None;
        }
        chain_type = Some(*target_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract,
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
pub(crate) fn exact_cast_then_offset_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_then_offset_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_offset_chain_cast_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_offset_chain_cast_runtime_inputs(target_type, operand, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_multiply_chain_cast_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_multiply_chain_cast_runtime_inputs(target_type, operand, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_affine_chain_cast_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_affine_chain_cast_runtime_inputs(target_type, operand, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_shift_left_chain_cast_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_shift_left_chain_cast_runtime_inputs(target_type, operand, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_mixed_shift_chain_cast_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_mixed_shift_chain_cast_runtime_inputs(
        target_type,
        operand,
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
pub(crate) fn exact_shift_right_chain_cast_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_shift_right_chain_cast_runtime_inputs(
        target_type,
        operand,
        scalar_parameter_count,
    )?
    .into_iter()
    .map(|input| match input {
        SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
        _ => None,
    })
    .collect()
}
