//! Closed sufficient-form classifiers for shared Boolean convergence.
//!
//! These routines recognize source expression shapes and retain their exact
//! runtime parameter roots. They are an untrusted producer convenience: the
//! terminal verifier still reconstructs every admitted obligation.

use super::*;

pub(super) fn checked_shared_boolean_convergence(
    facts: &CheckFacts,
    state: SymbolHandle,
    bindings: &[CheckedScalarBinding],
    return_expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    cleanup_actions: &[CheckedStructuralScalarReturnCleanupAction],
) -> Option<psi_checked_trees::CheckedStructuralBooleanConvergencePlan> {
    let [binding] = bindings else {
        return None;
    };
    if binding.statement_ordinal != 0 || binding.primitive_type != PrimitiveType::Bool {
        return None;
    }
    let expression = facts.values.scalar_expressions.expression_at(
        state,
        0,
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
    )?;
    let CheckedScalarExpression::Boolean(expression) = expression else {
        return None;
    };
    let runtime_inputs = shared_boolean_runtime_inputs(expression, scalar_parameter_count)?;
    let structural_fields = runtime_inputs
        .iter()
        .filter_map(|input| match input {
            SharedBooleanRuntimeInput::StructuralField {
                parameter_position,
                field,
            } => Some((*parameter_position, field)),
            SharedBooleanRuntimeInput::BooleanScalar(_)
            | SharedBooleanRuntimeInput::IntegerScalar(_) => None,
        })
        .collect::<Vec<_>>();
    let has_boolean_scalar_input = runtime_inputs
        .iter()
        .any(|input| matches!(input, SharedBooleanRuntimeInput::BooleanScalar(_)));
    let has_integer_scalar_input = runtime_inputs
        .iter()
        .any(|input| matches!(input, SharedBooleanRuntimeInput::IntegerScalar(_)));
    if structural_fields.len() > 1
        || (!structural_fields.is_empty() && !has_boolean_scalar_input)
        || (!structural_fields.is_empty() && has_integer_scalar_input)
        || structural_fields.first().is_some_and(|(position, _)| {
            !cleanup_actions.iter().any(|action| {
                matches!(
                    action,
                    CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)
                        if cleanup.source_parameter_index == *position
                )
            })
        })
    {
        return None;
    }
    if !checked_boolean_contains_short_circuit(expression)
        || runtime_inputs.is_empty()
        || !matches!(
            return_expression,
            CheckedScalarExpression::Boolean(expression)
                if matches!(expression.as_ref(),
                    psi_checked_trees::CheckedBooleanExpression::Local { position }
                        if *position == scalar_parameter_count)
        )
    {
        return None;
    }
    Some(psi_checked_trees::CheckedStructuralBooleanConvergencePlan { binding_ordinal: 0 })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SharedBooleanRuntimeInput {
    BooleanScalar(usize),
    IntegerScalar(usize),
    StructuralField {
        parameter_position: u32,
        field: String,
    },
}

type SharedIntegerRuntimeInputs = BTreeSet<SharedBooleanRuntimeInput>;
type SharedIntegerClassifier =
    fn(&CheckedScalarExpression, usize) -> Option<SharedIntegerRuntimeInputs>;
type SharedExactCastClassifier =
    fn(PrimitiveType, &CheckedScalarExpression, usize) -> Option<SharedIntegerRuntimeInputs>;

const EXACT_ADD_CLASSIFIERS: &[SharedIntegerClassifier] = &[
    shared_exact_shift_then_arithmetic_runtime_inputs,
    shared_exact_divide_remainder_cross_chain_runtime_inputs,
    shared_exact_divide_remainder_cross_cast_runtime_inputs,
    shared_exact_affine_shift_cast_sandwich_runtime_inputs,
    shared_exact_affine_cast_affine_runtime_inputs,
    shared_exact_signed_affine_cast_affine_runtime_inputs,
    shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_then_affine_runtime_inputs,
    shared_exact_cast_then_signed_affine_runtime_inputs,
    shared_exact_cast_then_offset_runtime_inputs,
    shared_exact_add_chain_runtime_inputs,
    shared_exact_mixed_add_subtract_chain_runtime_inputs,
    shared_exact_affine_chain_runtime_inputs,
    shared_exact_signed_affine_chain_runtime_inputs,
    shared_exact_affine_fork_join_runtime_inputs,
    shared_exact_distinct_root_affine_fork_join_runtime_inputs,
];

const EXACT_SUBTRACT_CLASSIFIERS: &[SharedIntegerClassifier] = &[
    shared_exact_shift_then_arithmetic_runtime_inputs,
    shared_exact_divide_remainder_cross_chain_runtime_inputs,
    shared_exact_divide_remainder_cross_cast_runtime_inputs,
    shared_exact_affine_shift_cast_sandwich_runtime_inputs,
    shared_exact_affine_cast_affine_runtime_inputs,
    shared_exact_signed_affine_cast_affine_runtime_inputs,
    shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_then_affine_runtime_inputs,
    shared_exact_cast_then_signed_affine_runtime_inputs,
    shared_exact_cast_then_offset_runtime_inputs,
    shared_exact_subtract_chain_runtime_inputs,
    shared_exact_mixed_add_subtract_chain_runtime_inputs,
    shared_exact_affine_chain_runtime_inputs,
    shared_exact_signed_affine_chain_runtime_inputs,
    shared_exact_affine_fork_join_runtime_inputs,
    shared_exact_distinct_root_affine_fork_join_runtime_inputs,
];

const EXACT_MULTIPLY_CLASSIFIERS: &[SharedIntegerClassifier] = &[
    shared_exact_shift_then_arithmetic_runtime_inputs,
    shared_exact_divide_remainder_cross_chain_runtime_inputs,
    shared_exact_divide_remainder_cross_cast_runtime_inputs,
    shared_exact_affine_shift_cast_sandwich_runtime_inputs,
    shared_exact_affine_cast_affine_runtime_inputs,
    shared_exact_signed_affine_cast_affine_runtime_inputs,
    shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_then_affine_runtime_inputs,
    shared_exact_cast_then_signed_affine_runtime_inputs,
    shared_exact_cast_then_multiply_runtime_inputs,
    shared_exact_cast_then_signed_multiply_runtime_inputs,
    shared_exact_multiply_chain_runtime_inputs,
    shared_exact_signed_multiply_chain_runtime_inputs,
    shared_exact_affine_chain_runtime_inputs,
    shared_exact_signed_affine_chain_runtime_inputs,
    shared_exact_same_root_affine_product_join_runtime_inputs,
    shared_exact_distinct_root_affine_product_join_runtime_inputs,
];

const EXACT_SHIFT_RIGHT_CLASSIFIERS: &[SharedIntegerClassifier] = &[
    shared_exact_mixed_shift_chain_runtime_inputs,
    shared_exact_divide_remainder_cross_chain_runtime_inputs,
    shared_exact_divide_remainder_cross_cast_runtime_inputs,
    shared_exact_affine_shift_cast_sandwich_runtime_inputs,
    shared_exact_shift_cast_shift_runtime_inputs,
    shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_then_mixed_shift_runtime_inputs,
    shared_exact_arithmetic_then_shift_runtime_inputs,
    shared_exact_cast_then_shift_right_runtime_inputs,
    shared_exact_shift_right_chain_runtime_inputs,
];

const EXACT_DIVIDE_REMAINDER_CLASSIFIERS: &[SharedIntegerClassifier] = &[
    shared_exact_same_root_affine_divide_remainder_join_runtime_inputs,
    shared_exact_divide_remainder_cross_chain_runtime_inputs,
    shared_exact_divide_remainder_cross_cast_runtime_inputs,
    shared_exact_divide_remainder_cast_sandwich_runtime_inputs,
    shared_exact_runtime_divisor_chain_runtime_inputs,
    shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_then_divide_remainder_runtime_inputs,
    shared_exact_divide_remainder_chain_runtime_inputs,
];

const EXACT_SHIFT_LEFT_CLASSIFIERS: &[SharedIntegerClassifier] = &[
    shared_exact_mixed_shift_chain_runtime_inputs,
    shared_exact_divide_remainder_cross_chain_runtime_inputs,
    shared_exact_divide_remainder_cross_cast_runtime_inputs,
    shared_exact_affine_shift_cast_sandwich_runtime_inputs,
    shared_exact_shift_cast_shift_runtime_inputs,
    shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_inputs,
    shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_chain_then_computed_suffix_runtime_inputs,
    shared_exact_cast_then_mixed_shift_runtime_inputs,
    shared_exact_arithmetic_then_shift_runtime_inputs,
    shared_exact_cast_then_shift_left_runtime_inputs,
    shared_exact_shift_left_chain_runtime_inputs,
];

const EXACT_CAST_CLASSIFIERS: &[SharedExactCastClassifier] = &[
    shared_roundtrip_exact_cast_runtime_inputs,
    shared_exact_cast_chain_runtime_inputs,
    shared_exact_computed_prefix_cast_chain_runtime_inputs,
    shared_exact_divide_remainder_chain_cast_runtime_inputs,
    shared_exact_mixed_shift_chain_cast_runtime_inputs,
    shared_exact_shift_right_chain_cast_runtime_inputs,
    shared_exact_shift_left_chain_cast_runtime_inputs,
    shared_exact_affine_chain_cast_runtime_inputs,
    shared_exact_multiply_chain_cast_runtime_inputs,
    shared_exact_signed_multiply_chain_cast_runtime_inputs,
    shared_exact_signed_affine_chain_cast_runtime_inputs,
    shared_exact_offset_chain_cast_runtime_inputs,
];

fn first_shared_integer_classification(
    classifiers: &[SharedIntegerClassifier],
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<SharedIntegerRuntimeInputs> {
    classifiers
        .iter()
        .find_map(|classifier| classifier(expression, scalar_parameter_count))
}

fn first_shared_exact_cast_classification(
    target_type: PrimitiveType,
    operand: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<SharedIntegerRuntimeInputs> {
    EXACT_CAST_CLASSIFIERS
        .iter()
        .find_map(|classifier| classifier(target_type, operand, scalar_parameter_count))
}

fn fixed_native_integer(primitive_type: PrimitiveType) -> bool {
    matches!(
        primitive_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    )
}

fn exact_binary_classifier_registry(
    kind: CheckedIntegerBinaryKind,
    primitive_type: PrimitiveType,
) -> Option<&'static [SharedIntegerClassifier]> {
    match kind {
        CheckedIntegerBinaryKind::ExactAdd => Some(EXACT_ADD_CLASSIFIERS),
        CheckedIntegerBinaryKind::ExactSubtract => Some(EXACT_SUBTRACT_CLASSIFIERS),
        CheckedIntegerBinaryKind::ExactMultiply => Some(EXACT_MULTIPLY_CLASSIFIERS),
        CheckedIntegerBinaryKind::ExactShiftRight if fixed_native_integer(primitive_type) => {
            Some(EXACT_SHIFT_RIGHT_CLASSIFIERS)
        }
        CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder
            if fixed_native_integer(primitive_type) =>
        {
            Some(EXACT_DIVIDE_REMAINDER_CLASSIFIERS)
        }
        CheckedIntegerBinaryKind::ExactShiftLeft if fixed_native_integer(primitive_type) => {
            Some(EXACT_SHIFT_LEFT_CLASSIFIERS)
        }
        _ => None,
    }
}

fn direct_binary_runtime_inputs(
    left: &CheckedScalarExpression,
    right: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<SharedIntegerRuntimeInputs> {
    let mut inputs =
        shared_integer_runtime_inputs_with_shells(left, scalar_parameter_count, 0, false)?;
    inputs.extend(shared_integer_runtime_inputs_with_shells(
        right,
        scalar_parameter_count,
        0,
        false,
    )?);
    Some(inputs)
}

/// Collect the distinct runtime Boolean inputs in the shared-join form. One
/// direct authored member identity on the nominal cleanup root is admitted
/// alongside scalar inputs; terminal production resolves it to one canonical
/// relevant Boolean field. Integer-comparison leaves separately admit scalar
/// parameters and landed constants beneath up to two total binary, bitwise-not,
/// or integer-widening shells, or one proof-bearing exact-cast, exact-add,
/// exact-subtract, exact-multiply, exact shift, exact-divide, or exact-remainder
/// computation shell. The single exact operation may be the additional
/// innermost shell beneath up to two bitwise-not, integer-widening, or
/// proof-free binary shells. Distinct binary subtrees may each contain one
/// independently proved exact leaf. One exact-add result may additionally feed
/// one same-type exact add when the inner operation has a landed constant
/// addend and otherwise direct operands, and the other outer operand is a
/// landed constant. A direct fixed-integer parameter may also pass through a
/// finite chain of valid widenings before an exact narrowing back to its
/// original carrier. One exact narrowing may instead consume a finite
/// left-associated same-carrier exact-add/subtract literal-offset chain rooted
/// at one direct fixed-native parameter.
/// Constants and Boolean equality against a constant add no new runtime input.
fn shared_boolean_runtime_inputs(
    expression: &psi_checked_trees::CheckedBooleanExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::Constant(_) => Some(BTreeSet::new()),
        psi_checked_trees::CheckedBooleanExpression::Parameter { position }
            if *position < scalar_parameter_count =>
        {
            Some(BTreeSet::from([SharedBooleanRuntimeInput::BooleanScalar(
                *position,
            )]))
        }
        psi_checked_trees::CheckedBooleanExpression::Not(operand) => {
            shared_boolean_runtime_inputs(operand, scalar_parameter_count)
        }
        psi_checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            match (left.as_ref(), right.as_ref()) {
                (psi_checked_trees::CheckedBooleanExpression::Constant(_), expression)
                | (expression, psi_checked_trees::CheckedBooleanExpression::Constant(_)) => {
                    shared_boolean_runtime_inputs(expression, scalar_parameter_count)
                }
                _ => None,
            }
        }
        psi_checked_trees::CheckedBooleanExpression::And { left, right }
        | psi_checked_trees::CheckedBooleanExpression::Or { left, right } => {
            let mut parameters = shared_boolean_runtime_inputs(left, scalar_parameter_count)?;
            parameters.extend(shared_boolean_runtime_inputs(
                right,
                scalar_parameter_count,
            )?);
            Some(parameters)
        }
        psi_checked_trees::CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } if path.len() == 1 => Some(BTreeSet::from([
            SharedBooleanRuntimeInput::StructuralField {
                parameter_position: *parameter_position,
                field: path[0].clone(),
            },
        ])),
        psi_checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            let mut inputs = shared_integer_runtime_inputs(left, scalar_parameter_count)?;
            inputs.extend(shared_integer_runtime_inputs(
                right,
                scalar_parameter_count,
            )?);
            Some(inputs)
        }
        psi_checked_trees::CheckedBooleanExpression::Parameter { .. }
        | psi_checked_trees::CheckedBooleanExpression::Local { .. }
        | psi_checked_trees::CheckedBooleanExpression::StructuralParameterField { .. } => None,
    }
}

fn shared_integer_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_integer_runtime_inputs_with_shells(expression, scalar_parameter_count, 2, true)
}

fn shared_integer_runtime_inputs_with_shells(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    remaining_shells: usize,
    proof_shell_allowed: bool,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    match expression {
        CheckedScalarExpression::IntegerLiteral { .. } => Some(BTreeSet::new()),
        CheckedScalarExpression::Parameter { position, .. }
            if *position < scalar_parameter_count =>
        {
            Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                *position,
            )]))
        }
        CheckedScalarExpression::IntegerBinary {
            kind:
                CheckedIntegerBinaryKind::BitwiseAnd
                | CheckedIntegerBinaryKind::BitwiseOr
                | CheckedIntegerBinaryKind::BitwiseXor
                | CheckedIntegerBinaryKind::WrappingShiftLeft
                | CheckedIntegerBinaryKind::WrappingShiftRight
                | CheckedIntegerBinaryKind::WrappingAdd
                | CheckedIntegerBinaryKind::SaturatingAdd
                | CheckedIntegerBinaryKind::WrappingSubtract
                | CheckedIntegerBinaryKind::SaturatingSubtract
                | CheckedIntegerBinaryKind::WrappingMultiply
                | CheckedIntegerBinaryKind::SaturatingMultiply,
            left,
            right,
            ..
        } if remaining_shells > 0 => {
            let collect = |left_proof_allowed, right_proof_allowed| {
                let mut inputs = shared_integer_runtime_inputs_with_shells(
                    left,
                    scalar_parameter_count,
                    remaining_shells - 1,
                    left_proof_allowed,
                )?;
                inputs.extend(shared_integer_runtime_inputs_with_shells(
                    right,
                    scalar_parameter_count,
                    remaining_shells - 1,
                    right_proof_allowed,
                )?);
                Some(inputs)
            };
            if proof_shell_allowed {
                collect(true, true)
            } else {
                collect(false, false)
            }
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } if proof_shell_allowed => {
            let classifiers = exact_binary_classifier_registry(*kind, *primitive_type)?;
            direct_binary_runtime_inputs(left, right, scalar_parameter_count).or_else(|| {
                first_shared_integer_classification(classifiers, expression, scalar_parameter_count)
            })
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. } if remaining_shells > 0 => {
            shared_integer_runtime_inputs_with_shells(
                operand,
                scalar_parameter_count,
                remaining_shells - 1,
                proof_shell_allowed,
            )
        }
        CheckedScalarExpression::IntegerWiden { operand, .. } if remaining_shells > 0 => {
            shared_integer_runtime_inputs_with_shells(
                operand,
                scalar_parameter_count,
                remaining_shells - 1,
                proof_shell_allowed,
            )
        }
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            ..
        } if proof_shell_allowed => {
            first_shared_exact_cast_classification(*primitive_type, operand, scalar_parameter_count)
                .or_else(|| {
                    shared_integer_runtime_inputs_with_shells(
                        operand,
                        scalar_parameter_count,
                        0,
                        false,
                    )
                })
        }
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::IntegerBinary { .. }
        | CheckedScalarExpression::IntegerBitwiseNot { .. }
        | CheckedScalarExpression::IntegerWiden { .. }
        | CheckedScalarExpression::IntegerExactCast { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::Boolean(_) => None,
    }
}

fn shared_roundtrip_exact_cast_runtime_inputs(
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

fn shared_exact_cast_chain_runtime_inputs(
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

fn partial_fixed_native_primitive_cast(source: PrimitiveType, target: PrimitiveType) -> bool {
    let Some(source_interval) = fixed_native_primitive_interval(source) else {
        return false;
    };
    let Some(target_interval) = fixed_native_primitive_interval(target) else {
        return false;
    };
    source != target
        && !(target_interval.0 <= source_interval.0 && source_interval.1 <= target_interval.1)
}

fn shared_exact_cast_chain_then_computed_suffix_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_inputs(
        expression,
        scalar_parameter_count,
        shared_exact_cast_chain_suffix_root_runtime_inputs,
    )
}

fn shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_inputs(
        expression,
        scalar_parameter_count,
        shared_exact_computed_prefix_cast_chain_suffix_root_runtime_inputs,
    )
}

fn shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_exact_computed_suffix_runtime_inputs(
        expression,
        scalar_parameter_count,
        shared_exact_computed_prefix_widen_chain_suffix_root_runtime_inputs,
    )
}

fn shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_inputs(
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

fn shared_exact_computed_suffix_runtime_inputs(
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

fn shared_exact_cast_chain_suffix_root_runtime_inputs(
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

fn shared_exact_computed_prefix_cast_chain_suffix_root_runtime_inputs(
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

fn shared_exact_computed_prefix_widen_chain_suffix_root_runtime_inputs(
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

fn shared_exact_computed_prefix_mixed_conversion_chain_suffix_root_runtime_inputs(
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

fn shared_exact_direct_parameter_suffix_root_runtime_inputs(
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

fn strict_fixed_native_primitive_widen(source: PrimitiveType, target: PrimitiveType) -> bool {
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

fn shared_exact_cast_chain_then_affine_runtime_inputs(
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

fn shared_exact_cast_chain_then_signed_product_runtime_inputs(
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

fn shared_exact_cast_chain_then_shift_runtime_inputs(
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

fn shared_exact_cast_chain_then_divide_remainder_runtime_inputs(
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

fn shared_exact_computed_prefix_cast_chain_runtime_inputs(
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
enum ExactDivideRemainderTransfer {
    Divide,
    Remainder,
}

fn shared_exact_divide_remainder_chain_cast_runtime_inputs(
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

fn fixed_native_primitive_interval(primitive_type: PrimitiveType) -> Option<(i128, i128)> {
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

fn exact_divide_remainder_interval_transfer(
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

fn landed_safe_exact_divide_remainder_literal(
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

fn shared_exact_offset_chain_cast_runtime_inputs(
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

fn shared_exact_multiply_chain_cast_runtime_inputs(
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

fn shared_exact_signed_multiply_chain_cast_runtime_inputs(
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

fn shared_exact_affine_chain_cast_runtime_inputs(
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

fn shared_exact_shift_left_chain_cast_runtime_inputs(
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

fn shared_exact_mixed_shift_chain_cast_runtime_inputs(
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

fn shared_exact_shift_right_chain_cast_runtime_inputs(
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

fn shared_exact_cast_then_offset_runtime_inputs(
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

fn shared_exact_add_chain_runtime_inputs(
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

fn shared_exact_subtract_chain_runtime_inputs(
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

fn shared_exact_mixed_add_subtract_chain_runtime_inputs(
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

fn exact_offset_landed_literal(
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

fn shared_exact_affine_chain_runtime_inputs(
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

fn checked_signed_pair_multiply(
    left: Option<(bool, u128)>,
    right: (bool, u128),
) -> Option<(bool, u128)> {
    let left = left?;
    if left.1 == 0 || right.1 == 0 {
        return Some((false, 0));
    }
    Some((left.0 ^ right.0, left.1.checked_mul(right.1)?))
}

fn checked_signed_pair_add(
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

fn signed_pair(value: i64) -> (bool, u128) {
    let magnitude = u128::from(value.unsigned_abs());
    (value < 0 && magnitude != 0, magnitude)
}

fn negated_signed_pair((negative, magnitude): (bool, u128)) -> (bool, u128) {
    (magnitude != 0 && !negative, magnitude)
}

fn checked_affine_pair_step(
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

fn checked_signed_affine_step(
    coefficient: Option<(bool, u128)>,
    offset: Option<(bool, u128)>,
    kind: CheckedIntegerBinaryKind,
    literal: i64,
) -> (Option<(bool, u128)>, Option<(bool, u128)>) {
    checked_affine_pair_step(coefficient, offset, kind, signed_pair(literal))
}

fn checked_affine_landed_literal_pair(
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

fn shared_exact_affine_fork_branch(
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

fn shared_exact_affine_fork_join_runtime_inputs(
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

fn shared_exact_distinct_root_affine_fork_join_runtime_inputs(
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

fn shared_exact_distinct_root_affine_product_join_runtime_inputs(
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

fn shared_exact_same_root_affine_product_join_runtime_inputs(
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

fn shared_exact_same_root_affine_divide_remainder_join_runtime_inputs(
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

fn shared_exact_signed_affine_chain_runtime_inputs(
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

fn shared_exact_signed_affine_chain_cast_runtime_inputs(
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

fn shared_exact_cast_then_signed_affine_runtime_inputs(
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

fn shared_exact_shift_then_arithmetic_runtime_inputs(
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

fn shared_exact_cast_then_affine_runtime_inputs(
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

fn shared_exact_affine_cast_affine_runtime_inputs(
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

fn shared_exact_signed_affine_cast_affine_runtime_inputs(
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

fn shared_exact_multiply_chain_runtime_inputs(
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

fn shared_exact_signed_multiply_chain_runtime_inputs(
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

fn shared_exact_cast_then_multiply_runtime_inputs(
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

fn shared_exact_cast_then_signed_multiply_runtime_inputs(
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

fn signed_exact_multiply_literal_value(
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
        (PrimitiveType::I8, Some(psi_numerics::literals::LandedIntegerType::I8))
        | (PrimitiveType::I16, Some(psi_numerics::literals::LandedIntegerType::I16))
        | (PrimitiveType::I32, Some(psi_numerics::literals::LandedIntegerType::I32))
        | (PrimitiveType::I64, Some(psi_numerics::literals::LandedIntegerType::I64)) => {
            literal.value_i64()
        }
        _ => None,
    }
}

fn checked_signed_product(product: Option<(bool, u128)>, factor: i64) -> Option<(bool, u128)> {
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

fn nonnegative_exact_multiply_literal(
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
            literal.value_i64().is_some_and(|value| value >= 0)
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

fn shared_exact_divide_remainder_chain_runtime_inputs(
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

fn shared_exact_runtime_divisor_chain_runtime_inputs(
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

fn shared_exact_cast_then_divide_remainder_runtime_inputs(
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

fn safe_exact_divide_remainder_literal(
    primitive_type: PrimitiveType,
    expression: &CheckedScalarExpression,
) -> bool {
    landed_safe_exact_divide_remainder_literal(primitive_type, expression).is_some()
}

fn shared_exact_shift_right_chain_runtime_inputs(
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

fn shared_exact_cast_then_shift_right_runtime_inputs(
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

fn safe_exact_shift_literal_count(
    value_type: PrimitiveType,
    expression: &CheckedScalarExpression,
) -> bool {
    landed_exact_shift_literal_count(value_type, expression).is_some()
}

fn landed_exact_shift_literal_count(
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

fn shared_exact_mixed_shift_chain_runtime_inputs(
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

fn shared_exact_arithmetic_then_shift_runtime_inputs(
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

fn shared_exact_cast_then_mixed_shift_runtime_inputs(
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

fn shared_exact_shift_cast_shift_runtime_inputs(
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

fn shared_exact_affine_shift_cast_sandwich_runtime_inputs(
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
enum ExactCrossCastChainFamily {
    DivideRemainder,
    Affine,
    Shift,
}

fn checked_exact_cross_cast_chain_family(
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

fn checked_exact_cross_cast_chain_link<'a>(
    expression: &'a CheckedScalarExpression,
    family: ExactCrossCastChainFamily,
) -> Option<(PrimitiveType, &'a CheckedScalarExpression)> {
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

fn checked_exact_cross_cast_parameter_root(
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

fn shared_exact_divide_remainder_cross_cast_runtime_inputs(
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

fn shared_exact_divide_remainder_cast_sandwich_runtime_inputs(
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

fn shared_exact_divide_remainder_cross_chain_runtime_inputs(
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

fn shared_exact_shift_left_chain_runtime_inputs(
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

fn shared_exact_cast_then_shift_left_runtime_inputs(
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
