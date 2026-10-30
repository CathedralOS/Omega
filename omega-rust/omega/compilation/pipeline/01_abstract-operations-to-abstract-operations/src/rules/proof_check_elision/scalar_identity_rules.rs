//! The proof-certified scalar identity rule family: one table of rule rows
//! over the shared identity proposal (`identity_rewrite`), each row naming its
//! canonical identity bytes and the closed shape partition it selects. The
//! family's contract -- the same three required analyses under proof-certified
//! safety -- and the one identity proposal are stated once, below the table.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use optimization_core::{AnalysisKind, AnalysisSet, OptimizationRuleContract};
use optimization_unit::{
    ProofCertifiedScalarIdentityKind, PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::IntegerValue;

use crate::rules::proof_check_elision::identity_rewrite::{
    ProofCertifiedScalarIdentityShape, propose_proof_certified_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

/// One scalar identity rule: its type, canonical identity bytes, and the
/// closed shape partition it proposes over. Every rule of the family requires
/// the scalar constants, use-definition and effect-summary products under the
/// pass's proof-certified safety class and runs
/// [`propose_proof_certified_scalar_identities`]; only the partition differs.
macro_rules! proof_certified_scalar_identity_rules {
    ($($(#[$meta:meta])* $name:ident = ($identity:literal, $shapes:path);)*) => {$(
        psi_rule_contract!(
            $(#[$meta])*
            $name,
            super::proof_check_elision_rule_contract(
                $identity,
                AnalysisSet::new([
                    AnalysisKind::ScalarConstants,
                    AnalysisKind::UseDefinition,
                    AnalysisKind::EffectSummaries,
                ]),
            ),
            propose = propose_proof_certified_scalar_identities,
            $shapes,
        );
    )*};
}

proof_certified_scalar_identity_rules! {
    /// Proof-certified division by one.
    LiveProofCertifiedIntegerDivideByOneEliminationRule = (
        b"omega.psi-rule.live-proof-certified-integer-divide-by-one-elimination.v1",
        proof_certified_integer_divide_by_one_shapes
    );
    /// Proof-certified exact multiplication by zero.
    LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule = (
        b"omega.psi-rule.live-proof-certified-exact-integer-multiply-by-zero-elimination.v1",
        proof_certified_exact_integer_multiply_by_zero_shapes
    );
    /// General proof-certified integer identities.
    LiveProofCertifiedIntegerIdentityEliminationRule = (
        b"omega.psi-rule.live-proof-certified-integer-identity-elimination.v1",
        proof_certified_scalar_identity_shapes
    );
    /// Proof-certified zero dividends.
    LiveProofCertifiedIntegerZeroDividendEliminationRule = (
        b"omega.psi-rule.live-proof-certified-integer-zero-dividend-elimination.v1",
        proof_certified_integer_zero_dividend_shapes
    );
    /// Proof-certified exact shifts of a zero value.
    LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule = (
        b"omega.psi-rule.live-proof-certified-exact-integer-zero-value-shift-elimination.v1",
        proof_certified_exact_integer_zero_value_shift_shapes
    );
}

/// Proof-certified division by one.
fn proof_certified_integer_divide_by_one_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let Some(operands) = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Divide)
    else {
        return Vec::new();
    };
    let (source_operation, result, scalar_type, left, right, identity) = (
        operands.psi_operation,
        operands.result,
        operands.scalar_type,
        operands.left,
        operands.right,
        match operands.arithmetic.policy() {
            IntegerOverflowPolicy::Exact => {
                ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight
            }
            IntegerOverflowPolicy::Wrapping => {
                ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight
            }
            IntegerOverflowPolicy::Saturating => {
                ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight
            }
        },
    );
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: right,
            scalar_type,
            identity,
        },
        scalar_type.one(),
    )]
}

/// Proof-certified exact multiplication by zero.
fn proof_certified_exact_integer_multiply_by_zero_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let Some(operands) = operation.integer_arithmetic_operands(IntegerArithmeticOperator::Multiply)
    else {
        return Vec::new();
    };
    if operands.arithmetic.policy() != IntegerOverflowPolicy::Exact {
        return Vec::new();
    }
    vec![
        (
            ProofCertifiedScalarIdentityShape {
                source_operation: operands.psi_operation,
                result: operands.result,
                replacement: operands.left,
                identity_operand: operands.left,
                scalar_type: operands.scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
            },
            operands.scalar_type.zero(),
        ),
        (
            ProofCertifiedScalarIdentityShape {
                source_operation: operands.psi_operation,
                result: operands.result,
                replacement: operands.right,
                identity_operand: operands.right,
                scalar_type: operands.scalar_type,
                identity: ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
            },
            operands.scalar_type.zero(),
        ),
    ]
}

/// General proof-certified integer identities.
fn proof_certified_scalar_identity_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    use ProofCertifiedScalarIdentityKind as Kind;
    match operation {
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } if arithmetic.policy() == IntegerOverflowPolicy::Exact => {
            let zero = scalar_type.zero();
            let one = scalar_type.one();
            // (replacement, identity operand, identity, identity constant),
            // left-literal law before right-literal law.
            let laws = match arithmetic.operator() {
                IntegerArithmeticOperator::Add => vec![
                    (*right, *left, Kind::ExactIntegerAddZeroLeft, zero),
                    (*left, *right, Kind::ExactIntegerAddZeroRight, zero),
                ],
                IntegerArithmeticOperator::Subtract => {
                    vec![(*left, *right, Kind::ExactIntegerSubtractZeroRight, zero)]
                }
                IntegerArithmeticOperator::Multiply => vec![
                    (*right, *left, Kind::ExactIntegerMultiplyOneLeft, one),
                    (*left, *right, Kind::ExactIntegerMultiplyOneRight, one),
                ],
                IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => {
                    Vec::new()
                }
            };
            laws.into_iter()
                .map(|(replacement, identity_operand, identity, constant)| {
                    (
                        ProofCertifiedScalarIdentityShape {
                            source_operation: *psi_operation,
                            result: *result,
                            replacement,
                            identity_operand,
                            scalar_type: *scalar_type,
                            identity,
                        },
                        constant,
                    )
                })
                .collect()
        }
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Exact { .. },
            direction,
            value,
            count,
        } => vec![(
            ProofCertifiedScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *value,
                identity_operand: *count,
                scalar_type: *value_type,
                identity: match direction {
                    IntegerShiftDirection::Left => Kind::ExactIntegerShiftLeftZeroCount,
                    IntegerShiftDirection::Right => Kind::ExactIntegerShiftRightZeroCount,
                },
            },
            count_type.zero(),
        )],
        _ => Vec::new(),
    }
}

/// Proof-certified zero dividends.
fn proof_certified_integer_zero_dividend_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let Some(operands) = operation.integer_arithmetic() else {
        return Vec::new();
    };
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    use ProofCertifiedScalarIdentityKind as Kind;
    let identity = match (operands.arithmetic.policy(), operands.arithmetic.operator()) {
        (Policy::Exact, Operator::Divide) => Kind::ExactIntegerDivideZeroLeft,
        (Policy::Wrapping, Operator::Divide) => Kind::WrappingIntegerDivideZeroLeft,
        (Policy::Saturating, Operator::Divide) => Kind::SaturatingIntegerDivideZeroLeft,
        (Policy::Exact, Operator::Remainder) => Kind::ExactIntegerRemainderZeroLeft,
        (Policy::Wrapping, Operator::Remainder) => Kind::WrappingIntegerRemainderZeroLeft,
        (Policy::Saturating, Operator::Remainder) => Kind::SaturatingIntegerRemainderZeroLeft,
        (_, Operator::Add | Operator::Subtract | Operator::Multiply) => {
            return Vec::new();
        }
    };
    let (source_operation, result, scalar_type, left) = (
        operands.psi_operation,
        operands.result,
        operands.scalar_type,
        operands.left,
    );
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            identity_operand: left,
            scalar_type,
            identity,
        },
        scalar_type.zero(),
    )]
}

/// Proof-certified exact shifts of a zero value.
fn proof_certified_exact_integer_zero_value_shift_shapes(
    operation: &O,
) -> Vec<(ProofCertifiedScalarIdentityShape, IntegerValue)> {
    let Some(operands) = operation.integer_shift() else {
        return Vec::new();
    };
    if !matches!(operands.policy, IntegerShiftPolicy::Exact { .. }) {
        return Vec::new();
    }
    let identity = match operands.direction {
        IntegerShiftDirection::Left => {
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue
        }
        IntegerShiftDirection::Right => {
            ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue
        }
    };
    let (source_operation, result, scalar_type, value) = (
        operands.psi_operation,
        operands.result,
        operands.value_type,
        operands.value,
    );
    vec![(
        ProofCertifiedScalarIdentityShape {
            source_operation,
            result,
            replacement: value,
            identity_operand: value,
            scalar_type,
            identity,
        },
        scalar_type.zero(),
    )]
}
