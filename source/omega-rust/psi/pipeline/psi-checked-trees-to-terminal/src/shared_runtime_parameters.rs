//! Runtime-input custody for checked shared-convergence plans.
//!
//! This is an untrusted certificate-producer boundary during the Q7 migration.
//! The registries preserve the established recognizer precedence; they do not
//! choose the future canonical semantic-ledger goal or grant proof authority.

use super::*;

mod affine;
mod boolean;
mod conversions;
mod products_divisors;
mod shifts;

use affine::*;
use conversions::*;
use products_divisors::*;
use shifts::*;

type RuntimeParameters = BTreeSet<SharedBooleanRuntimeInput>;
type ExpressionClassifier = fn(&LoweredDirectExpression) -> Option<RuntimeParameters>;
type CastClassifier = fn(ScalarType, &LoweredDirectExpression) -> Option<RuntimeParameters>;

#[derive(Clone, Copy)]
struct NamedExpressionClassifier {
    name: &'static str,
    classify: ExpressionClassifier,
}

impl NamedExpressionClassifier {
    const fn new(name: &'static str, classify: ExpressionClassifier) -> Self {
        Self { name, classify }
    }
}

#[derive(Clone, Copy)]
struct NamedCastClassifier {
    name: &'static str,
    classify: CastClassifier,
}

impl NamedCastClassifier {
    const fn new(name: &'static str, classify: CastClassifier) -> Self {
        Self { name, classify }
    }
}

fn first_expression_classification(
    expression: &LoweredDirectExpression,
    registries: &[&[NamedExpressionClassifier]],
) -> Option<RuntimeParameters> {
    registries
        .iter()
        .flat_map(|registry| *registry)
        .find_map(|entry| {
            debug_assert!(!entry.name.is_empty());
            (entry.classify)(expression)
        })
}

fn first_cast_classification(
    target_type: ScalarType,
    operand: &LoweredDirectExpression,
    registry: &[NamedCastClassifier],
) -> Option<RuntimeParameters> {
    registry.iter().find_map(|entry| {
        debug_assert!(!entry.name.is_empty());
        (entry.classify)(target_type, operand)
    })
}

const CROSS_FAMILY_ARITHMETIC_CLASSIFIERS: &[NamedExpressionClassifier] = &[
    NamedExpressionClassifier::new(
        "shift-then-arithmetic",
        shared_exact_shift_then_arithmetic_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "divide-remainder-cross-chain",
        shared_exact_divide_remainder_cross_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "divide-remainder-cross-cast",
        shared_exact_divide_remainder_cross_cast_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "affine-shift-cast-sandwich",
        shared_exact_affine_shift_cast_sandwich_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "affine-cast-affine",
        shared_exact_affine_cast_affine_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "signed-affine-cast-affine",
        shared_exact_signed_affine_cast_affine_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "computed-prefix-cast-chain-computed-suffix",
        shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "computed-prefix-widen-chain-computed-suffix",
        shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "computed-prefix-mixed-conversion-chain-computed-suffix",
        shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "cast-chain-computed-suffix",
        shared_exact_cast_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "cast-then-affine",
        shared_exact_cast_then_affine_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "cast-then-signed-affine",
        shared_exact_cast_then_signed_affine_runtime_parameters,
    ),
];

const ADD_CLASSIFIERS: &[NamedExpressionClassifier] = &[
    NamedExpressionClassifier::new(
        "cast-then-offset",
        shared_exact_cast_then_offset_runtime_parameters,
    ),
    NamedExpressionClassifier::new("add-chain", shared_exact_add_chain_runtime_parameters),
    NamedExpressionClassifier::new(
        "mixed-add-subtract-chain",
        shared_exact_mixed_add_subtract_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new("affine-chain", shared_exact_affine_chain_runtime_parameters),
    NamedExpressionClassifier::new(
        "signed-affine-chain",
        shared_exact_signed_affine_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "same-root-affine-fork-join",
        shared_exact_affine_fork_join_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "distinct-root-affine-fork-join",
        shared_exact_distinct_root_affine_fork_join_runtime_parameters,
    ),
];

const SUBTRACT_CLASSIFIERS: &[NamedExpressionClassifier] = &[
    NamedExpressionClassifier::new(
        "cast-then-offset",
        shared_exact_cast_then_offset_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "subtract-chain",
        shared_exact_subtract_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "mixed-add-subtract-chain",
        shared_exact_mixed_add_subtract_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new("affine-chain", shared_exact_affine_chain_runtime_parameters),
    NamedExpressionClassifier::new(
        "signed-affine-chain",
        shared_exact_signed_affine_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "same-root-affine-fork-join",
        shared_exact_affine_fork_join_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "distinct-root-affine-fork-join",
        shared_exact_distinct_root_affine_fork_join_runtime_parameters,
    ),
];

const MULTIPLY_CLASSIFIERS: &[NamedExpressionClassifier] = &[
    NamedExpressionClassifier::new(
        "cast-then-multiply",
        shared_exact_cast_then_multiply_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "cast-then-signed-multiply",
        shared_exact_cast_then_signed_multiply_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "multiply-chain",
        shared_exact_multiply_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "signed-multiply-chain",
        shared_exact_signed_multiply_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new("affine-chain", shared_exact_affine_chain_runtime_parameters),
    NamedExpressionClassifier::new(
        "signed-affine-chain",
        shared_exact_signed_affine_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "same-root-affine-product-join",
        shared_exact_same_root_affine_product_join_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "distinct-root-affine-product-join",
        shared_exact_distinct_root_affine_product_join_runtime_parameters,
    ),
];

const SHIFT_CROSS_FAMILY_CLASSIFIERS: &[NamedExpressionClassifier] = &[
    NamedExpressionClassifier::new(
        "mixed-shift-chain",
        shared_exact_mixed_shift_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "divide-remainder-cross-chain",
        shared_exact_divide_remainder_cross_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "divide-remainder-cross-cast",
        shared_exact_divide_remainder_cross_cast_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "affine-shift-cast-sandwich",
        shared_exact_affine_shift_cast_sandwich_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "shift-cast-shift",
        shared_exact_shift_cast_shift_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "computed-prefix-cast-chain-computed-suffix",
        shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "computed-prefix-widen-chain-computed-suffix",
        shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "computed-prefix-mixed-conversion-chain-computed-suffix",
        shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "cast-chain-computed-suffix",
        shared_exact_cast_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "cast-then-mixed-shift",
        shared_exact_cast_then_mixed_shift_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "arithmetic-then-shift",
        shared_exact_arithmetic_then_shift_runtime_parameters,
    ),
];

const SHIFT_RIGHT_CLASSIFIERS: &[NamedExpressionClassifier] = &[
    NamedExpressionClassifier::new(
        "cast-then-shift-right",
        shared_exact_cast_then_shift_right_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "shift-right-chain",
        shared_exact_shift_right_chain_runtime_parameters,
    ),
];

const SHIFT_LEFT_CLASSIFIERS: &[NamedExpressionClassifier] = &[
    NamedExpressionClassifier::new(
        "cast-then-shift-left",
        shared_exact_cast_then_shift_left_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "shift-left-chain",
        shared_exact_shift_left_chain_runtime_parameters,
    ),
];

const DIVIDE_REMAINDER_CLASSIFIERS: &[NamedExpressionClassifier] = &[
    NamedExpressionClassifier::new(
        "same-root-affine-divide-remainder-join",
        shared_exact_same_root_affine_divide_remainder_join_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "divide-remainder-cross-chain",
        shared_exact_divide_remainder_cross_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "divide-remainder-cross-cast",
        shared_exact_divide_remainder_cross_cast_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "divide-remainder-cast-sandwich",
        shared_exact_divide_remainder_cast_sandwich_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "runtime-divisor-chain",
        shared_exact_runtime_divisor_chain_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "computed-prefix-cast-chain-computed-suffix",
        shared_exact_computed_prefix_cast_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "computed-prefix-widen-chain-computed-suffix",
        shared_exact_computed_prefix_widen_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "computed-prefix-mixed-conversion-chain-computed-suffix",
        shared_exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "cast-chain-computed-suffix",
        shared_exact_cast_chain_then_computed_suffix_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "cast-then-divide-remainder",
        shared_exact_cast_then_divide_remainder_runtime_parameters,
    ),
    NamedExpressionClassifier::new(
        "divide-remainder-chain",
        shared_exact_divide_remainder_chain_runtime_parameters,
    ),
];

const CAST_CLASSIFIERS: &[NamedCastClassifier] = &[
    NamedCastClassifier::new(
        "roundtrip-cast",
        shared_roundtrip_exact_cast_runtime_parameters,
    ),
    NamedCastClassifier::new("cast-chain", shared_exact_cast_chain_runtime_parameters),
    NamedCastClassifier::new(
        "computed-prefix-cast-chain",
        shared_exact_computed_prefix_cast_chain_runtime_parameters,
    ),
    NamedCastClassifier::new(
        "divide-remainder-chain-cast",
        shared_exact_divide_remainder_chain_cast_runtime_parameters,
    ),
    NamedCastClassifier::new(
        "mixed-shift-chain-cast",
        shared_exact_mixed_shift_chain_cast_runtime_parameters,
    ),
    NamedCastClassifier::new(
        "shift-right-chain-cast",
        shared_exact_shift_right_chain_cast_runtime_parameters,
    ),
    NamedCastClassifier::new(
        "shift-left-chain-cast",
        shared_exact_shift_left_chain_cast_runtime_parameters,
    ),
    NamedCastClassifier::new(
        "affine-chain-cast",
        shared_exact_affine_chain_cast_runtime_parameters,
    ),
    NamedCastClassifier::new(
        "multiply-chain-cast",
        shared_exact_multiply_chain_cast_runtime_parameters,
    ),
    NamedCastClassifier::new(
        "signed-multiply-chain-cast",
        shared_exact_signed_multiply_chain_cast_runtime_parameters,
    ),
    NamedCastClassifier::new(
        "signed-affine-chain-cast",
        shared_exact_signed_affine_chain_cast_runtime_parameters,
    ),
    NamedCastClassifier::new(
        "offset-chain-cast",
        shared_exact_offset_chain_cast_runtime_parameters,
    ),
];

pub(super) fn shared_boolean_runtime_parameters(
    expression: &LoweredBooleanReturnExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => Some(BTreeSet::new()),
        LoweredBooleanReturnExpression::Parameter { position } => {
            Some(BTreeSet::from([SharedBooleanRuntimeInput::BooleanScalar(
                *position,
            )]))
        }
        LoweredBooleanReturnExpression::StructuralField { source, field } => {
            Some(BTreeSet::from([
                SharedBooleanRuntimeInput::StructuralField {
                    source: *source,
                    field: *field,
                },
            ]))
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            shared_boolean_runtime_parameters(operand)
        }
        LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            let mut parameters = shared_boolean_runtime_parameters(left)?;
            parameters.extend(shared_boolean_runtime_parameters(right)?);
            Some(parameters)
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            let mut parameters = shared_integer_runtime_parameters(left)?;
            parameters.extend(shared_integer_runtime_parameters(right)?);
            Some(parameters)
        }
        LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::Equal { .. } => None,
    }
}

fn shared_integer_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    shared_integer_runtime_parameters_with_shells(expression, 2, true)
}

fn shared_integer_runtime_parameters_with_shells(
    expression: &LoweredDirectExpression,
    remaining_shells: usize,
    proof_shell_allowed: bool,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    match expression {
        LoweredDirectExpression::IntegerLiteral { .. } => Some(BTreeSet::new()),
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        } => matches!(scalar_type, ScalarType::Integer(_))
            .then(|| BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(*position)])),
        LoweredDirectExpression::IntegerBinary {
            kind:
                LoweredIntegerBinaryKind::BitwiseAnd
                | LoweredIntegerBinaryKind::BitwiseOr
                | LoweredIntegerBinaryKind::BitwiseXor
                | LoweredIntegerBinaryKind::WrappingShiftLeft
                | LoweredIntegerBinaryKind::WrappingShiftRight
                | LoweredIntegerBinaryKind::WrappingAdd
                | LoweredIntegerBinaryKind::SaturatingAdd
                | LoweredIntegerBinaryKind::WrappingSubtract
                | LoweredIntegerBinaryKind::SaturatingSubtract
                | LoweredIntegerBinaryKind::WrappingMultiply
                | LoweredIntegerBinaryKind::SaturatingMultiply,
            left,
            right,
            ..
        } if remaining_shells > 0 => {
            let collect = |left_proof_allowed, right_proof_allowed| {
                let mut parameters = shared_integer_runtime_parameters_with_shells(
                    left,
                    remaining_shells - 1,
                    left_proof_allowed,
                )?;
                parameters.extend(shared_integer_runtime_parameters_with_shells(
                    right,
                    remaining_shells - 1,
                    right_proof_allowed,
                )?);
                Some(parameters)
            };
            if proof_shell_allowed {
                collect(true, true)
            } else {
                collect(false, false)
            }
        }
        LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactAdd,
            left,
            right,
            ..
        } if proof_shell_allowed => {
            let collect_direct = |left, right| {
                let mut parameters = shared_integer_runtime_parameters_with_shells(left, 0, false)?;
                parameters.extend(shared_integer_runtime_parameters_with_shells(
                    right, 0, false,
                )?);
                Some(parameters)
            };
            collect_direct(left, right).or_else(|| {
                first_expression_classification(
                    expression,
                    &[CROSS_FAMILY_ARITHMETIC_CLASSIFIERS, ADD_CLASSIFIERS],
                )
            })
        }
        LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactSubtract,
            left,
            right,
            ..
        } if proof_shell_allowed => {
            let collect_direct = || {
                let mut parameters = shared_integer_runtime_parameters_with_shells(left, 0, false)?;
                parameters.extend(shared_integer_runtime_parameters_with_shells(
                    right, 0, false,
                )?);
                Some(parameters)
            };
            collect_direct().or_else(|| {
                first_expression_classification(
                    expression,
                    &[CROSS_FAMILY_ARITHMETIC_CLASSIFIERS, SUBTRACT_CLASSIFIERS],
                )
            })
        }
        LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactMultiply,
            scalar_type: ScalarType::Integer(_),
            left,
            right,
        } if proof_shell_allowed => {
            let collect_direct = || {
                let mut parameters = shared_integer_runtime_parameters_with_shells(left, 0, false)?;
                parameters.extend(shared_integer_runtime_parameters_with_shells(
                    right, 0, false,
                )?);
                Some(parameters)
            };
            collect_direct().or_else(|| {
                first_expression_classification(
                    expression,
                    &[CROSS_FAMILY_ARITHMETIC_CLASSIFIERS, MULTIPLY_CLASSIFIERS],
                )
            })
        }
        LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactShiftRight,
            scalar_type: ScalarType::Integer(_),
            left,
            right,
        } if proof_shell_allowed => {
            let collect_direct = || {
                let mut parameters = shared_integer_runtime_parameters_with_shells(left, 0, false)?;
                parameters.extend(shared_integer_runtime_parameters_with_shells(
                    right, 0, false,
                )?);
                Some(parameters)
            };
            collect_direct().or_else(|| {
                first_expression_classification(
                    expression,
                    &[SHIFT_CROSS_FAMILY_CLASSIFIERS, SHIFT_RIGHT_CLASSIFIERS],
                )
            })
        }
        LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactDivide | LoweredIntegerBinaryKind::ExactRemainder,
            scalar_type: ScalarType::Integer(_),
            left,
            right,
        } if proof_shell_allowed => {
            let collect_direct = || {
                let mut parameters = shared_integer_runtime_parameters_with_shells(left, 0, false)?;
                parameters.extend(shared_integer_runtime_parameters_with_shells(
                    right, 0, false,
                )?);
                Some(parameters)
            };
            collect_direct().or_else(|| {
                first_expression_classification(expression, &[DIVIDE_REMAINDER_CLASSIFIERS])
            })
        }
        LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactShiftLeft,
            scalar_type: ScalarType::Integer(_),
            left,
            right,
        } if proof_shell_allowed => {
            let collect_direct = || {
                let mut parameters = shared_integer_runtime_parameters_with_shells(left, 0, false)?;
                parameters.extend(shared_integer_runtime_parameters_with_shells(
                    right, 0, false,
                )?);
                Some(parameters)
            };
            collect_direct().or_else(|| {
                first_expression_classification(
                    expression,
                    &[SHIFT_CROSS_FAMILY_CLASSIFIERS, SHIFT_LEFT_CLASSIFIERS],
                )
            })
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. } if remaining_shells > 0 => {
            shared_integer_runtime_parameters_with_shells(
                operand,
                remaining_shells - 1,
                proof_shell_allowed,
            )
        }
        LoweredDirectExpression::IntegerWiden { operand, .. } if remaining_shells > 0 => {
            shared_integer_runtime_parameters_with_shells(
                operand,
                remaining_shells - 1,
                proof_shell_allowed,
            )
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } if proof_shell_allowed => {
            first_cast_classification(*scalar_type, operand, CAST_CLASSIFIERS)
                .or_else(|| shared_integer_runtime_parameters_with_shells(operand, 0, false))
        }
        LoweredDirectExpression::Local { .. }
        | LoweredDirectExpression::IntegerBinary { .. }
        | LoweredDirectExpression::IntegerBitwiseNot { .. }
        | LoweredDirectExpression::IntegerWiden { .. }
        | LoweredDirectExpression::IntegerExactCast { .. }
        | LoweredDirectExpression::Boolean { .. } => None,
    }
}

fn native_fixed_integer_type(integer_type: IntegerType) -> bool {
    !integer_type.is_address() && matches!(integer_type.bits(), 8 | 16 | 32 | 64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SharedBooleanRuntimeInput {
    BooleanScalar(usize),
    IntegerScalar(usize),
    StructuralField {
        source: PlaceId,
        field: StructuralFieldId,
    },
}

pub(super) fn valid_shared_boolean_runtime_inputs(
    inputs: &BTreeSet<SharedBooleanRuntimeInput>,
) -> bool {
    boolean::valid_shared_boolean_runtime_inputs(inputs)
}

pub(super) fn resolve_shared_boolean_member_fields(
    expression: LoweredBooleanReturnExpression,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    boolean::resolve_shared_boolean_member_fields(expression, parameters, structural_types)
}

pub(super) fn normalize_shared_boolean_comparison_leaves(
    expression: &LoweredBooleanReturnExpression,
) -> Option<LoweredBooleanReturnExpression> {
    boolean::normalize_shared_boolean_comparison_leaves(expression)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_unique_expression_registry(
        label: &str,
        registries: &[&[NamedExpressionClassifier]],
        expected_count: usize,
    ) {
        let mut names = BTreeSet::new();
        for entry in registries.iter().flat_map(|registry| *registry) {
            assert!(!entry.name.is_empty(), "{label} has an unnamed classifier");
            assert!(
                names.insert(entry.name),
                "{label} repeats classifier {}",
                entry.name
            );
        }
        assert_eq!(names.len(), expected_count, "{label} inventory drifted");
    }

    #[test]
    fn shared_runtime_classifier_registries_are_named_ordered_and_unique() {
        assert_unique_expression_registry(
            "exact add",
            &[CROSS_FAMILY_ARITHMETIC_CLASSIFIERS, ADD_CLASSIFIERS],
            19,
        );
        assert_unique_expression_registry(
            "exact subtract",
            &[CROSS_FAMILY_ARITHMETIC_CLASSIFIERS, SUBTRACT_CLASSIFIERS],
            19,
        );
        assert_unique_expression_registry(
            "exact multiply",
            &[CROSS_FAMILY_ARITHMETIC_CLASSIFIERS, MULTIPLY_CLASSIFIERS],
            20,
        );
        assert_unique_expression_registry(
            "exact shift right",
            &[SHIFT_CROSS_FAMILY_CLASSIFIERS, SHIFT_RIGHT_CLASSIFIERS],
            13,
        );
        assert_unique_expression_registry(
            "exact shift left",
            &[SHIFT_CROSS_FAMILY_CLASSIFIERS, SHIFT_LEFT_CLASSIFIERS],
            13,
        );
        assert_unique_expression_registry(
            "exact divide/remainder",
            &[DIVIDE_REMAINDER_CLASSIFIERS],
            11,
        );

        let names = CAST_CLASSIFIERS
            .iter()
            .map(|entry| entry.name)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 12, "exact cast inventory drifted");
        assert!(CAST_CLASSIFIERS.iter().all(|entry| !entry.name.is_empty()));
    }
}
