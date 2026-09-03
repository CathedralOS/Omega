//! Shift and cross-family divide/remainder sufficient forms.

use super::*;

pub(super) fn shared_exact_shift_right_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_operation = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactShiftRight,
            primitive_type,
            left,
            right,
        } = expression
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

pub(super) fn shared_exact_cast_then_shift_right_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactShiftRight,
            primitive_type: value_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *value_type)
            || !safe_exact_shift_literal_count(*value_type, right)
        {
            return None;
        }
        chain_type = Some(*value_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if *cast_target_type == *value_type => {
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
pub(crate) fn exact_cast_then_shift_right_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_then_shift_right_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn safe_exact_shift_literal_count(
    value_type: PrimitiveType,
    expression: &CheckedScalarExpression,
) -> bool {
    landed_exact_shift_literal_count(value_type, expression).is_some()
}

pub(super) fn landed_exact_shift_literal_count(
    value_type: PrimitiveType,
    expression: &CheckedScalarExpression,
) -> Option<u128> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    let maximum = match value_type {
        PrimitiveType::I8 | PrimitiveType::U8 => 7,
        PrimitiveType::I16 | PrimitiveType::U16 => 15,
        PrimitiveType::I32 | PrimitiveType::U32 => 31,
        PrimitiveType::I64 | PrimitiveType::U64 => 63,
        _ => return None,
    };
    let landing = match literal.landing().map(|landing| landing.landed_type) {
        Some(psi_numerics::literals::LandedIntegerType::I8)
        | Some(psi_numerics::literals::LandedIntegerType::I16)
        | Some(psi_numerics::literals::LandedIntegerType::I32)
        | Some(psi_numerics::literals::LandedIntegerType::I64) => literal
            .value_i64()
            .and_then(|value| u64::try_from(value).ok()),
        Some(psi_numerics::literals::LandedIntegerType::U8)
        | Some(psi_numerics::literals::LandedIntegerType::U16)
        | Some(psi_numerics::literals::LandedIntegerType::U32)
        | Some(psi_numerics::literals::LandedIntegerType::U64) => literal.value_u64(),
        _ => return None,
    };
    landing.filter(|count| *count <= maximum).map(u128::from)
}

pub(super) fn shared_exact_mixed_shift_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut value_type = None;
    let mut cumulative_left = 0_u128;
    let mut cumulative_right = 0_u128;
    let mut saw_left = false;
    let mut saw_right = false;
    loop {
        let (kind, primitive_type, left, right) = match expression {
            CheckedScalarExpression::IntegerBinary {
                kind: kind @ CheckedIntegerBinaryKind::ExactShiftLeft,
                primitive_type,
                left,
                right,
            }
            | CheckedScalarExpression::IntegerBinary {
                kind: kind @ CheckedIntegerBinaryKind::ExactShiftRight,
                primitive_type,
                left,
                right,
            } => (kind, primitive_type, left, right),
            _ => return None,
        };
        if value_type.is_some_and(|value_type| value_type != *primitive_type) {
            return None;
        }
        value_type = Some(*primitive_type);
        let count = landed_exact_shift_literal_count(*primitive_type, right)?;
        match kind {
            CheckedIntegerBinaryKind::ExactShiftLeft => {
                cumulative_left = cumulative_left.checked_add(count)?;
                saw_left = true;
            }
            CheckedIntegerBinaryKind::ExactShiftRight => {
                cumulative_right = cumulative_right.checked_add(count)?;
                saw_right = true;
            }
            _ => unreachable!("matched one exact shift kind"),
        }
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactShiftLeft | CheckedIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if saw_left
                && saw_right
                && Some(*root_type) == value_type
                && *position < scalar_parameter_count =>
            {
                let _ = (cumulative_left, cumulative_right);
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_arithmetic_then_shift_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut value_type = None;
    let mut saw_left = false;
    loop {
        let (kind, primitive_type, left, right) = match expression {
            CheckedScalarExpression::IntegerBinary {
                kind: kind @ CheckedIntegerBinaryKind::ExactShiftLeft,
                primitive_type,
                left,
                right,
            }
            | CheckedScalarExpression::IntegerBinary {
                kind: kind @ CheckedIntegerBinaryKind::ExactShiftRight,
                primitive_type,
                left,
                right,
            } => (kind, primitive_type, left, right),
            _ => return None,
        };
        if value_type.is_some_and(|value_type| value_type != *primitive_type)
            || landed_exact_shift_literal_count(*primitive_type, right).is_none()
        {
            return None;
        }
        value_type = Some(*primitive_type);
        saw_left |= *kind == CheckedIntegerBinaryKind::ExactShiftLeft;
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactShiftLeft | CheckedIntegerBinaryKind::ExactShiftRight,
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
        if Some(*primitive_type) != value_type
            || match kind {
                CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract => {
                    !exact_offset_landed_literal(*primitive_type, right)
                }
                CheckedIntegerBinaryKind::ExactMultiply => {
                    !nonnegative_exact_multiply_literal(*primitive_type, right)
                }
                _ => unreachable!("matched one exact arithmetic operation"),
            }
        {
            return None;
        }
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactAdd
                    | CheckedIntegerBinaryKind::ExactSubtract
                    | CheckedIntegerBinaryKind::ExactMultiply,
                ..
            } => expression = nested,
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if Some(*root_type) == value_type && *position < scalar_parameter_count => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
pub(crate) fn exact_arithmetic_then_shift_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_arithmetic_then_shift_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn shared_exact_cast_then_mixed_shift_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut value_type = None;
    let mut saw_left = false;
    let mut saw_right = false;
    loop {
        let (kind, primitive_type, left, right) = match expression {
            CheckedScalarExpression::IntegerBinary {
                kind: kind @ CheckedIntegerBinaryKind::ExactShiftLeft,
                primitive_type,
                left,
                right,
            }
            | CheckedScalarExpression::IntegerBinary {
                kind: kind @ CheckedIntegerBinaryKind::ExactShiftRight,
                primitive_type,
                left,
                right,
            } => (kind, primitive_type, left, right),
            _ => return None,
        };
        if value_type.is_some_and(|value_type| value_type != *primitive_type)
            || landed_exact_shift_literal_count(*primitive_type, right).is_none()
        {
            return None;
        }
        value_type = Some(*primitive_type);
        saw_left |= *kind == CheckedIntegerBinaryKind::ExactShiftLeft;
        saw_right |= *kind == CheckedIntegerBinaryKind::ExactShiftRight;
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactShiftLeft | CheckedIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if saw_left && saw_right && Some(*cast_target_type) == value_type => {
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

pub(super) fn shared_exact_shift_cast_shift_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut target_type = None;
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
        if target_type.is_some_and(|target_type| target_type != *primitive_type)
            || landed_exact_shift_literal_count(*primitive_type, right).is_none()
        {
            return None;
        }
        target_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactShiftLeft | CheckedIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if Some(*cast_target_type) == target_type => {
                let mut operand = operand.as_ref();
                let mut source_type = None;
                loop {
                    let CheckedScalarExpression::IntegerBinary {
                        kind:
                            CheckedIntegerBinaryKind::ExactShiftLeft
                            | CheckedIntegerBinaryKind::ExactShiftRight,
                        primitive_type,
                        left,
                        right,
                    } = operand
                    else {
                        return None;
                    };
                    if source_type.is_some_and(|source_type| source_type != *primitive_type)
                        || landed_exact_shift_literal_count(*primitive_type, right).is_none()
                    {
                        return None;
                    }
                    source_type = Some(*primitive_type);
                    match left.as_ref() {
                        nested @ CheckedScalarExpression::IntegerBinary {
                            kind:
                                CheckedIntegerBinaryKind::ExactShiftLeft
                                | CheckedIntegerBinaryKind::ExactShiftRight,
                            ..
                        } => operand = nested,
                        CheckedScalarExpression::Parameter {
                            position,
                            primitive_type: root_type,
                        } if Some(*root_type) == source_type
                            && *position < scalar_parameter_count =>
                        {
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

pub(super) fn shared_exact_affine_shift_cast_sandwich_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    if matches!(
        expression,
        CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactAdd
                | CheckedIntegerBinaryKind::ExactSubtract
                | CheckedIntegerBinaryKind::ExactMultiply,
            ..
        }
    ) {
        let mut target_type = None;
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
            if target_type.is_some_and(|target_type| target_type != *primitive_type)
                || match kind {
                    CheckedIntegerBinaryKind::ExactAdd
                    | CheckedIntegerBinaryKind::ExactSubtract => {
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
            target_type = Some(*primitive_type);
            match left.as_ref() {
                nested @ CheckedScalarExpression::IntegerBinary {
                    kind:
                        CheckedIntegerBinaryKind::ExactAdd
                        | CheckedIntegerBinaryKind::ExactSubtract
                        | CheckedIntegerBinaryKind::ExactMultiply,
                    ..
                } => expression = nested,
                CheckedScalarExpression::IntegerExactCast {
                    primitive_type: cast_target_type,
                    operand,
                    ..
                } if Some(*cast_target_type) == target_type => {
                    let mut operand = operand.as_ref();
                    let mut source_type = None;
                    loop {
                        let CheckedScalarExpression::IntegerBinary {
                            kind:
                                CheckedIntegerBinaryKind::ExactShiftLeft
                                | CheckedIntegerBinaryKind::ExactShiftRight,
                            primitive_type,
                            left,
                            right,
                        } = operand
                        else {
                            return None;
                        };
                        if source_type.is_some_and(|source_type| source_type != *primitive_type)
                            || landed_exact_shift_literal_count(*primitive_type, right).is_none()
                        {
                            return None;
                        }
                        source_type = Some(*primitive_type);
                        match left.as_ref() {
                            nested @ CheckedScalarExpression::IntegerBinary {
                                kind:
                                    CheckedIntegerBinaryKind::ExactShiftLeft
                                    | CheckedIntegerBinaryKind::ExactShiftRight,
                                ..
                            } => operand = nested,
                            CheckedScalarExpression::Parameter {
                                position,
                                primitive_type: root_type,
                            } if Some(*root_type) == source_type
                                && *position < scalar_parameter_count =>
                            {
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
        if target_type.is_some_and(|target_type| target_type != *primitive_type)
            || landed_exact_shift_literal_count(*primitive_type, right).is_none()
        {
            return None;
        }
        target_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactShiftLeft | CheckedIntegerBinaryKind::ExactShiftRight,
                ..
            } => expression = nested,
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if Some(*cast_target_type) == target_type => {
                let mut operand = operand.as_ref();
                let mut source_type = None;
                loop {
                    let CheckedScalarExpression::IntegerBinary {
                        kind:
                            kind @ (CheckedIntegerBinaryKind::ExactAdd
                            | CheckedIntegerBinaryKind::ExactSubtract
                            | CheckedIntegerBinaryKind::ExactMultiply),
                        primitive_type,
                        left,
                        right,
                    } = operand
                    else {
                        return None;
                    };
                    if source_type.is_some_and(|source_type| source_type != *primitive_type)
                        || match kind {
                            CheckedIntegerBinaryKind::ExactAdd
                            | CheckedIntegerBinaryKind::ExactSubtract => {
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
                    source_type = Some(*primitive_type);
                    match left.as_ref() {
                        nested @ CheckedScalarExpression::IntegerBinary {
                            kind:
                                CheckedIntegerBinaryKind::ExactAdd
                                | CheckedIntegerBinaryKind::ExactSubtract
                                | CheckedIntegerBinaryKind::ExactMultiply,
                            ..
                        } => operand = nested,
                        CheckedScalarExpression::Parameter {
                            position,
                            primitive_type: root_type,
                        } if Some(*root_type) == source_type
                            && *position < scalar_parameter_count =>
                        {
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

pub(super) fn checked_exact_cross_cast_chain_family(
    expression: &CheckedScalarExpression,
) -> Option<ExactCrossCastChainFamily> {
    match expression {
        CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder,
            ..
        } => Some(ExactCrossCastChainFamily::DivideRemainder),
        CheckedScalarExpression::IntegerBinary {
            kind:
                CheckedIntegerBinaryKind::ExactAdd
                | CheckedIntegerBinaryKind::ExactSubtract
                | CheckedIntegerBinaryKind::ExactMultiply,
            ..
        } => Some(ExactCrossCastChainFamily::Affine),
        CheckedScalarExpression::IntegerBinary {
            kind:
                CheckedIntegerBinaryKind::ExactShiftLeft | CheckedIntegerBinaryKind::ExactShiftRight,
            ..
        } => Some(ExactCrossCastChainFamily::Shift),
        _ => None,
    }
}

pub(super) fn checked_exact_cross_cast_chain_link(
    expression: &CheckedScalarExpression,
    family: ExactCrossCastChainFamily,
) -> Option<(PrimitiveType, &CheckedScalarExpression)> {
    match (family, expression) {
        (
            ExactCrossCastChainFamily::DivideRemainder,
            CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder,
                primitive_type,
                left,
                right,
            },
        ) if safe_exact_divide_remainder_literal(*primitive_type, right) => {
            Some((*primitive_type, left))
        }
        (
            ExactCrossCastChainFamily::Affine,
            CheckedScalarExpression::IntegerBinary {
                kind:
                    kind @ (CheckedIntegerBinaryKind::ExactAdd
                    | CheckedIntegerBinaryKind::ExactSubtract
                    | CheckedIntegerBinaryKind::ExactMultiply),
                primitive_type,
                left,
                right,
            },
        ) if match kind {
            CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract => {
                exact_offset_landed_literal(*primitive_type, right)
            }
            CheckedIntegerBinaryKind::ExactMultiply => {
                nonnegative_exact_multiply_literal(*primitive_type, right)
            }
            _ => false,
        } =>
        {
            Some((*primitive_type, left))
        }
        (
            ExactCrossCastChainFamily::Shift,
            CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactShiftLeft | CheckedIntegerBinaryKind::ExactShiftRight,
                primitive_type,
                left,
                right,
            },
        ) if landed_exact_shift_literal_count(*primitive_type, right).is_some() => {
            Some((*primitive_type, left))
        }
        _ => None,
    }
}

pub(super) fn checked_exact_cross_cast_parameter_root(
    mut expression: &CheckedScalarExpression,
    family: ExactCrossCastChainFamily,
    scalar_parameter_count: usize,
) -> Option<(PrimitiveType, usize)> {
    let mut chain_type = None;
    loop {
        let (primitive_type, left) = checked_exact_cross_cast_chain_link(expression, family)?;
        if fixed_native_primitive_interval(primitive_type).is_none()
            || chain_type.is_some_and(|chain_type| chain_type != primitive_type)
        {
            return None;
        }
        chain_type = Some(primitive_type);
        match left {
            nested if checked_exact_cross_cast_chain_family(nested) == Some(family) => {
                expression = nested;
            }
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if *root_type == primitive_type && *position < scalar_parameter_count => {
                return Some((primitive_type, *position));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_divide_remainder_cross_cast_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let outer_family = checked_exact_cross_cast_chain_family(expression)?;
    let mut target_type = None;
    loop {
        let (primitive_type, left) = checked_exact_cross_cast_chain_link(expression, outer_family)?;
        if fixed_native_primitive_interval(primitive_type).is_none()
            || target_type.is_some_and(|target_type| target_type != primitive_type)
        {
            return None;
        }
        target_type = Some(primitive_type);
        match left {
            nested if checked_exact_cross_cast_chain_family(nested) == Some(outer_family) => {
                expression = nested;
            }
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if Some(*cast_target_type) == target_type => {
                let source_family = checked_exact_cross_cast_chain_family(operand)?;
                if (outer_family == ExactCrossCastChainFamily::DivideRemainder)
                    == (source_family == ExactCrossCastChainFamily::DivideRemainder)
                {
                    return None;
                }
                let (_, position) = checked_exact_cross_cast_parameter_root(
                    operand,
                    source_family,
                    scalar_parameter_count,
                )?;
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_divide_remainder_cast_sandwich_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let family = ExactCrossCastChainFamily::DivideRemainder;
    let mut target_type = None;
    loop {
        let (primitive_type, left) = checked_exact_cross_cast_chain_link(expression, family)?;
        if fixed_native_primitive_interval(primitive_type).is_none()
            || target_type.is_some_and(|target_type| target_type != primitive_type)
        {
            return None;
        }
        target_type = Some(primitive_type);
        match left {
            nested if checked_exact_cross_cast_chain_family(nested) == Some(family) => {
                expression = nested;
            }
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if Some(*cast_target_type) == target_type => {
                let (_, position) = checked_exact_cross_cast_parameter_root(
                    operand,
                    family,
                    scalar_parameter_count,
                )?;
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_divide_remainder_cross_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let outer_family = checked_exact_cross_cast_chain_family(expression)?;
    let mut outer_type = None;
    loop {
        let (primitive_type, left) = checked_exact_cross_cast_chain_link(expression, outer_family)?;
        if fixed_native_primitive_interval(primitive_type).is_none()
            || outer_type.is_some_and(|outer_type| outer_type != primitive_type)
        {
            return None;
        }
        outer_type = Some(primitive_type);
        match left {
            nested if checked_exact_cross_cast_chain_family(nested) == Some(outer_family) => {
                expression = nested;
            }
            source => {
                let source_family = checked_exact_cross_cast_chain_family(source)?;
                if (outer_family == ExactCrossCastChainFamily::DivideRemainder)
                    == (source_family == ExactCrossCastChainFamily::DivideRemainder)
                {
                    return None;
                }
                let (source_type, position) = checked_exact_cross_cast_parameter_root(
                    source,
                    source_family,
                    scalar_parameter_count,
                )?;
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

#[cfg(test)]
pub(crate) fn exact_divide_remainder_cross_chain_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_divide_remainder_cross_chain_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_divide_remainder_cross_cast_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_divide_remainder_cast_sandwich_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_divide_remainder_cast_sandwich_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_affine_shift_cast_sandwich_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_affine_shift_cast_sandwich_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_shift_cast_shift_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_shift_cast_shift_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_mixed_shift_chain_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_mixed_shift_chain_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_cast_then_mixed_shift_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_then_mixed_shift_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn shared_exact_shift_left_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_operation = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactShiftLeft,
            primitive_type,
            left,
            right,
        } = expression
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

pub(super) fn shared_exact_cast_then_shift_left_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactShiftLeft,
            primitive_type: value_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *value_type)
            || !safe_exact_shift_literal_count(*value_type, right)
        {
            return None;
        }
        chain_type = Some(*value_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactShiftLeft,
                ..
            } => expression = nested,
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: cast_target_type,
                operand,
                ..
            } if *cast_target_type == *value_type => {
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
pub(crate) fn exact_shift_left_chain_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_shift_left_chain_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_cast_then_shift_left_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_then_shift_left_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}
