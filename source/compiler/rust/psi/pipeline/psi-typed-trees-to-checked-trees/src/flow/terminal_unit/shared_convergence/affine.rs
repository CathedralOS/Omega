//! Additive, affine, and affine-join sufficient forms.

use super::*;

pub(super) fn shared_exact_add_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_add = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactAdd,
            primitive_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type) {
            return None;
        }
        chain_type = Some(*primitive_type);
        let left_is_literal = matches!(
            left.as_ref(),
            CheckedScalarExpression::IntegerLiteral { .. }
        );
        let right_is_literal = matches!(
            right.as_ref(),
            CheckedScalarExpression::IntegerLiteral { .. }
        );
        match (left.as_ref(), right.as_ref()) {
            (
                nested @ CheckedScalarExpression::IntegerBinary {
                    kind: CheckedIntegerBinaryKind::ExactAdd,
                    ..
                },
                _,
            ) if right_is_literal => {
                saw_nested_add = true;
                expression = nested;
            }
            (
                _,
                nested @ CheckedScalarExpression::IntegerBinary {
                    kind: CheckedIntegerBinaryKind::ExactAdd,
                    ..
                },
            ) if left_is_literal => {
                saw_nested_add = true;
                expression = nested;
            }
            _ if saw_nested_add && (left_is_literal || right_is_literal) => {
                let mut inputs = shared_integer_runtime_inputs_with_shells(
                    left,
                    scalar_parameter_count,
                    0,
                    false,
                )?;
                inputs.extend(shared_integer_runtime_inputs_with_shells(
                    right,
                    scalar_parameter_count,
                    0,
                    false,
                )?);
                return Some(inputs);
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_subtract_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_nested_subtract = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactSubtract,
            primitive_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type)
            || !matches!(
                right.as_ref(),
                CheckedScalarExpression::IntegerLiteral { .. }
            )
        {
            return None;
        }
        chain_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactSubtract,
                ..
            } => {
                saw_nested_subtract = true;
                expression = nested;
            }
            _ if saw_nested_subtract => {
                let mut inputs = shared_integer_runtime_inputs_with_shells(
                    left,
                    scalar_parameter_count,
                    0,
                    false,
                )?;
                inputs.extend(shared_integer_runtime_inputs_with_shells(
                    right,
                    scalar_parameter_count,
                    0,
                    false,
                )?);
                return Some(inputs);
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_mixed_add_subtract_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_add = false;
    let mut saw_subtract = false;
    let mut saw_nested_operation = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind:
                kind @ (CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract),
            primitive_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type)
            || !exact_offset_landed_literal(*primitive_type, right)
        {
            return None;
        }
        chain_type = Some(*primitive_type);
        saw_add |= *kind == CheckedIntegerBinaryKind::ExactAdd;
        saw_subtract |= *kind == CheckedIntegerBinaryKind::ExactSubtract;
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract,
                ..
            } => {
                saw_nested_operation = true;
                expression = nested;
            }
            CheckedScalarExpression::Parameter {
                position,
                primitive_type: root_type,
            } if saw_nested_operation
                && saw_add
                && saw_subtract
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

pub(super) fn exact_offset_landed_literal(
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
        (PrimitiveType::I8, Some(psi_numerics::literals::LandedIntegerType::I8))
        | (PrimitiveType::I16, Some(psi_numerics::literals::LandedIntegerType::I16))
        | (PrimitiveType::I32, Some(psi_numerics::literals::LandedIntegerType::I32))
        | (PrimitiveType::I64, Some(psi_numerics::literals::LandedIntegerType::I64)) => {
            literal.value_i64().is_some()
        }
        (PrimitiveType::U8, Some(psi_numerics::literals::LandedIntegerType::U8))
        | (PrimitiveType::U16, Some(psi_numerics::literals::LandedIntegerType::U16))
        | (PrimitiveType::U32, Some(psi_numerics::literals::LandedIntegerType::U32))
        | (PrimitiveType::U64, Some(psi_numerics::literals::LandedIntegerType::U64)) => {
            literal.value_u64().is_some()
        }
        _ => false,
    }
}

#[cfg(test)]
pub(crate) fn exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_mixed_add_subtract_chain_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn shared_exact_affine_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_offset = false;
    let mut saw_multiply = false;
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
        saw_offset |= matches!(
            kind,
            CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract
        );
        saw_multiply |= *kind == CheckedIntegerBinaryKind::ExactMultiply;
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
            } if saw_offset
                && saw_multiply
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

pub(super) fn checked_signed_pair_multiply(
    left: Option<(bool, u128)>,
    right: (bool, u128),
) -> Option<(bool, u128)> {
    let left = left?;
    if left.1 == 0 || right.1 == 0 {
        return Some((false, 0));
    }
    Some((left.0 ^ right.0, left.1.checked_mul(right.1)?))
}

pub(super) fn checked_signed_pair_add(
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

pub(super) fn signed_pair(value: i64) -> (bool, u128) {
    let magnitude = u128::from(value.unsigned_abs());
    (value < 0 && magnitude != 0, magnitude)
}

pub(super) fn negated_signed_pair((negative, magnitude): (bool, u128)) -> (bool, u128) {
    (magnitude != 0 && !negative, magnitude)
}

pub(super) fn checked_affine_pair_step(
    coefficient: Option<(bool, u128)>,
    offset: Option<(bool, u128)>,
    kind: CheckedIntegerBinaryKind,
    literal: (bool, u128),
) -> (Option<(bool, u128)>, Option<(bool, u128)>) {
    let (Some(coefficient), Some(offset)) = (coefficient, offset) else {
        return (None, None);
    };
    let (nested_coefficient, nested_offset) = match kind {
        CheckedIntegerBinaryKind::ExactAdd => ((false, 1), literal),
        CheckedIntegerBinaryKind::ExactSubtract => ((false, 1), negated_signed_pair(literal)),
        CheckedIntegerBinaryKind::ExactMultiply => (literal, (false, 0)),
        _ => return (None, None),
    };
    let nested_offset = checked_signed_pair_multiply(Some(nested_offset), coefficient);
    (
        checked_signed_pair_multiply(Some(coefficient), nested_coefficient),
        checked_signed_pair_add(nested_offset, offset),
    )
}

pub(super) fn checked_signed_affine_step(
    coefficient: Option<(bool, u128)>,
    offset: Option<(bool, u128)>,
    kind: CheckedIntegerBinaryKind,
    literal: i64,
) -> (Option<(bool, u128)>, Option<(bool, u128)>) {
    checked_affine_pair_step(coefficient, offset, kind, signed_pair(literal))
}

pub(super) fn checked_affine_landed_literal_pair(
    primitive_type: PrimitiveType,
    expression: &CheckedScalarExpression,
) -> Option<(bool, u128)> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    match (
        primitive_type,
        literal.landing().map(|landing| landing.landed_type),
    ) {
        (PrimitiveType::I8, Some(psi_numerics::literals::LandedIntegerType::I8))
        | (PrimitiveType::I16, Some(psi_numerics::literals::LandedIntegerType::I16))
        | (PrimitiveType::I32, Some(psi_numerics::literals::LandedIntegerType::I32))
        | (PrimitiveType::I64, Some(psi_numerics::literals::LandedIntegerType::I64)) => {
            literal.value_i64().map(signed_pair)
        }
        (PrimitiveType::U8, Some(psi_numerics::literals::LandedIntegerType::U8))
        | (PrimitiveType::U16, Some(psi_numerics::literals::LandedIntegerType::U16))
        | (PrimitiveType::U32, Some(psi_numerics::literals::LandedIntegerType::U32))
        | (PrimitiveType::U64, Some(psi_numerics::literals::LandedIntegerType::U64)) => {
            literal.value_u64().map(|value| (false, u128::from(value)))
        }
        _ => None,
    }
}

pub(super) fn shared_exact_affine_fork_branch(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<(PrimitiveType, usize, (bool, u128), (bool, u128))> {
    let mut branch_type = None;
    let mut coefficient = Some((false, 1_u128));
    let mut offset = Some((false, 0_u128));
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
        if branch_type.is_some_and(|branch_type| branch_type != *primitive_type) {
            return None;
        }
        let literal = checked_affine_landed_literal_pair(*primitive_type, right)?;
        (coefficient, offset) = checked_affine_pair_step(coefficient, offset, *kind, literal);
        branch_type = Some(*primitive_type);
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
            } if Some(*root_type) == branch_type && *position < scalar_parameter_count => {
                return Some((*root_type, *position, coefficient?, offset?));
            }
            _ => return None,
        }
    }
}

pub(super) fn shared_exact_affine_fork_join_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let CheckedScalarExpression::IntegerBinary {
        kind:
            outer_kind @ (CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract),
        primitive_type,
        left,
        right,
    } = expression
    else {
        return None;
    };
    let (left_type, left_root, left_coefficient, left_offset) =
        shared_exact_affine_fork_branch(left, scalar_parameter_count)?;
    let (right_type, right_root, mut right_coefficient, mut right_offset) =
        shared_exact_affine_fork_branch(right, scalar_parameter_count)?;
    if left_type != *primitive_type || right_type != *primitive_type || left_root != right_root {
        return None;
    }
    if *outer_kind == CheckedIntegerBinaryKind::ExactSubtract {
        right_coefficient = negated_signed_pair(right_coefficient);
        right_offset = negated_signed_pair(right_offset);
    }
    checked_signed_pair_add(Some(left_coefficient), right_coefficient)?;
    checked_signed_pair_add(Some(left_offset), right_offset)?;
    Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
        left_root,
    )]))
}

#[cfg(test)]
pub(crate) fn exact_affine_fork_join_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_affine_fork_join_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn shared_exact_distinct_root_affine_fork_join_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract,
        primitive_type,
        left,
        right,
    } = expression
    else {
        return None;
    };
    let (left_type, left_root, _, _) =
        shared_exact_affine_fork_branch(left, scalar_parameter_count)?;
    let (right_type, right_root, _, _) =
        shared_exact_affine_fork_branch(right, scalar_parameter_count)?;
    if left_type != *primitive_type || right_type != *primitive_type || left_root == right_root {
        return None;
    }
    Some(BTreeSet::from([
        SharedBooleanRuntimeInput::IntegerScalar(left_root),
        SharedBooleanRuntimeInput::IntegerScalar(right_root),
    ]))
}

pub(super) fn shared_exact_distinct_root_affine_product_join_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactMultiply,
        primitive_type:
            primitive_type @ (PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64),
        left,
        right,
    } = expression
    else {
        return None;
    };
    let (left_type, left_root, _, _) =
        shared_exact_affine_fork_branch(left, scalar_parameter_count)?;
    let (right_type, right_root, _, _) =
        shared_exact_affine_fork_branch(right, scalar_parameter_count)?;
    if left_type != *primitive_type || right_type != *primitive_type || left_root == right_root {
        return None;
    }
    Some(BTreeSet::from([
        SharedBooleanRuntimeInput::IntegerScalar(left_root),
        SharedBooleanRuntimeInput::IntegerScalar(right_root),
    ]))
}

pub(super) fn shared_exact_same_root_affine_product_join_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactMultiply,
        primitive_type:
            primitive_type @ (PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64),
        left,
        right,
    } = expression
    else {
        return None;
    };
    let (left_type, left_root, left_coefficient, _) =
        shared_exact_affine_fork_branch(left, scalar_parameter_count)?;
    let (right_type, right_root, right_coefficient, _) =
        shared_exact_affine_fork_branch(right, scalar_parameter_count)?;
    if left_type != *primitive_type
        || right_type != *primitive_type
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

pub(super) fn shared_exact_same_root_affine_divide_remainder_join_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let CheckedScalarExpression::IntegerBinary {
        kind: CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder,
        primitive_type:
            primitive_type @ (PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64),
        left,
        right,
    } = expression
    else {
        return None;
    };
    let (left_type, left_root, left_coefficient, _) =
        shared_exact_affine_fork_branch(left, scalar_parameter_count)?;
    let (right_type, right_root, right_coefficient, _) =
        shared_exact_affine_fork_branch(right, scalar_parameter_count)?;
    if left_type != *primitive_type
        || right_type != *primitive_type
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

#[cfg(test)]
pub(crate) fn exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_same_root_affine_divide_remainder_join_runtime_inputs(
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
pub(crate) fn exact_same_root_affine_product_join_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_same_root_affine_product_join_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_distinct_root_affine_product_join_runtime_inputs(
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
pub(crate) fn exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_distinct_root_affine_fork_join_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn shared_exact_signed_affine_chain_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut coefficient = Some((false, 1_u128));
    let mut offset = Some((false, 0_u128));
    let mut saw_offset = false;
    let mut saw_negative_factor = false;
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
        let literal = signed_exact_multiply_literal_value(*primitive_type, right)?;
        if chain_type.is_some_and(|chain_type| chain_type != *primitive_type) {
            return None;
        }
        (coefficient, offset) = checked_signed_affine_step(coefficient, offset, *kind, literal);
        saw_offset |= matches!(
            kind,
            CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract
        );
        saw_negative_factor |= *kind == CheckedIntegerBinaryKind::ExactMultiply && literal < 0;
        chain_type = Some(*primitive_type);
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
            } if coefficient.is_some()
                && offset.is_some()
                && saw_offset
                && saw_negative_factor
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

pub(super) fn shared_exact_signed_affine_chain_cast_runtime_inputs(
    target_type: PrimitiveType,
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let CheckedScalarExpression::IntegerBinary {
        primitive_type: source_type,
        ..
    } = expression
    else {
        return None;
    };
    if *source_type == target_type
        || !matches!(
            target_type,
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
        )
    {
        return None;
    }
    shared_exact_signed_affine_chain_runtime_inputs(expression, scalar_parameter_count)
}

pub(super) fn shared_exact_cast_then_signed_affine_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut coefficient = Some((false, 1_u128));
    let mut offset = Some((false, 0_u128));
    let mut saw_offset = false;
    let mut saw_negative_factor = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind:
                kind @ (CheckedIntegerBinaryKind::ExactAdd
                | CheckedIntegerBinaryKind::ExactSubtract
                | CheckedIntegerBinaryKind::ExactMultiply),
            primitive_type: target_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        let literal = signed_exact_multiply_literal_value(*target_type, right)?;
        if chain_type.is_some_and(|chain_type| chain_type != *target_type) {
            return None;
        }
        (coefficient, offset) = checked_signed_affine_step(coefficient, offset, *kind, literal);
        saw_offset |= matches!(
            kind,
            CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract
        );
        saw_negative_factor |= *kind == CheckedIntegerBinaryKind::ExactMultiply && literal < 0;
        chain_type = Some(*target_type);
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
            } if coefficient.is_some()
                && offset.is_some()
                && saw_offset
                && saw_negative_factor
                && *cast_target_type == *target_type =>
            {
                let CheckedScalarExpression::Parameter {
                    position,
                    primitive_type: source_type,
                } = operand.as_ref()
                else {
                    return None;
                };
                return (*source_type != *target_type && *position < scalar_parameter_count).then(
                    || BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)]),
                );
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
pub(crate) fn exact_signed_affine_chain_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_signed_affine_chain_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_signed_affine_chain_cast_runtime_parameter_positions_for_test(
    target_type: PrimitiveType,
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_signed_affine_chain_cast_runtime_inputs(
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
pub(crate) fn exact_cast_then_signed_affine_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_then_signed_affine_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn shared_exact_shift_then_arithmetic_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut value_type = None;
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
        if value_type.is_some_and(|value_type| value_type != *primitive_type)
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
        value_type = Some(*primitive_type);
        match left.as_ref() {
            nested @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactAdd
                    | CheckedIntegerBinaryKind::ExactSubtract
                    | CheckedIntegerBinaryKind::ExactMultiply,
                ..
            } => expression = nested,
            shift @ CheckedScalarExpression::IntegerBinary {
                kind:
                    CheckedIntegerBinaryKind::ExactShiftLeft | CheckedIntegerBinaryKind::ExactShiftRight,
                ..
            } => {
                expression = shift;
                break;
            }
            _ => return None,
        }
    }

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
        if Some(*primitive_type) != value_type
            || landed_exact_shift_literal_count(*primitive_type, right).is_none()
        {
            return None;
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
pub(crate) fn exact_shift_then_arithmetic_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_shift_then_arithmetic_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

pub(super) fn shared_exact_cast_then_affine_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut chain_type = None;
    let mut saw_offset = false;
    let mut saw_multiply = false;
    loop {
        let CheckedScalarExpression::IntegerBinary {
            kind:
                kind @ (CheckedIntegerBinaryKind::ExactAdd
                | CheckedIntegerBinaryKind::ExactSubtract
                | CheckedIntegerBinaryKind::ExactMultiply),
            primitive_type: target_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        if chain_type.is_some_and(|chain_type| chain_type != *target_type)
            || match kind {
                CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract => {
                    !exact_offset_landed_literal(*target_type, right)
                }
                CheckedIntegerBinaryKind::ExactMultiply => {
                    !nonnegative_exact_multiply_literal(*target_type, right)
                }
                _ => unreachable!("matched one exact affine operation"),
            }
        {
            return None;
        }
        chain_type = Some(*target_type);
        saw_offset |= matches!(
            kind,
            CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract
        );
        saw_multiply |= *kind == CheckedIntegerBinaryKind::ExactMultiply;
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
            } if saw_offset && saw_multiply && *cast_target_type == *target_type => {
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

pub(super) fn shared_exact_affine_cast_affine_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
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
                            source_kind @ (CheckedIntegerBinaryKind::ExactAdd
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
                        || match source_kind {
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

pub(super) fn shared_exact_signed_affine_cast_affine_runtime_inputs(
    mut expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let mut target_type = None;
    let mut target_coefficient = Some((false, 1_u128));
    let mut target_offset = Some((false, 0_u128));
    let mut target_saw_offset = false;
    let mut target_saw_negative_factor = false;
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
        let literal = signed_exact_multiply_literal_value(*primitive_type, right)?;
        if target_type.is_some_and(|target_type| target_type != *primitive_type) {
            return None;
        }
        (target_coefficient, target_offset) =
            checked_signed_affine_step(target_coefficient, target_offset, *kind, literal);
        target_saw_offset |= matches!(
            kind,
            CheckedIntegerBinaryKind::ExactAdd | CheckedIntegerBinaryKind::ExactSubtract
        );
        target_saw_negative_factor |=
            *kind == CheckedIntegerBinaryKind::ExactMultiply && literal < 0;
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
                    let CheckedScalarExpression::IntegerBinary {
                        kind:
                            source_kind @ (CheckedIntegerBinaryKind::ExactAdd
                            | CheckedIntegerBinaryKind::ExactSubtract
                            | CheckedIntegerBinaryKind::ExactMultiply),
                        primitive_type,
                        left,
                        right,
                    } = source_expression
                    else {
                        return None;
                    };
                    let literal = signed_exact_multiply_literal_value(*primitive_type, right)?;
                    if source_type.is_some_and(|source_type| source_type != *primitive_type) {
                        return None;
                    }
                    (source_coefficient, source_offset) = checked_signed_affine_step(
                        source_coefficient,
                        source_offset,
                        *source_kind,
                        literal,
                    );
                    source_saw_offset |= matches!(
                        source_kind,
                        CheckedIntegerBinaryKind::ExactAdd
                            | CheckedIntegerBinaryKind::ExactSubtract
                    );
                    source_saw_negative_factor |=
                        *source_kind == CheckedIntegerBinaryKind::ExactMultiply && literal < 0;
                    source_type = Some(*primitive_type);
                    match left.as_ref() {
                        nested @ CheckedScalarExpression::IntegerBinary {
                            kind:
                                CheckedIntegerBinaryKind::ExactAdd
                                | CheckedIntegerBinaryKind::ExactSubtract
                                | CheckedIntegerBinaryKind::ExactMultiply,
                            ..
                        } => source_expression = nested,
                        CheckedScalarExpression::Parameter {
                            position,
                            primitive_type: root_type,
                        } if source_coefficient.is_some()
                            && source_offset.is_some()
                            && Some(*root_type) == source_type
                            && *root_type != *cast_target_type
                            && *position < scalar_parameter_count
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

#[cfg(test)]
pub(crate) fn exact_affine_cast_affine_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_affine_cast_affine_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_signed_affine_cast_affine_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_signed_affine_cast_affine_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_cast_then_affine_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_cast_then_affine_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn exact_affine_chain_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_exact_affine_chain_runtime_inputs(expression, scalar_parameter_count)?
        .into_iter()
        .map(|input| match input {
            SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
            _ => None,
        })
        .collect()
}
