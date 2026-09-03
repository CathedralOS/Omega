//! Exact shift and cross-family shift-chain runtime-input classifiers.

use super::*;

pub(super) fn shared_exact_shift_right_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_operation = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactShiftRight,
            scalar_type: ScalarType::Integer(value_type),
            left,
            right,
        } = expression
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
            } => {
                saw_nested_operation = true;
                expression = nested;
            }
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if saw_nested_operation && *root_type == *value_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_then_shift_right_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactShiftRight,
            scalar_type: ScalarType::Integer(value_type),
            left,
            right,
        } = expression
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
            } => expression = nested,
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if *cast_target_type == *value_type => {
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

pub(super) fn safe_exact_shift_landed_literal_count(
    value_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> bool {
    landed_exact_shift_literal_count(value_type, expression).is_some()
}

pub(super) fn landed_exact_shift_literal_count(
    value_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> Option<u128> {
    let LoweredDirectExpression::IntegerLiteral {
        value,
        scalar_type: ScalarType::Integer(count_type),
    } = expression
    else {
        return None;
    };
    if !native_fixed_integer_type(*count_type) {
        return None;
    }
    let count = match value {
        IntegerValue::Signed(value) => u128::try_from(*value).ok(),
        IntegerValue::Unsigned(value) => Some(*value),
    };
    count.filter(|count| *count < u128::from(value_type.bits()))
}

pub(super) fn shared_exact_mixed_shift_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut value_type = None;
    let mut cumulative_left = 0_u128;
    let mut cumulative_right = 0_u128;
    let mut saw_left = false;
    let mut saw_right = false;
    loop {
        let (kind, integer_type, left, right) = match expression {
            LoweredDirectExpression::IntegerBinary {
                kind: kind @ LoweredIntegerBinaryKind::ExactShiftLeft,
                scalar_type: ScalarType::Integer(integer_type),
                left,
                right,
            }
            | LoweredDirectExpression::IntegerBinary {
                kind: kind @ LoweredIntegerBinaryKind::ExactShiftRight,
                scalar_type: ScalarType::Integer(integer_type),
                left,
                right,
            } => (kind, integer_type, left, right),
            _ => return None,
        };
        if !native_fixed_integer_type(*integer_type)
            || value_type.is_some_and(|value_type| value_type != *integer_type)
        {
            return None;
        }
        value_type = Some(*integer_type);
        let count = landed_exact_shift_literal_count(*integer_type, right)?;
        match kind {
            LoweredIntegerBinaryKind::ExactShiftLeft => {
                cumulative_left = cumulative_left.checked_add(count)?;
                saw_left = true;
            }
            LoweredIntegerBinaryKind::ExactShiftRight => {
                cumulative_right = cumulative_right.checked_add(count)?;
                saw_right = true;
            }
            _ => unreachable!("matched one exact shift kind"),
        }
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactShiftLeft | LoweredIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if saw_left && saw_right && Some(*root_type) == value_type => {
                let _ = (cumulative_left, cumulative_right);
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_then_mixed_shift_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut value_type = None;
    let mut saw_left = false;
    let mut saw_right = false;
    loop {
        let (kind, integer_type, left, right) = match expression {
            LoweredDirectExpression::IntegerBinary {
                kind: kind @ LoweredIntegerBinaryKind::ExactShiftLeft,
                scalar_type: ScalarType::Integer(integer_type),
                left,
                right,
            }
            | LoweredDirectExpression::IntegerBinary {
                kind: kind @ LoweredIntegerBinaryKind::ExactShiftRight,
                scalar_type: ScalarType::Integer(integer_type),
                left,
                right,
            } => (kind, integer_type, left, right),
            _ => return None,
        };
        if !native_fixed_integer_type(*integer_type)
            || value_type.is_some_and(|value_type| value_type != *integer_type)
            || landed_exact_shift_literal_count(*integer_type, right).is_none()
        {
            return None;
        }
        value_type = Some(*integer_type);
        saw_left |= *kind == LoweredIntegerBinaryKind::ExactShiftLeft;
        saw_right |= *kind == LoweredIntegerBinaryKind::ExactShiftRight;
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactShiftLeft | LoweredIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if saw_left && saw_right && Some(*cast_target_type) == value_type => {
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

pub(super) fn shared_exact_shift_cast_shift_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut target_type = None;
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
            || target_type.is_some_and(|target_type| target_type != *integer_type)
            || landed_exact_shift_literal_count(*integer_type, right).is_none()
        {
            return None;
        }
        target_type = Some(*integer_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactShiftLeft | LoweredIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if Some(*cast_target_type) == target_type => {
                let mut operand = operand.as_ref();
                let mut source_type = None;
                loop {
                    let LoweredDirectExpression::IntegerBinary {
                        kind:
                            LoweredIntegerBinaryKind::ExactShiftLeft
                            | LoweredIntegerBinaryKind::ExactShiftRight,
                        scalar_type: ScalarType::Integer(integer_type),
                        left,
                        right,
                    } = operand
                    else {
                        return None;
                    };
                    if !native_fixed_integer_type(*integer_type)
                        || source_type.is_some_and(|source_type| source_type != *integer_type)
                        || landed_exact_shift_literal_count(*integer_type, right).is_none()
                    {
                        return None;
                    }
                    source_type = Some(*integer_type);
                    match left.as_ref() {
                        nested @ LoweredDirectExpression::IntegerBinary {
                            kind:
                                LoweredIntegerBinaryKind::ExactShiftLeft
                                | LoweredIntegerBinaryKind::ExactShiftRight,
                            ..
                        } => operand = nested,
                        LoweredDirectExpression::Parameter {
                            position,
                            scalar_type: ScalarType::Integer(root_type),
                        } if Some(*root_type) == source_type => {
                            return Some(BTreeSet::from([
                                SharedBooleanRuntimeInput::IntegerScalar(*position),
                            ]));
                        }
                        _ => return None,
                    }
                }
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_affine_shift_cast_sandwich_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    if matches!(
        expression,
        LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactAdd
                | LoweredIntegerBinaryKind::ExactSubtract
                | LoweredIntegerBinaryKind::ExactMultiply,
            ..
        }
    ) {
        let mut target_type = None;
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
                || target_type.is_some_and(|target_type| target_type != *integer_type)
                || match kind {
                    LoweredIntegerBinaryKind::ExactAdd
                    | LoweredIntegerBinaryKind::ExactSubtract => {
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
            target_type = Some(*integer_type);
            match left.as_ref() {
                nested @ LoweredDirectExpression::IntegerBinary {
                    kind:
                        LoweredIntegerBinaryKind::ExactAdd
                        | LoweredIntegerBinaryKind::ExactSubtract
                        | LoweredIntegerBinaryKind::ExactMultiply,
                    ..
                } => expression = nested,
                LoweredDirectExpression::IntegerExactCast {
                    scalar_type: ScalarType::Integer(cast_target_type),
                    operand,
                } if Some(*cast_target_type) == target_type => {
                    let mut operand = operand.as_ref();
                    let mut source_type = None;
                    loop {
                        let LoweredDirectExpression::IntegerBinary {
                            kind:
                                LoweredIntegerBinaryKind::ExactShiftLeft
                                | LoweredIntegerBinaryKind::ExactShiftRight,
                            scalar_type: ScalarType::Integer(integer_type),
                            left,
                            right,
                        } = operand
                        else {
                            return None;
                        };
                        if !native_fixed_integer_type(*integer_type)
                            || source_type.is_some_and(|source_type| source_type != *integer_type)
                            || landed_exact_shift_literal_count(*integer_type, right).is_none()
                        {
                            return None;
                        }
                        source_type = Some(*integer_type);
                        match left.as_ref() {
                            nested @ LoweredDirectExpression::IntegerBinary {
                                kind:
                                    LoweredIntegerBinaryKind::ExactShiftLeft
                                    | LoweredIntegerBinaryKind::ExactShiftRight,
                                ..
                            } => operand = nested,
                            LoweredDirectExpression::Parameter {
                                position,
                                scalar_type: ScalarType::Integer(root_type),
                            } if Some(*root_type) == source_type => {
                                return Some(BTreeSet::from([
                                    SharedBooleanRuntimeInput::IntegerScalar(*position),
                                ]));
                            }
                            _ => return None,
                        }
                    }
                }
                _ => return None,
            }
        }
    }

    let mut target_type = None;
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
            || target_type.is_some_and(|target_type| target_type != *integer_type)
            || landed_exact_shift_literal_count(*integer_type, right).is_none()
        {
            return None;
        }
        target_type = Some(*integer_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactShiftLeft | LoweredIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if Some(*cast_target_type) == target_type => {
                let mut operand = operand.as_ref();
                let mut source_type = None;
                loop {
                    let LoweredDirectExpression::IntegerBinary {
                        kind:
                            kind @ (LoweredIntegerBinaryKind::ExactAdd
                            | LoweredIntegerBinaryKind::ExactSubtract
                            | LoweredIntegerBinaryKind::ExactMultiply),
                        scalar_type: ScalarType::Integer(integer_type),
                        left,
                        right,
                    } = operand
                    else {
                        return None;
                    };
                    if !native_fixed_integer_type(*integer_type)
                        || source_type.is_some_and(|source_type| source_type != *integer_type)
                        || match kind {
                            LoweredIntegerBinaryKind::ExactAdd
                            | LoweredIntegerBinaryKind::ExactSubtract => {
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
                    source_type = Some(*integer_type);
                    match left.as_ref() {
                        nested @ LoweredDirectExpression::IntegerBinary {
                            kind:
                                LoweredIntegerBinaryKind::ExactAdd
                                | LoweredIntegerBinaryKind::ExactSubtract
                                | LoweredIntegerBinaryKind::ExactMultiply,
                            ..
                        } => operand = nested,
                        LoweredDirectExpression::Parameter {
                            position,
                            scalar_type: ScalarType::Integer(root_type),
                        } if Some(*root_type) == source_type => {
                            return Some(BTreeSet::from([
                                SharedBooleanRuntimeInput::IntegerScalar(*position),
                            ]));
                        }
                        _ => return None,
                    }
                }
            }
            _ => return None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactCrossCastChainFamily {
    DivideRemainder,
    Affine,
    Shift,
}

pub(super) fn lowered_exact_cross_cast_chain_family(
    expression: &LoweredDirectExpression,
) -> Option<ExactCrossCastChainFamily> {
    match expression {
        LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
            ..
        } => Some(ExactCrossCastChainFamily::DivideRemainder),
        LoweredDirectExpression::IntegerBinary {
            kind:
                LoweredIntegerBinaryKind::ExactAdd
                | LoweredIntegerBinaryKind::ExactSubtract
                | LoweredIntegerBinaryKind::ExactMultiply,
            ..
        } => Some(ExactCrossCastChainFamily::Affine),
        LoweredDirectExpression::IntegerBinary {
            kind:
                LoweredIntegerBinaryKind::ExactShiftLeft | LoweredIntegerBinaryKind::ExactShiftRight,
            ..
        } => Some(ExactCrossCastChainFamily::Shift),
        _ => None,
    }
}

pub(super) fn lowered_exact_cross_cast_chain_link(
    expression: &LoweredDirectExpression,
    family: ExactCrossCastChainFamily,
) -> Option<(psi_core::IntegerType, &LoweredDirectExpression)> {
    match (family, expression) {
        (
            ExactCrossCastChainFamily::DivideRemainder,
            LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
                scalar_type: ScalarType::Integer(integer_type),
                left,
                right,
            },
        ) if safe_exact_divide_remainder_landed_literal(*integer_type, right) => {
            Some((*integer_type, left))
        }
        (
            ExactCrossCastChainFamily::Affine,
            LoweredDirectExpression::IntegerBinary {
                kind:
                    kind @ (LoweredIntegerBinaryKind::ExactAdd
                    | LoweredIntegerBinaryKind::ExactSubtract
                    | LoweredIntegerBinaryKind::ExactMultiply),
                scalar_type: ScalarType::Integer(integer_type),
                left,
                right,
            },
        ) if match kind {
            LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract => {
                exact_offset_landed_literal(*integer_type, right)
            }
            LoweredIntegerBinaryKind::ExactMultiply => {
                nonnegative_exact_multiply_landed_literal(*integer_type, right)
            }
            _ => false,
        } =>
        {
            Some((*integer_type, left))
        }
        (
            ExactCrossCastChainFamily::Shift,
            LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactShiftLeft | LoweredIntegerBinaryKind::ExactShiftRight,
                scalar_type: ScalarType::Integer(integer_type),
                left,
                right,
            },
        ) if landed_exact_shift_literal_count(*integer_type, right).is_some() => {
            Some((*integer_type, left))
        }
        _ => None,
    }
}

pub(super) fn lowered_exact_cross_cast_parameter_root(
    mut expression: &LoweredDirectExpression,
    family: ExactCrossCastChainFamily,
) -> Option<(psi_core::IntegerType, usize)> {
    let mut chain_type = None;
    loop {
        let (integer_type, left) = lowered_exact_cross_cast_chain_link(expression, family)?;
        if !native_fixed_integer_type(integer_type)
            || chain_type.is_some_and(|chain_type| chain_type != integer_type)
        {
            return None;
        }
        chain_type = Some(integer_type);
        match left {
            nested if lowered_exact_cross_cast_chain_family(nested) == Some(family) => {
                expression = nested;
            }
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == integer_type => return Some((integer_type, *position)),
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_divide_remainder_cross_cast_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let outer_family = lowered_exact_cross_cast_chain_family(expression)?;
    let mut target_type = None;
    loop {
        let (integer_type, left) = lowered_exact_cross_cast_chain_link(expression, outer_family)?;
        if !native_fixed_integer_type(integer_type)
            || target_type.is_some_and(|target_type| target_type != integer_type)
        {
            return None;
        }
        target_type = Some(integer_type);
        match left {
            nested if lowered_exact_cross_cast_chain_family(nested) == Some(outer_family) => {
                expression = nested;
            }
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if Some(*cast_target_type) == target_type => {
                let source_family = lowered_exact_cross_cast_chain_family(operand)?;
                if (outer_family == ExactCrossCastChainFamily::DivideRemainder)
                    == (source_family == ExactCrossCastChainFamily::DivideRemainder)
                {
                    return None;
                }
                let (_, position) =
                    lowered_exact_cross_cast_parameter_root(operand, source_family)?;
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_divide_remainder_cast_sandwich_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let family = ExactCrossCastChainFamily::DivideRemainder;
    let mut target_type = None;
    loop {
        let (integer_type, left) = lowered_exact_cross_cast_chain_link(expression, family)?;
        if !native_fixed_integer_type(integer_type)
            || target_type.is_some_and(|target_type| target_type != integer_type)
        {
            return None;
        }
        target_type = Some(integer_type);
        match left {
            nested if lowered_exact_cross_cast_chain_family(nested) == Some(family) => {
                expression = nested;
            }
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if Some(*cast_target_type) == target_type => {
                let (_, position) = lowered_exact_cross_cast_parameter_root(operand, family)?;
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_divide_remainder_cross_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let outer_family = lowered_exact_cross_cast_chain_family(expression)?;
    let mut outer_type = None;
    loop {
        let (integer_type, left) = lowered_exact_cross_cast_chain_link(expression, outer_family)?;
        if !native_fixed_integer_type(integer_type)
            || outer_type.is_some_and(|outer_type| outer_type != integer_type)
        {
            return None;
        }
        outer_type = Some(integer_type);
        match left {
            nested if lowered_exact_cross_cast_chain_family(nested) == Some(outer_family) => {
                expression = nested;
            }
            source => {
                let source_family = lowered_exact_cross_cast_chain_family(source)?;
                if (outer_family == ExactCrossCastChainFamily::DivideRemainder)
                    == (source_family == ExactCrossCastChainFamily::DivideRemainder)
                {
                    return None;
                }
                let (source_type, position) =
                    lowered_exact_cross_cast_parameter_root(source, source_family)?;
                if Some(source_type) != outer_type {
                    return None;
                }
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    position,
                )]));
            }
        }
    }
}

pub(super) fn shared_exact_arithmetic_then_shift_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut value_type = None;
    let mut saw_left = false;
    loop {
        let (kind, integer_type, left, right) = match expression {
            LoweredDirectExpression::IntegerBinary {
                kind: kind @ LoweredIntegerBinaryKind::ExactShiftLeft,
                scalar_type: ScalarType::Integer(integer_type),
                left,
                right,
            }
            | LoweredDirectExpression::IntegerBinary {
                kind: kind @ LoweredIntegerBinaryKind::ExactShiftRight,
                scalar_type: ScalarType::Integer(integer_type),
                left,
                right,
            } => (kind, integer_type, left, right),
            _ => return None,
        };
        if !native_fixed_integer_type(*integer_type)
            || value_type.is_some_and(|value_type| value_type != *integer_type)
            || landed_exact_shift_literal_count(*integer_type, right).is_none()
        {
            return None;
        }
        value_type = Some(*integer_type);
        saw_left |= *kind == LoweredIntegerBinaryKind::ExactShiftLeft;
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactShiftLeft | LoweredIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            arithmetic if saw_left => {
                expression = arithmetic;
                break;
            }
            _ => return None,
        }
    }

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
        if Some(*integer_type) != value_type
            || match kind {
                LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract => {
                    !exact_offset_landed_literal(*integer_type, right)
                }
                LoweredIntegerBinaryKind::ExactMultiply => {
                    !nonnegative_exact_multiply_landed_literal(*integer_type, right)
                }
                _ => unreachable!("matched one exact arithmetic operation"),
            }
        {
            return None;
        }
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactAdd
                    | LoweredIntegerBinaryKind::ExactSubtract
                    | LoweredIntegerBinaryKind::ExactMultiply,
                ..
            } => expression = nested,
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if Some(*root_type) == value_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_shift_left_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_operation = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactShiftLeft,
            scalar_type: ScalarType::Integer(value_type),
            left,
            right,
        } = expression
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
            } => {
                saw_nested_operation = true;
                expression = nested;
            }
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if saw_nested_operation && *root_type == *value_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_then_shift_left_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactShiftLeft,
            scalar_type: ScalarType::Integer(value_type),
            left,
            right,
        } = expression
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
            } => expression = nested,
            LoweredDirectExpression::IntegerExactCast {
                scalar_type: ScalarType::Integer(cast_target_type),
                operand,
            } if *cast_target_type == *value_type => {
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
