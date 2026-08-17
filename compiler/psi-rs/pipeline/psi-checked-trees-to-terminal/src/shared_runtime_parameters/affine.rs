//! Affine, offset-chain, and affine fork/join runtime-input classifiers.

use super::*;

pub(super) fn shared_exact_cast_then_offset_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract,
            scalar_type: ScalarType::Integer(target_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        if !native_fixed_integer_type(*target_type)
            || chain_type.is_some_and(|chain_type| chain_type != *target_type)
            || !exact_offset_landed_literal(*target_type, right)
        {
            return None;
        }
        chain_type = Some(*target_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract,
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

pub(super) fn shared_exact_add_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_add = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactAdd,
            scalar_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *scalar_type) {
            return None;
        }
        chain_type = Some(*scalar_type);
        let left_is_literal = matches!(
            left.as_ref(),
            LoweredDirectExpression::IntegerLiteral { .. }
        );
        let right_is_literal = matches!(
            right.as_ref(),
            LoweredDirectExpression::IntegerLiteral { .. }
        );
        match (left.as_ref(), right.as_ref()) {
            (
                nested @ LoweredDirectExpression::IntegerBinary {
                    kind: LoweredIntegerBinaryKind::ExactAdd,
                    ..
                },
                _,
            ) if right_is_literal => {
                saw_nested_add = true;
                expression = nested;
            }
            (
                _,
                nested @ LoweredDirectExpression::IntegerBinary {
                    kind: LoweredIntegerBinaryKind::ExactAdd,
                    ..
                },
            ) if left_is_literal => {
                saw_nested_add = true;
                expression = nested;
            }
            _ if saw_nested_add && (left_is_literal || right_is_literal) => {
                let mut parameters = shared_integer_runtime_parameters_with_shells(left, 0, false)?;
                parameters.extend(shared_integer_runtime_parameters_with_shells(
                    right, 0, false,
                )?);
                return Some(parameters);
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_subtract_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_subtract = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactSubtract,
            scalar_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *scalar_type)
            || !matches!(
                right.as_ref(),
                LoweredDirectExpression::IntegerLiteral { .. }
            )
        {
            return None;
        }
        chain_type = Some(*scalar_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactSubtract,
                ..
            } => {
                saw_nested_subtract = true;
                expression = nested;
            }
            _ if saw_nested_subtract => {
                let mut parameters = shared_integer_runtime_parameters_with_shells(left, 0, false)?;
                parameters.extend(shared_integer_runtime_parameters_with_shells(
                    right, 0, false,
                )?);
                return Some(parameters);
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_mixed_add_subtract_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_add = false;
    let mut saw_subtract = false;
    let mut saw_nested_operation = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind:
                kind @ (LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract),
            scalar_type: ScalarType::Integer(integer_type),
            left,
            right,
        } = expression
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
        saw_add |= *kind == LoweredIntegerBinaryKind::ExactAdd;
        saw_subtract |= *kind == LoweredIntegerBinaryKind::ExactSubtract;
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract,
                ..
            } => {
                saw_nested_operation = true;
                expression = nested;
            }
            LoweredDirectExpression::Parameter {
                position,
                scalar_type: ScalarType::Integer(root_type),
            } if saw_nested_operation && saw_add && saw_subtract && *root_type == *integer_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn exact_offset_landed_literal(
    integer_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> bool {
    let LoweredDirectExpression::IntegerLiteral { value, scalar_type } = expression else {
        return false;
    };
    if *scalar_type != ScalarType::Integer(integer_type) || !native_fixed_integer_type(integer_type)
    {
        return false;
    }
    matches!(
        (integer_type.sign(), value),
        (IntegerSign::Signed, IntegerValue::Signed(_))
            | (IntegerSign::Unsigned, IntegerValue::Unsigned(_))
    )
}

pub(super) fn shared_exact_affine_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_offset = false;
    let mut saw_multiply = false;
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
        saw_offset |= matches!(
            kind,
            LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract
        );
        saw_multiply |= *kind == LoweredIntegerBinaryKind::ExactMultiply;
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
            } if saw_offset && saw_multiply && *root_type == *integer_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_shift_then_arithmetic_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut value_type = None;
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
            || value_type.is_some_and(|value_type| value_type != *integer_type)
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
        value_type = Some(*integer_type);
        match left.as_ref() {
            nested @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactAdd
                    | LoweredIntegerBinaryKind::ExactSubtract
                    | LoweredIntegerBinaryKind::ExactMultiply,
                ..
            } => expression = nested,
            shift @ LoweredDirectExpression::IntegerBinary {
                kind:
                    LoweredIntegerBinaryKind::ExactShiftLeft | LoweredIntegerBinaryKind::ExactShiftRight,
                ..
            } => {
                expression = shift;
                break;
            }
            _ => return None,
        }
    }

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
        if Some(*integer_type) != value_type
            || landed_exact_shift_literal_count(*integer_type, right).is_none()
        {
            return None;
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
            } if Some(*root_type) == value_type => {
                return Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                    *position,
                )]));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_cast_then_affine_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_offset = false;
    let mut saw_multiply = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind:
                kind @ (LoweredIntegerBinaryKind::ExactAdd
                | LoweredIntegerBinaryKind::ExactSubtract
                | LoweredIntegerBinaryKind::ExactMultiply),
            scalar_type: ScalarType::Integer(target_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        if !native_fixed_integer_type(*target_type)
            || chain_type.is_some_and(|chain_type| chain_type != *target_type)
            || match kind {
                LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract => {
                    !exact_offset_landed_literal(*target_type, right)
                }
                LoweredIntegerBinaryKind::ExactMultiply => {
                    !nonnegative_exact_multiply_landed_literal(*target_type, right)
                }
                _ => unreachable!("matched one exact affine operation"),
            }
        {
            return None;
        }
        chain_type = Some(*target_type);
        saw_offset |= matches!(
            kind,
            LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract
        );
        saw_multiply |= *kind == LoweredIntegerBinaryKind::ExactMultiply;
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
            } if saw_offset && saw_multiply && *cast_target_type == *target_type => {
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

pub(super) fn shared_exact_affine_cast_affine_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
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
                            source_kind @ (LoweredIntegerBinaryKind::ExactAdd
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
                        || match source_kind {
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

pub(super) fn shared_exact_signed_affine_cast_affine_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut target_type = None;
    let mut target_coefficient = Some((false, 1_u128));
    let mut target_offset = Some((false, 0_u128));
    let mut target_saw_offset = false;
    let mut target_saw_negative_factor = false;
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
        let literal = signed_exact_multiply_landed_value(*integer_type, right)?;
        if !native_fixed_integer_type(*integer_type)
            || target_type.is_some_and(|target_type| target_type != *integer_type)
        {
            return None;
        }
        (target_coefficient, target_offset) =
            checked_lowered_signed_affine_step(target_coefficient, target_offset, *kind, literal);
        target_saw_offset |= matches!(
            kind,
            LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract
        );
        target_saw_negative_factor |=
            *kind == LoweredIntegerBinaryKind::ExactMultiply && literal < 0;
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
            } if target_coefficient.is_some()
                && target_offset.is_some()
                && Some(*cast_target_type) == target_type =>
            {
                let mut source_expression = operand.as_ref();
                let mut source_type = None;
                let mut source_coefficient = Some((false, 1_u128));
                let mut source_offset = Some((false, 0_u128));
                let mut source_saw_offset = false;
                let mut source_saw_negative_factor = false;
                loop {
                    let LoweredDirectExpression::IntegerBinary {
                        kind:
                            source_kind @ (LoweredIntegerBinaryKind::ExactAdd
                            | LoweredIntegerBinaryKind::ExactSubtract
                            | LoweredIntegerBinaryKind::ExactMultiply),
                        scalar_type: ScalarType::Integer(integer_type),
                        left,
                        right,
                    } = source_expression
                    else {
                        return None;
                    };
                    let literal = signed_exact_multiply_landed_value(*integer_type, right)?;
                    if !native_fixed_integer_type(*integer_type)
                        || source_type.is_some_and(|source_type| source_type != *integer_type)
                    {
                        return None;
                    }
                    (source_coefficient, source_offset) = checked_lowered_signed_affine_step(
                        source_coefficient,
                        source_offset,
                        *source_kind,
                        literal,
                    );
                    source_saw_offset |= matches!(
                        source_kind,
                        LoweredIntegerBinaryKind::ExactAdd
                            | LoweredIntegerBinaryKind::ExactSubtract
                    );
                    source_saw_negative_factor |=
                        *source_kind == LoweredIntegerBinaryKind::ExactMultiply && literal < 0;
                    source_type = Some(*integer_type);
                    match left.as_ref() {
                        nested @ LoweredDirectExpression::IntegerBinary {
                            kind:
                                LoweredIntegerBinaryKind::ExactAdd
                                | LoweredIntegerBinaryKind::ExactSubtract
                                | LoweredIntegerBinaryKind::ExactMultiply,
                            ..
                        } => source_expression = nested,
                        LoweredDirectExpression::Parameter {
                            position,
                            scalar_type: ScalarType::Integer(root_type),
                        } if source_coefficient.is_some()
                            && source_offset.is_some()
                            && Some(*root_type) == source_type
                            && *root_type != *cast_target_type
                            && ((source_saw_offset && source_saw_negative_factor)
                                || (!source_saw_negative_factor
                                    && target_saw_offset
                                    && target_saw_negative_factor)) =>
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

pub(super) fn checked_lowered_signed_pair_multiply(
    left: Option<(bool, u128)>,
    right: (bool, u128),
) -> Option<(bool, u128)> {
    let left = left?;
    if left.1 == 0 || right.1 == 0 {
        return Some((false, 0));
    }
    Some((left.0 ^ right.0, left.1.checked_mul(right.1)?))
}

pub(super) fn checked_lowered_signed_pair_add(
    left: Option<(bool, u128)>,
    right: (bool, u128),
) -> Option<(bool, u128)> {
    let left = left?;
    Some(match (left, right) {
        ((false, left), (false, right)) => (false, left.checked_add(right)?),
        ((true, left), (true, right)) => (true, left.checked_add(right)?),
        ((false, left), (true, right)) if left >= right => (false, left - right),
        ((false, left), (true, right)) => (true, right - left),
        ((true, left), (false, right)) if right >= left => (false, right - left),
        ((true, left), (false, right)) => (true, left - right),
    })
}

pub(super) fn lowered_signed_pair(value: i128) -> (bool, u128) {
    let magnitude = value.unsigned_abs();
    (value < 0 && magnitude != 0, magnitude)
}

pub(super) fn negated_lowered_signed_pair((negative, magnitude): (bool, u128)) -> (bool, u128) {
    (magnitude != 0 && !negative, magnitude)
}

pub(super) fn checked_lowered_affine_pair_step(
    coefficient: Option<(bool, u128)>,
    offset: Option<(bool, u128)>,
    kind: LoweredIntegerBinaryKind,
    literal: (bool, u128),
) -> (Option<(bool, u128)>, Option<(bool, u128)>) {
    let (Some(coefficient), Some(offset)) = (coefficient, offset) else {
        return (None, None);
    };
    let (nested_coefficient, nested_offset) = match kind {
        LoweredIntegerBinaryKind::ExactAdd => ((false, 1), literal),
        LoweredIntegerBinaryKind::ExactSubtract => {
            ((false, 1), negated_lowered_signed_pair(literal))
        }
        LoweredIntegerBinaryKind::ExactMultiply => (literal, (false, 0)),
        _ => return (None, None),
    };
    let nested_offset = checked_lowered_signed_pair_multiply(Some(nested_offset), coefficient);
    (
        checked_lowered_signed_pair_multiply(Some(coefficient), nested_coefficient),
        checked_lowered_signed_pair_add(nested_offset, offset),
    )
}

pub(super) fn checked_lowered_signed_affine_step(
    coefficient: Option<(bool, u128)>,
    offset: Option<(bool, u128)>,
    kind: LoweredIntegerBinaryKind,
    literal: i128,
) -> (Option<(bool, u128)>, Option<(bool, u128)>) {
    checked_lowered_affine_pair_step(coefficient, offset, kind, lowered_signed_pair(literal))
}

pub(super) fn lowered_affine_landed_literal_pair(
    integer_type: IntegerType,
    expression: &LoweredDirectExpression,
) -> Option<(bool, u128)> {
    let LoweredDirectExpression::IntegerLiteral { value, scalar_type } = expression else {
        return None;
    };
    if !native_fixed_integer_type(integer_type) || *scalar_type != ScalarType::Integer(integer_type)
    {
        return None;
    }
    match (integer_type.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => Some(lowered_signed_pair(*value)),
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => Some((false, *value)),
        _ => None,
    }
}

pub(super) fn shared_exact_affine_fork_branch_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<(IntegerType, usize, (bool, u128), (bool, u128))> {
    let mut branch_type = None;
    let mut coefficient = Some((false, 1_u128));
    let mut offset = Some((false, 0_u128));
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
            || branch_type.is_some_and(|branch_type| branch_type != *integer_type)
        {
            return None;
        }
        let literal = lowered_affine_landed_literal_pair(*integer_type, right)?;
        (coefficient, offset) =
            checked_lowered_affine_pair_step(coefficient, offset, *kind, literal);
        branch_type = Some(*integer_type);
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
            } if Some(*root_type) == branch_type => {
                return Some((*root_type, *position, coefficient?, offset?));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_affine_fork_join_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let LoweredDirectExpression::IntegerBinary {
        kind:
            outer_kind @ (LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract),
        scalar_type: ScalarType::Integer(integer_type),
        left,
        right,
    } = expression
    else {
        return None;
    };
    let (left_type, left_root, left_coefficient, left_offset) =
        shared_exact_affine_fork_branch_parameters(left)?;
    let (right_type, right_root, mut right_coefficient, mut right_offset) =
        shared_exact_affine_fork_branch_parameters(right)?;
    if left_type != *integer_type || right_type != *integer_type || left_root != right_root {
        return None;
    }
    if *outer_kind == LoweredIntegerBinaryKind::ExactSubtract {
        right_coefficient = negated_lowered_signed_pair(right_coefficient);
        right_offset = negated_lowered_signed_pair(right_offset);
    }
    checked_lowered_signed_pair_add(Some(left_coefficient), right_coefficient)?;
    checked_lowered_signed_pair_add(Some(left_offset), right_offset)?;
    Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
        left_root,
    )]))
}

pub(super) fn shared_exact_distinct_root_affine_fork_join_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let LoweredDirectExpression::IntegerBinary {
        kind: LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract,
        scalar_type: ScalarType::Integer(integer_type),
        left,
        right,
    } = expression
    else {
        return None;
    };
    let (left_type, left_root, _, _) = shared_exact_affine_fork_branch_parameters(left)?;
    let (right_type, right_root, _, _) = shared_exact_affine_fork_branch_parameters(right)?;
    if left_type != *integer_type || right_type != *integer_type || left_root == right_root {
        return None;
    }
    Some(BTreeSet::from([
        SharedBooleanRuntimeInput::IntegerScalar(left_root),
        SharedBooleanRuntimeInput::IntegerScalar(right_root),
    ]))
}

pub(super) fn shared_exact_distinct_root_affine_product_join_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let LoweredDirectExpression::IntegerBinary {
        kind: LoweredIntegerBinaryKind::ExactMultiply,
        scalar_type: ScalarType::Integer(integer_type),
        left,
        right,
    } = expression
    else {
        return None;
    };
    if integer_type.sign() != IntegerSign::Signed {
        return None;
    }
    let (left_type, left_root, _, _) = shared_exact_affine_fork_branch_parameters(left)?;
    let (right_type, right_root, _, _) = shared_exact_affine_fork_branch_parameters(right)?;
    if left_type != *integer_type || right_type != *integer_type || left_root == right_root {
        return None;
    }
    Some(BTreeSet::from([
        SharedBooleanRuntimeInput::IntegerScalar(left_root),
        SharedBooleanRuntimeInput::IntegerScalar(right_root),
    ]))
}

pub(super) fn shared_exact_same_root_affine_product_join_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let LoweredDirectExpression::IntegerBinary {
        kind: LoweredIntegerBinaryKind::ExactMultiply,
        scalar_type: ScalarType::Integer(integer_type),
        left,
        right,
    } = expression
    else {
        return None;
    };
    if integer_type.sign() != IntegerSign::Signed {
        return None;
    }
    let (left_type, left_root, left_coefficient, _) =
        shared_exact_affine_fork_branch_parameters(left)?;
    let (right_type, right_root, right_coefficient, _) =
        shared_exact_affine_fork_branch_parameters(right)?;
    if left_type != *integer_type
        || right_type != *integer_type
        || left_root != right_root
        || left_coefficient.1 == 0
        || right_coefficient.1 == 0
    {
        return None;
    }
    Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
        left_root,
    )]))
}

pub(super) fn shared_exact_same_root_affine_divide_remainder_join_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let LoweredDirectExpression::IntegerBinary {
        kind: LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
        scalar_type: ScalarType::Integer(integer_type),
        left,
        right,
    } = expression
    else {
        return None;
    };
    if integer_type.sign() != IntegerSign::Signed || !native_fixed_integer_type(*integer_type) {
        return None;
    }
    let (left_type, left_root, left_coefficient, _) =
        shared_exact_affine_fork_branch_parameters(left)?;
    let (right_type, right_root, right_coefficient, _) =
        shared_exact_affine_fork_branch_parameters(right)?;
    if left_type != *integer_type
        || right_type != *integer_type
        || left_root != right_root
        || left_coefficient.1 == 0
        || right_coefficient.1 == 0
    {
        return None;
    }
    Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
        left_root,
    )]))
}

pub(super) fn shared_exact_signed_affine_chain_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut coefficient = Some((false, 1_u128));
    let mut offset = Some((false, 0_u128));
    let mut saw_offset = false;
    let mut saw_negative_factor = false;
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
        let literal = signed_exact_multiply_landed_value(*integer_type, right)?;
        if !native_fixed_integer_type(*integer_type)
            || chain_type.is_some_and(|chain_type| chain_type != *integer_type)
        {
            return None;
        }
        (coefficient, offset) =
            checked_lowered_signed_affine_step(coefficient, offset, *kind, literal);
        saw_offset |= matches!(
            kind,
            LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract
        );
        saw_negative_factor |= *kind == LoweredIntegerBinaryKind::ExactMultiply && literal < 0;
        chain_type = Some(*integer_type);
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
            } if coefficient.is_some()
                && offset.is_some()
                && saw_offset
                && saw_negative_factor
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

pub(super) fn shared_exact_signed_affine_chain_cast_runtime_parameters(
    target_type: ScalarType,
    operand: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(target_type) = target_type else {
        return None;
    };
    let LoweredDirectExpression::IntegerBinary {
        scalar_type: ScalarType::Integer(source_type),
        ..
    } = operand
    else {
        return None;
    };
    if !native_fixed_integer_type(target_type) || *source_type == target_type {
        return None;
    }
    shared_exact_signed_affine_chain_runtime_parameters(operand)
}

pub(super) fn shared_exact_cast_then_signed_affine_runtime_parameters(
    mut expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut coefficient = Some((false, 1_u128));
    let mut offset = Some((false, 0_u128));
    let mut saw_offset = false;
    let mut saw_negative_factor = false;
    loop {
        let LoweredDirectExpression::IntegerBinary {
            kind:
                kind @ (LoweredIntegerBinaryKind::ExactAdd
                | LoweredIntegerBinaryKind::ExactSubtract
                | LoweredIntegerBinaryKind::ExactMultiply),
            scalar_type: ScalarType::Integer(target_type),
            left,
            right,
        } = expression
        else {
            return None;
        };
        let literal = signed_exact_multiply_landed_value(*target_type, right)?;
        if !native_fixed_integer_type(*target_type)
            || chain_type.is_some_and(|chain_type| chain_type != *target_type)
        {
            return None;
        }
        (coefficient, offset) =
            checked_lowered_signed_affine_step(coefficient, offset, *kind, literal);
        saw_offset |= matches!(
            kind,
            LoweredIntegerBinaryKind::ExactAdd | LoweredIntegerBinaryKind::ExactSubtract
        );
        saw_negative_factor |= *kind == LoweredIntegerBinaryKind::ExactMultiply && literal < 0;
        chain_type = Some(*target_type);
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
            } if coefficient.is_some()
                && offset.is_some()
                && saw_offset
                && saw_negative_factor
                && *cast_target_type == *target_type =>
            {
                let LoweredDirectExpression::Parameter {
                    position,
                    scalar_type: ScalarType::Integer(source_type),
                } = operand.as_ref()
                else {
                    return None;
                };
                return (native_fixed_integer_type(*source_type) && *source_type != *target_type)
                    .then(|| {
                        BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)])
                    });
            }
            _ => return None,
        }
    }
}
