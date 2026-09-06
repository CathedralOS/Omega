//! Closed sufficient-form classifiers for shared Boolean convergence.
//!
//! These routines recognize source expression shapes and retain their exact
//! runtime parameter roots. They are an untrusted producer convenience: the
//! terminal verifier still reconstructs every admitted obligation.

use super::*;

pub(crate) mod affine;
pub(crate) mod cast_chains;
pub(crate) mod products;
pub(crate) mod shifts;

use affine::*;
use cast_chains::*;
use products::*;
use shifts::*;

pub(super) fn checked_shared_boolean_convergence(
    facts: &CheckFacts,
    state: SymbolHandle,
    bindings: &[CheckedScalarBinding],
    return_expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    cleanup_actions: &[CheckedStructuralScalarReturnCleanupAction],
) -> Option<checked_trees::CheckedStructuralBooleanConvergencePlan> {
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
                    checked_trees::CheckedBooleanExpression::Local { position }
                        if *position == scalar_parameter_count)
        )
    {
        return None;
    }
    Some(checked_trees::CheckedStructuralBooleanConvergencePlan { binding_ordinal: 0 })
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

pub(super) fn shared_boolean_has_member_and_integer_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> bool {
    let CheckedScalarExpression::Boolean(expression) = expression else {
        return false;
    };
    shared_boolean_runtime_inputs(expression, scalar_parameter_count).is_some_and(|inputs| {
        inputs
            .iter()
            .any(|input| matches!(input, SharedBooleanRuntimeInput::StructuralField { .. }))
            && inputs
                .iter()
                .any(|input| matches!(input, SharedBooleanRuntimeInput::IntegerScalar(_)))
    })
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
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => None,
        checked_trees::CheckedBooleanExpression::Constant(_) => Some(BTreeSet::new()),
        checked_trees::CheckedBooleanExpression::Parameter { position }
            if *position < scalar_parameter_count =>
        {
            Some(BTreeSet::from([SharedBooleanRuntimeInput::BooleanScalar(
                *position,
            )]))
        }
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            shared_boolean_runtime_inputs(operand, scalar_parameter_count)
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            match (left.as_ref(), right.as_ref()) {
                (checked_trees::CheckedBooleanExpression::Constant(_), expression)
                | (expression, checked_trees::CheckedBooleanExpression::Constant(_)) => {
                    shared_boolean_runtime_inputs(expression, scalar_parameter_count)
                }
                _ => None,
            }
        }
        checked_trees::CheckedBooleanExpression::And { left, right }
        | checked_trees::CheckedBooleanExpression::Or { left, right } => {
            let mut parameters = shared_boolean_runtime_inputs(left, scalar_parameter_count)?;
            parameters.extend(shared_boolean_runtime_inputs(
                right,
                scalar_parameter_count,
            )?);
            Some(parameters)
        }
        checked_trees::CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } if matches!(
            path.as_slice(),
            [checked_trees::CheckedStructuralPredicatePathSegment::Field(
                _
            )]
        ) =>
        {
            let [checked_trees::CheckedStructuralPredicatePathSegment::Field(field)] =
                path.as_slice()
            else {
                unreachable!("guarded by one field segment")
            };
            Some(BTreeSet::from([
                SharedBooleanRuntimeInput::StructuralField {
                    parameter_position: *parameter_position,
                    field: field.clone(),
                },
            ]))
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            let mut inputs = shared_integer_runtime_inputs(left, scalar_parameter_count)?;
            inputs.extend(shared_integer_runtime_inputs(
                right,
                scalar_parameter_count,
            )?);
            Some(inputs)
        }
        checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => None,
        checked_trees::CheckedBooleanExpression::Parameter { .. }
        | checked_trees::CheckedBooleanExpression::Local { .. }
        | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. } => None,
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
        CheckedScalarExpression::StorageRead { .. } => None,
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
        CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::IntegerBinary { .. }
        | CheckedScalarExpression::IntegerBitwiseNot { .. }
        | CheckedScalarExpression::IntegerWiden { .. }
        | CheckedScalarExpression::IntegerExactCast { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::Boolean(_) => None,
    }
}
