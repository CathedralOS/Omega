/*
Sparse conditional constant propagation (`sccp`): local rule order and shared
constant-fold macro over binary, unary, and cast integer constant folds,
Boolean constant folds, range-against-constant comparisons, and
range-against-range comparisons.
*/

macro_rules! constant_fold_rules {
    ($($name:ident = ($identity:literal, $safety:ident, $kind:expr);)*) => {$(
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                contract($identity, OptimizationSafetyClass::$safety)
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                propose(unit, analyses, Self::contract(), $kind)
            }
        }
    )*};
}

pub(crate) mod boolean_constants {
    use abstract_operations::AbstractOperation as O;
    use optimization_core::{AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass};
    use optimization_unit::{
        IntegerEvaluationWitness, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
        PsiRewriteCandidate,
    };

    use crate::rules::sparse_conditional_constant_propagation::{
        constant_evaluation_contract as contract, integer_constant, integer_value_type,
    };
    use crate::{
        AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
        ScalarConstantAnalysis,
    };

    constant_fold_rules! {
        BooleanEqualConstantsRule = (b"omega.psi-rule.boolean-equal-constants.v1", ExactOperationSemantics, BooleanEvaluationKind::Equal);
        BooleanNotConstantsRule = (b"omega.psi-rule.boolean-not-constants.v1", ExactOperationSemantics, BooleanEvaluationKind::Not);
        IntegerEqualConstantsRule = (b"omega.psi-rule.integer-equal-constants.v1", ExactOperationSemantics, BooleanEvaluationKind::IntegerEqual);
        IntegerLessOrEqualConstantsRule = (b"omega.psi-rule.integer-less-or-equal-constants.v1", ExactOperationSemantics, BooleanEvaluationKind::IntegerLessOrEqual);
        IntegerLessThanConstantsRule = (b"omega.psi-rule.integer-less-than-constants.v1", ExactOperationSemantics, BooleanEvaluationKind::IntegerLessThan);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum BooleanEvaluationKind {
        Not,
        Equal,
        IntegerEqual,
        IntegerLessThan,
        IntegerLessOrEqual,
    }

    fn propose(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        kind: BooleanEvaluationKind,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
        super::propose_boolean_evaluations(unit, contract, |function, _, node, _| {
            evaluate(function, node, constants, kind)
        })
    }

    fn evaluate(
        function: &PsiOptimizationFunction,
        node: &OptimizationNode,
        constants: &ScalarConstantAnalysis,
        kind: BooleanEvaluationKind,
    ) -> Option<super::BooleanConstantEvaluation> {
        let (source_operation, result, constant, witness) = match (&node.operation, kind) {
            (
                O::BooleanNot {
                    psi_operation,
                    result,
                    operand,
                },
                BooleanEvaluationKind::Not,
            ) => {
                let (operand, operand_fact) =
                    constants.boolean_constant(function.machine, *operand)?;
                (
                    *psi_operation,
                    *result,
                    !operand,
                    IntegerEvaluationWitness::Unary { operand_fact },
                )
            }
            (
                O::BooleanEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                },
                BooleanEvaluationKind::Equal,
            ) => {
                let (left, left_fact) = constants.boolean_constant(function.machine, *left)?;
                let (right, right_fact) = constants.boolean_constant(function.machine, *right)?;
                (
                    *psi_operation,
                    *result,
                    left == right,
                    IntegerEvaluationWitness::Binary {
                        left_fact,
                        right_fact,
                    },
                )
            }
            (
                O::IntegerEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                },
                BooleanEvaluationKind::IntegerEqual,
            )
            | (
                O::IntegerLessThan {
                    psi_operation,
                    result,
                    left,
                    right,
                },
                BooleanEvaluationKind::IntegerLessThan,
            )
            | (
                O::IntegerLessOrEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                },
                BooleanEvaluationKind::IntegerLessOrEqual,
            ) => {
                let (left_value, left_fact) = integer_constant(constants, function.machine, *left)?;
                let (right_value, right_fact) =
                    integer_constant(constants, function.machine, *right)?;
                let left_type = integer_value_type(function, *left)?;
                if integer_value_type(function, *right) != Some(left_type) {
                    return None;
                }
                let ordering = left_type.compare(left_value, right_value)?;
                let constant = match kind {
                    BooleanEvaluationKind::IntegerEqual => ordering.is_eq(),
                    BooleanEvaluationKind::IntegerLessThan => ordering.is_lt(),
                    BooleanEvaluationKind::IntegerLessOrEqual => !ordering.is_gt(),
                    BooleanEvaluationKind::Not | BooleanEvaluationKind::Equal => unreachable!(),
                };
                (
                    *psi_operation,
                    *result,
                    constant,
                    IntegerEvaluationWitness::Binary {
                        left_fact,
                        right_fact,
                    },
                )
            }
            _ => return None,
        };
        Some(super::BooleanConstantEvaluation {
            source_operation,
            result,
            constant,
            witness,
        })
    }
}
pub(crate) mod integer_binary_constants {
    use abstract_operations::{
        IntegerArithmetic, IntegerArithmeticOperator, IntegerArithmeticOperator as Operator,
        IntegerOverflowPolicy, IntegerOverflowPolicy as Policy, IntegerShiftDirection,
        IntegerShiftDirection as Direction, IntegerShiftPolicy,
    };
    use optimization_core::{
        OptimizationRuleContract, OptimizationSafetyClass, ScalarConstantFactIdentity,
    };
    use optimization_unit::{IntegerEvaluationWitness, PsiOptimizationUnit, PsiRewriteCandidate};
    use semantic_vocabulary::{IntegerType, IntegerValue, MachineId, OperationId, ValueId};

    use super::integer_binary_shapes;
    use crate::rules::accepted_obligations::accepted_obligation_fact;
    use crate::rules::sparse_conditional_constant_propagation::{
        constant_evaluation_contract as contract, integer_constant,
    };
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    constant_fold_rules! {
        ExactIntegerAddConstantsRule = (b"omega.psi-rule.exact-integer-add-constants.v1", ProofCertified, IntegerBinaryKind::Arithmetic(Policy::Exact, Operator::Add));
        ExactIntegerDivideConstantsRule = (b"omega.psi-rule.exact-integer-divide-constants.v1", ProofCertified, IntegerBinaryKind::Arithmetic(Policy::Exact, Operator::Divide));
        ExactIntegerMultiplyConstantsRule = (b"omega.psi-rule.exact-integer-multiply-constants.v1", ProofCertified, IntegerBinaryKind::Arithmetic(Policy::Exact, Operator::Multiply));
        ExactIntegerRemainderConstantsRule = (b"omega.psi-rule.exact-integer-remainder-constants.v1", ProofCertified, IntegerBinaryKind::Arithmetic(Policy::Exact, Operator::Remainder));
        ExactIntegerShiftLeftConstantsRule = (b"omega.psi-rule.exact-integer-shift-left-constants.v1", ProofCertified, IntegerBinaryKind::Shift(Policy::Exact, Direction::Left));
        ExactIntegerShiftRightConstantsRule = (b"omega.psi-rule.exact-integer-shift-right-constants.v1", ProofCertified, IntegerBinaryKind::Shift(Policy::Exact, Direction::Right));
        ExactIntegerSubtractConstantsRule = (b"omega.psi-rule.exact-integer-subtract-constants.v1", ProofCertified, IntegerBinaryKind::Arithmetic(Policy::Exact, Operator::Subtract));
        IntegerBitwiseAndConstantsRule = (b"omega.psi-rule.integer-bitwise-and-constants.v1", ExactOperationSemantics, IntegerBinaryKind::BitwiseAnd);
        IntegerBitwiseOrConstantsRule = (b"omega.psi-rule.integer-bitwise-or-constants.v1", ExactOperationSemantics, IntegerBinaryKind::BitwiseOr);
        IntegerBitwiseXorConstantsRule = (b"omega.psi-rule.integer-bitwise-xor-constants.v1", ExactOperationSemantics, IntegerBinaryKind::BitwiseXor);
        SaturatingIntegerAddConstantsRule = (b"omega.psi-rule.saturating-integer-add-constants.v1", ExactOperationSemantics, IntegerBinaryKind::Arithmetic(Policy::Saturating, Operator::Add));
        SaturatingIntegerDivideConstantsRule = (b"omega.psi-rule.saturating-integer-divide-constants.v1", ProofCertified, IntegerBinaryKind::Arithmetic(Policy::Saturating, Operator::Divide));
        SaturatingIntegerMultiplyConstantsRule = (b"omega.psi-rule.saturating-integer-multiply-constants.v1", ExactOperationSemantics, IntegerBinaryKind::Arithmetic(Policy::Saturating, Operator::Multiply));
        SaturatingIntegerRemainderConstantsRule = (b"omega.psi-rule.saturating-integer-remainder-constants.v1", ProofCertified, IntegerBinaryKind::Arithmetic(Policy::Saturating, Operator::Remainder));
        SaturatingIntegerSubtractConstantsRule = (b"omega.psi-rule.saturating-integer-subtract-constants.v1", ExactOperationSemantics, IntegerBinaryKind::Arithmetic(Policy::Saturating, Operator::Subtract));
        WrappingIntegerAddConstantsRule = (b"omega.psi-rule.wrapping-integer-add-constants.v1", ExactOperationSemantics, IntegerBinaryKind::Arithmetic(Policy::Wrapping, Operator::Add));
        WrappingIntegerDivideConstantsRule = (b"omega.psi-rule.wrapping-integer-divide-constants.v1", ProofCertified, IntegerBinaryKind::Arithmetic(Policy::Wrapping, Operator::Divide));
        WrappingIntegerMultiplyConstantsRule = (b"omega.psi-rule.wrapping-integer-multiply-constants.v1", ExactOperationSemantics, IntegerBinaryKind::Arithmetic(Policy::Wrapping, Operator::Multiply));
        WrappingIntegerRemainderConstantsRule = (b"omega.psi-rule.wrapping-integer-remainder-constants.v1", ProofCertified, IntegerBinaryKind::Arithmetic(Policy::Wrapping, Operator::Remainder));
        WrappingIntegerShiftLeftConstantsRule = (b"omega.psi-rule.wrapping-integer-shift-left-constants.v1", ExactOperationSemantics, IntegerBinaryKind::Shift(Policy::Wrapping, Direction::Left));
        WrappingIntegerShiftRightConstantsRule = (b"omega.psi-rule.wrapping-integer-shift-right-constants.v1", ExactOperationSemantics, IntegerBinaryKind::Shift(Policy::Wrapping, Direction::Right));
        WrappingIntegerSubtractConstantsRule = (b"omega.psi-rule.wrapping-integer-subtract-constants.v1", ExactOperationSemantics, IntegerBinaryKind::Arithmetic(Policy::Wrapping, Operator::Subtract));
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum IntegerBinaryKind {
        Arithmetic(IntegerOverflowPolicy, IntegerArithmeticOperator),
        Shift(IntegerOverflowPolicy, IntegerShiftDirection),
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum IntegerBinaryOperation {
        Arithmetic(IntegerArithmetic),
        Shift {
            policy: IntegerShiftPolicy,
            direction: IntegerShiftDirection,
            count_type: IntegerType,
        },
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
    }

    impl IntegerBinaryOperation {
        fn kind(self) -> IntegerBinaryKind {
            match self {
                Self::Arithmetic(arithmetic) => {
                    IntegerBinaryKind::Arithmetic(arithmetic.policy(), arithmetic.operator())
                }
                Self::Shift {
                    policy, direction, ..
                } => IntegerBinaryKind::Shift(policy.overflow_policy(), direction),
                Self::BitwiseAnd => IntegerBinaryKind::BitwiseAnd,
                Self::BitwiseOr => IntegerBinaryKind::BitwiseOr,
                Self::BitwiseXor => IntegerBinaryKind::BitwiseXor,
            }
        }
    }

    pub(crate) struct IntegerBinaryShape {
        pub(crate) source: OperationId,
        pub(crate) result: ValueId,
        pub(crate) scalar_type: IntegerType,
        pub(crate) left: ValueId,
        pub(crate) right: ValueId,
        pub(crate) operation: IntegerBinaryOperation,
    }

    fn propose(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        kind: IntegerBinaryKind,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::propose_integer_constant_folds(
            unit,
            analyses,
            contract,
            |unit, machine, node, constants| {
                let Some(shape) = integer_binary_shapes::classify(&node.operation) else {
                    return Ok(None);
                };
                if shape.operation.kind() != kind {
                    return Ok(None);
                }
                let Some((left_value, left_fact)) =
                    integer_constant(constants, machine, shape.left)
                else {
                    return Ok(None);
                };
                let Some((right_value, right_fact)) =
                    integer_constant(constants, machine, shape.right)
                else {
                    return Ok(None);
                };
                let Some(constant) = evaluate(&shape, left_value, right_value) else {
                    return Ok(None);
                };
                Ok(Some(super::IntegerConstantFold {
                    source_operation: shape.source,
                    result: shape.result,
                    scalar_type: shape.scalar_type,
                    constant,
                    witness: witness(
                        unit,
                        machine,
                        shape.source,
                        contract.safety_class(),
                        left_fact,
                        right_fact,
                    )?,
                }))
            },
        )
    }

    fn evaluate(
        shape: &IntegerBinaryShape,
        left: IntegerValue,
        right: IntegerValue,
    ) -> Option<IntegerValue> {
        match shape.operation {
            IntegerBinaryOperation::Arithmetic(arithmetic) => {
                arithmetic.evaluate(shape.scalar_type, left, right)
            }
            IntegerBinaryOperation::Shift {
                policy,
                direction,
                count_type,
            } => policy.evaluate(direction, shape.scalar_type, left, count_type, right),
            IntegerBinaryOperation::BitwiseAnd => shape.scalar_type.bitwise_and(left, right),
            IntegerBinaryOperation::BitwiseOr => shape.scalar_type.bitwise_or(left, right),
            IntegerBinaryOperation::BitwiseXor => shape.scalar_type.bitwise_xor(left, right),
        }
    }

    fn witness(
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        operation: OperationId,
        safety: OptimizationSafetyClass,
        left_fact: ScalarConstantFactIdentity,
        right_fact: ScalarConstantFactIdentity,
    ) -> Result<IntegerEvaluationWitness, RuleProposalError> {
        if safety == OptimizationSafetyClass::ProofCertified {
            Ok(IntegerEvaluationWitness::ProofCertifiedBinary {
                left_fact,
                right_fact,
                obligation_fact: accepted_obligation_fact(unit, machine, operation)?,
            })
        } else {
            Ok(IntegerEvaluationWitness::Binary {
                left_fact,
                right_fact,
            })
        }
    }
}
pub(crate) mod integer_binary_shapes {
    use abstract_operations::AbstractOperation as O;

    use crate::rules::sparse_conditional_constant_propagation::integer_binary_constants::{
        IntegerBinaryOperation, IntegerBinaryShape,
    };

    pub(crate) fn classify(operation: &O) -> Option<IntegerBinaryShape> {
        let (source, result, scalar_type, left, right, operation) = match operation {
            O::IntegerShift {
                psi_operation,
                result,
                value_type,
                count_type,
                policy,
                direction,
                value,
                count,
            } => (
                *psi_operation,
                *result,
                *value_type,
                *value,
                *count,
                IntegerBinaryOperation::Shift {
                    policy: *policy,
                    direction: *direction,
                    count_type: *count_type,
                },
            ),
            O::IntegerArithmetic {
                psi_operation,
                result,
                scalar_type,
                arithmetic,
                left,
                right,
            } => (
                *psi_operation,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryOperation::Arithmetic(*arithmetic),
            ),
            O::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => (
                *psi_operation,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryOperation::BitwiseAnd,
            ),
            O::IntegerBitwiseOr {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => (
                *psi_operation,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryOperation::BitwiseOr,
            ),
            O::IntegerBitwiseXor {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => (
                *psi_operation,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryOperation::BitwiseXor,
            ),
            _ => return None,
        };
        Some(IntegerBinaryShape {
            source,
            result,
            scalar_type,
            left,
            right,
            operation,
        })
    }
}
pub(crate) mod integer_cast_constants {
    use abstract_operations::AbstractOperation as O;
    use optimization_core::{
        OptimizationRuleContract, OptimizationSafetyClass, ScalarConstantFactIdentity,
    };
    use optimization_unit::{IntegerEvaluationWitness, PsiOptimizationUnit, PsiRewriteCandidate};
    use semantic_vocabulary::{MachineId, OperationId};

    use crate::rules::accepted_obligations::accepted_obligation_fact;
    use crate::rules::sparse_conditional_constant_propagation::{
        constant_evaluation_contract, integer_constant,
    };
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    psi_rule_contract!(
        ExactIntegerCastConstantsRule,
        constant_evaluation_contract(
            b"omega.psi-rule.exact-integer-cast-constants.v1",
            OptimizationSafetyClass::ProofCertified,
        ),
        propose = propose,
    );

    fn propose(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::propose_integer_constant_folds(
            unit,
            analyses,
            contract,
            |unit, machine, node, constants| {
                let O::IntegerExactCast {
                    psi_operation,
                    result,
                    source_type,
                    target_type,
                    operand,
                    ..
                } = node.operation
                else {
                    return Ok(None);
                };
                let Some((operand_value, operand_fact)) =
                    integer_constant(constants, machine, operand)
                else {
                    return Ok(None);
                };
                let Some(constant) = source_type.exact_cast_value_to(target_type, operand_value)
                else {
                    return Ok(None);
                };
                Ok(Some(super::IntegerConstantFold {
                    source_operation: psi_operation,
                    result,
                    scalar_type: target_type,
                    constant,
                    witness: proof_certified_unary_witness(
                        unit,
                        machine,
                        psi_operation,
                        operand_fact,
                    )?,
                }))
            },
        )
    }

    fn proof_certified_unary_witness(
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        operation: OperationId,
        operand_fact: ScalarConstantFactIdentity,
    ) -> Result<IntegerEvaluationWitness, RuleProposalError> {
        Ok(IntegerEvaluationWitness::ProofCertifiedUnary {
            operand_fact,
            obligation_fact: accepted_obligation_fact(unit, machine, operation)?,
        })
    }
}
pub(crate) mod integer_unary_constants {
    use abstract_operations::AbstractOperation as O;
    use optimization_core::{OptimizationRuleContract, OptimizationSafetyClass};
    use optimization_unit::{IntegerEvaluationWitness, PsiOptimizationUnit, PsiRewriteCandidate};

    use crate::rules::sparse_conditional_constant_propagation::{
        constant_evaluation_contract as contract, integer_constant,
    };
    use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

    constant_fold_rules! {
        IntegerBitwiseNotConstantsRule = (b"omega.psi-rule.integer-bitwise-not-constants.v1", ExactOperationSemantics, IntegerUnaryKind::BitwiseNot);
        IntegerWidenConstantsRule = (b"omega.psi-rule.integer-widen-constants.v1", ExactOperationSemantics, IntegerUnaryKind::Widen);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum IntegerUnaryKind {
        Widen,
        BitwiseNot,
    }

    fn propose(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        kind: IntegerUnaryKind,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::propose_integer_constant_folds(
            unit,
            analyses,
            contract,
            |_, machine, node, constants| {
                let (source_operation, result, source_type, target_type, operand) =
                    match (&node.operation, kind) {
                        (
                            O::IntegerWiden {
                                psi_operation,
                                result,
                                source_type,
                                target_type,
                                operand,
                            },
                            IntegerUnaryKind::Widen,
                        ) => (
                            *psi_operation,
                            *result,
                            *source_type,
                            *target_type,
                            *operand,
                        ),
                        (
                            O::IntegerBitwiseNot {
                                psi_operation,
                                result,
                                scalar_type,
                                operand,
                            },
                            IntegerUnaryKind::BitwiseNot,
                        ) => (
                            *psi_operation,
                            *result,
                            *scalar_type,
                            *scalar_type,
                            *operand,
                        ),
                        _ => return Ok(None),
                    };
                let Some((operand_value, operand_fact)) =
                    integer_constant(constants, machine, operand)
                else {
                    return Ok(None);
                };
                let constant = match kind {
                    IntegerUnaryKind::Widen => {
                        source_type.widen_value_to(target_type, operand_value)
                    }
                    IntegerUnaryKind::BitwiseNot => source_type.bitwise_not(operand_value),
                };
                let Some(constant) = constant else {
                    return Ok(None);
                };
                Ok(Some(super::IntegerConstantFold {
                    source_operation,
                    result,
                    scalar_type: target_type,
                    constant,
                    witness: IntegerEvaluationWitness::Unary { operand_fact },
                }))
            },
        )
    }
}
pub(crate) mod range_against_constant {
    use abstract_operations::AbstractOperation as O;
    use optimization_core::{
        AnalysisKind, AnalysisSet, OptimizationRuleContract, OptimizationSafetyClass,
    };
    use optimization_unit::{IntegerEvaluationWitness, PsiOptimizationUnit, PsiRewriteCandidate};
    use semantic_vocabulary::{IntegerType, IntegerValue};

    use super::IntegerRangeComparisonKind;
    use crate::rules::sparse_conditional_constant_propagation::{
        integer_value_type, range_comparison_contract,
    };
    use crate::{
        AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
        ScalarConstantAnalysis, ValueRangeAnalysis,
    };

    fn contract(
        identity: &'static [u8],
        _safety: OptimizationSafetyClass,
    ) -> OptimizationRuleContract {
        range_comparison_contract(
            identity,
            AnalysisSet::new([AnalysisKind::ScalarConstants, AnalysisKind::ValueRanges]),
        )
    }

    fn propose(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        kind: IntegerRangeComparisonKind,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
        required_analysis!(analyses, AnalysisKind::ValueRanges => Some(AnalysisProduct::ValueRanges(ranges)));
        propose_over(unit, constants, ranges, contract, kind)
    }

    constant_fold_rules! {
        IntegerEqualConstantRangeRule = (b"omega.psi-rule.integer-equal-constant-range.v1", ProofCertified, IntegerRangeComparisonKind::ConstantEqualRange);
        IntegerEqualRangeConstantRule = (b"omega.psi-rule.integer-equal-range-constant.v1", ProofCertified, IntegerRangeComparisonKind::RangeEqualConstant);
        IntegerLessOrEqualConstantRangeRule = (b"omega.psi-rule.integer-less-or-equal-constant-range.v1", ProofCertified, IntegerRangeComparisonKind::ConstantLessOrEqualRange);
        IntegerLessOrEqualRangeConstantRule = (b"omega.psi-rule.integer-less-or-equal-range-constant.v1", ProofCertified, IntegerRangeComparisonKind::RangeLessOrEqualConstant);
        IntegerLessThanConstantRangeRule = (b"omega.psi-rule.integer-less-than-constant-range.v1", ProofCertified, IntegerRangeComparisonKind::ConstantLessThanRange);
        IntegerLessThanRangeConstantRule = (b"omega.psi-rule.integer-less-than-range-constant.v1", ProofCertified, IntegerRangeComparisonKind::RangeLessThanConstant);
    }

    fn propose_over(
        unit: &PsiOptimizationUnit,
        constants: &ScalarConstantAnalysis,
        ranges: &ValueRangeAnalysis,
        contract: OptimizationRuleContract,
        kind: IntegerRangeComparisonKind,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::propose_boolean_evaluations(unit, contract, |function, block, node, node_index| {
            let (psi_operation, result, left, right) = match (kind, &node.operation) {
                (
                    IntegerRangeComparisonKind::RangeEqualConstant
                    | IntegerRangeComparisonKind::ConstantEqualRange,
                    O::IntegerEqual {
                        psi_operation,
                        result,
                        left,
                        right,
                    },
                )
                | (
                    IntegerRangeComparisonKind::RangeLessThanConstant
                    | IntegerRangeComparisonKind::ConstantLessThanRange,
                    O::IntegerLessThan {
                        psi_operation,
                        result,
                        left,
                        right,
                    },
                )
                | (
                    IntegerRangeComparisonKind::RangeLessOrEqualConstant
                    | IntegerRangeComparisonKind::ConstantLessOrEqualRange,
                    O::IntegerLessOrEqual {
                        psi_operation,
                        result,
                        left,
                        right,
                    },
                ) => (*psi_operation, *result, *left, *right),
                _ => return None,
            };
            let (range_value, constant_operand) = match kind {
                IntegerRangeComparisonKind::RangeEqualConstant
                | IntegerRangeComparisonKind::RangeLessThanConstant
                | IntegerRangeComparisonKind::RangeLessOrEqualConstant => (left, right),
                IntegerRangeComparisonKind::ConstantEqualRange
                | IntegerRangeComparisonKind::ConstantLessThanRange
                | IntegerRangeComparisonKind::ConstantLessOrEqualRange => (right, left),
            };
            let (constant_value, constant_fact) =
                constants.literal_integer_constant(function.machine, constant_operand)?;
            let scalar_type = integer_value_type(function, range_value)?;
            if integer_value_type(function, constant_operand) != Some(scalar_type) {
                return None;
            }
            let range = ranges.facts.iter().find(|fact| {
                fact.valid_in.machine == function.machine
                    && fact.value == range_value
                    && fact.scalar_type == scalar_type
                    && ranges.fact_applies_at(fact, unit, function.machine, block, node_index)
            })?;
            let constant = evaluate(
                kind,
                scalar_type,
                range.minimum,
                range.maximum,
                constant_value,
            )?;
            Some(super::BooleanConstantEvaluation {
                source_operation: psi_operation,
                result,
                constant,
                witness: IntegerEvaluationWitness::RangeAgainstConstant {
                    range_fact: range.identity,
                    constant_fact,
                },
            })
        })
    }

    pub(crate) fn evaluate(
        kind: IntegerRangeComparisonKind,
        scalar_type: IntegerType,
        minimum: IntegerValue,
        maximum: IntegerValue,
        constant: IntegerValue,
    ) -> Option<bool> {
        let minimum_to_constant = scalar_type.compare(minimum, constant)?;
        let maximum_to_constant = scalar_type.compare(maximum, constant)?;
        match kind {
            IntegerRangeComparisonKind::RangeEqualConstant
            | IntegerRangeComparisonKind::ConstantEqualRange => (minimum_to_constant.is_eq()
                && maximum_to_constant.is_eq())
            .then_some(true)
            .or_else(|| {
                (minimum_to_constant.is_gt() || maximum_to_constant.is_lt()).then_some(false)
            }),
            IntegerRangeComparisonKind::RangeLessThanConstant => maximum_to_constant
                .is_lt()
                .then_some(true)
                .or_else(|| (!minimum_to_constant.is_lt()).then_some(false)),
            IntegerRangeComparisonKind::ConstantLessThanRange => minimum_to_constant
                .is_gt()
                .then_some(true)
                .or_else(|| (!maximum_to_constant.is_gt()).then_some(false)),
            IntegerRangeComparisonKind::RangeLessOrEqualConstant => (!maximum_to_constant.is_gt())
                .then_some(true)
                .or_else(|| minimum_to_constant.is_gt().then_some(false)),
            IntegerRangeComparisonKind::ConstantLessOrEqualRange => (!minimum_to_constant.is_lt())
                .then_some(true)
                .or_else(|| maximum_to_constant.is_lt().then_some(false)),
        }
    }
}
pub(crate) mod range_against_range {
    use abstract_operations::AbstractOperation as O;
    use optimization_core::{
        AnalysisKind, AnalysisSet, OptimizationRuleContract, OptimizationSafetyClass,
    };
    use optimization_unit::{IntegerEvaluationWitness, PsiOptimizationUnit, PsiRewriteCandidate};
    use semantic_vocabulary::{IntegerType, IntegerValue};

    use super::IntegerRangePairComparisonKind;
    use crate::rules::sparse_conditional_constant_propagation::{
        integer_value_type, range_comparison_contract,
    };
    use crate::{
        AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
        ValueRangeAnalysis,
    };

    fn contract(
        identity: &'static [u8],
        _safety: OptimizationSafetyClass,
    ) -> OptimizationRuleContract {
        range_comparison_contract(identity, AnalysisSet::new([AnalysisKind::ValueRanges]))
    }

    fn propose(
        unit: &PsiOptimizationUnit,
        analyses: RuleAnalysisView<'_>,
        contract: OptimizationRuleContract,
        kind: IntegerRangePairComparisonKind,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        required_analysis!(analyses, AnalysisKind::ValueRanges => Some(AnalysisProduct::ValueRanges(ranges)));
        propose_over(unit, ranges, contract, kind)
    }

    constant_fold_rules! {
        IntegerEqualRangeRangeRule = (b"omega.psi-rule.integer-equal-range-range.v1", ProofCertified, IntegerRangePairComparisonKind::Equal);
        IntegerLessOrEqualRangeRangeRule = (b"omega.psi-rule.integer-less-or-equal-range-range.v1", ProofCertified, IntegerRangePairComparisonKind::LessOrEqual);
        IntegerLessThanRangeRangeRule = (b"omega.psi-rule.integer-less-than-range-range.v1", ProofCertified, IntegerRangePairComparisonKind::LessThan);
    }

    fn propose_over(
        unit: &PsiOptimizationUnit,
        ranges: &ValueRangeAnalysis,
        contract: OptimizationRuleContract,
        kind: IntegerRangePairComparisonKind,
    ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
        super::propose_boolean_evaluations(unit, contract, |function, block, node, node_index| {
            let (psi_operation, result, left, right) = match (kind, &node.operation) {
                (
                    IntegerRangePairComparisonKind::Equal,
                    O::IntegerEqual {
                        psi_operation,
                        result,
                        left,
                        right,
                    },
                )
                | (
                    IntegerRangePairComparisonKind::LessThan,
                    O::IntegerLessThan {
                        psi_operation,
                        result,
                        left,
                        right,
                    },
                )
                | (
                    IntegerRangePairComparisonKind::LessOrEqual,
                    O::IntegerLessOrEqual {
                        psi_operation,
                        result,
                        left,
                        right,
                    },
                ) => (*psi_operation, *result, *left, *right),
                _ => return None,
            };
            let scalar_type = integer_value_type(function, left)?;
            if integer_value_type(function, right) != Some(scalar_type) {
                return None;
            }
            let proof_range = |value| {
                ranges.facts.iter().find(|fact| {
                    fact.valid_in.machine == function.machine
                        && fact.value == value
                        && fact.scalar_type == scalar_type
                        && ranges.fact_applies_at(fact, unit, function.machine, block, node_index)
                })
            };
            let (left_range, right_range) = (proof_range(left)?, proof_range(right)?);
            let constant = evaluate(
                kind,
                scalar_type,
                left == right,
                left_range.minimum,
                left_range.maximum,
                right_range.minimum,
                right_range.maximum,
            )?;
            Some(super::BooleanConstantEvaluation {
                source_operation: psi_operation,
                result,
                constant,
                witness: IntegerEvaluationWitness::RangeAgainstRange {
                    left_range_fact: left_range.identity,
                    right_range_fact: right_range.identity,
                },
            })
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn evaluate(
        kind: IntegerRangePairComparisonKind,
        scalar_type: IntegerType,
        same_value: bool,
        left_minimum: IntegerValue,
        left_maximum: IntegerValue,
        right_minimum: IntegerValue,
        right_maximum: IntegerValue,
    ) -> Option<bool> {
        if same_value {
            return Some(!matches!(kind, IntegerRangePairComparisonKind::LessThan));
        }
        let left_maximum_to_right_minimum = scalar_type.compare(left_maximum, right_minimum)?;
        let left_minimum_to_right_maximum = scalar_type.compare(left_minimum, right_maximum)?;
        match kind {
            IntegerRangePairComparisonKind::Equal => {
                let both_equal_singletons =
                    scalar_type.compare(left_minimum, left_maximum)?.is_eq()
                        && scalar_type.compare(right_minimum, right_maximum)?.is_eq()
                        && scalar_type.compare(left_minimum, right_minimum)?.is_eq();
                both_equal_singletons.then_some(true).or_else(|| {
                    (left_maximum_to_right_minimum.is_lt() || left_minimum_to_right_maximum.is_gt())
                        .then_some(false)
                })
            }
            IntegerRangePairComparisonKind::LessThan => left_maximum_to_right_minimum
                .is_lt()
                .then_some(true)
                .or_else(|| (!left_minimum_to_right_maximum.is_lt()).then_some(false)),
            IntegerRangePairComparisonKind::LessOrEqual => (!left_maximum_to_right_minimum.is_gt())
                .then_some(true)
                .or_else(|| left_minimum_to_right_maximum.is_gt().then_some(false)),
        }
    }
}

pub use boolean_constants::{
    BooleanEqualConstantsRule, BooleanNotConstantsRule, IntegerEqualConstantsRule,
    IntegerLessOrEqualConstantsRule, IntegerLessThanConstantsRule,
};
pub use integer_binary_constants::{
    ExactIntegerAddConstantsRule, ExactIntegerDivideConstantsRule,
    ExactIntegerMultiplyConstantsRule, ExactIntegerRemainderConstantsRule,
    ExactIntegerShiftLeftConstantsRule, ExactIntegerShiftRightConstantsRule,
    ExactIntegerSubtractConstantsRule, IntegerBitwiseAndConstantsRule,
    IntegerBitwiseOrConstantsRule, IntegerBitwiseXorConstantsRule,
    SaturatingIntegerAddConstantsRule, SaturatingIntegerDivideConstantsRule,
    SaturatingIntegerMultiplyConstantsRule, SaturatingIntegerRemainderConstantsRule,
    SaturatingIntegerSubtractConstantsRule, WrappingIntegerAddConstantsRule,
    WrappingIntegerDivideConstantsRule, WrappingIntegerMultiplyConstantsRule,
    WrappingIntegerRemainderConstantsRule, WrappingIntegerShiftLeftConstantsRule,
    WrappingIntegerShiftRightConstantsRule, WrappingIntegerSubtractConstantsRule,
};
pub(crate) use integer_cast_constants::ExactIntegerCastConstantsRule;
pub use integer_unary_constants::{IntegerBitwiseNotConstantsRule, IntegerWidenConstantsRule};
pub use range_against_constant::{
    IntegerEqualConstantRangeRule, IntegerEqualRangeConstantRule,
    IntegerLessOrEqualConstantRangeRule, IntegerLessOrEqualRangeConstantRule,
    IntegerLessThanConstantRangeRule, IntegerLessThanRangeConstantRule,
};
pub use range_against_range::{
    IntegerEqualRangeRangeRule, IntegerLessOrEqualRangeRangeRule, IntegerLessThanRangeRangeRule,
};

use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationRuleContract,
    OptimizationSafetyClass, ScalarConstantFactIdentity,
};
use optimization_unit::{
    BooleanConstantRewrite, IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation,
    PsiOptimizationUnit, PsiRewriteCandidate,
};
use semantic_vocabulary::{BlockId, IntegerType, IntegerValue, MachineId, OperationId, ValueId};

use crate::rules::SCCP_PASS_NAME;
use crate::rules::catalog::BuiltInRuleRegistration;
use crate::rules::control_flow_cleanup::node_realized_provenance;
use crate::{
    AnalysisProduct, RuleAnalysisView, RuleProposalError, ScalarConstantAnalysis,
    ScalarConstantValue,
};

pub(crate) fn built_in_registrations() -> Vec<BuiltInRuleRegistration> {
    vec![
        BuiltInRuleRegistration::new(0, ExactIntegerAddConstantsRule),
        BuiltInRuleRegistration::new(1, ExactIntegerSubtractConstantsRule),
        BuiltInRuleRegistration::new(2, ExactIntegerMultiplyConstantsRule),
        BuiltInRuleRegistration::new(3, WrappingIntegerAddConstantsRule),
        BuiltInRuleRegistration::new(4, WrappingIntegerSubtractConstantsRule),
        BuiltInRuleRegistration::new(5, WrappingIntegerMultiplyConstantsRule),
        BuiltInRuleRegistration::new(6, SaturatingIntegerAddConstantsRule),
        BuiltInRuleRegistration::new(7, SaturatingIntegerSubtractConstantsRule),
        BuiltInRuleRegistration::new(8, SaturatingIntegerMultiplyConstantsRule),
        BuiltInRuleRegistration::new(9, ExactIntegerDivideConstantsRule),
        BuiltInRuleRegistration::new(10, ExactIntegerRemainderConstantsRule),
        BuiltInRuleRegistration::new(11, WrappingIntegerDivideConstantsRule),
        BuiltInRuleRegistration::new(12, WrappingIntegerRemainderConstantsRule),
        BuiltInRuleRegistration::new(13, SaturatingIntegerDivideConstantsRule),
        BuiltInRuleRegistration::new(14, SaturatingIntegerRemainderConstantsRule),
        BuiltInRuleRegistration::new(15, ExactIntegerShiftLeftConstantsRule),
        BuiltInRuleRegistration::new(16, ExactIntegerShiftRightConstantsRule),
        BuiltInRuleRegistration::new(17, WrappingIntegerShiftLeftConstantsRule),
        BuiltInRuleRegistration::new(18, WrappingIntegerShiftRightConstantsRule),
        BuiltInRuleRegistration::new(19, ExactIntegerCastConstantsRule),
        BuiltInRuleRegistration::new(20, IntegerWidenConstantsRule),
        BuiltInRuleRegistration::new(21, IntegerBitwiseNotConstantsRule),
        BuiltInRuleRegistration::new(22, IntegerBitwiseAndConstantsRule),
        BuiltInRuleRegistration::new(23, IntegerBitwiseOrConstantsRule),
        BuiltInRuleRegistration::new(24, IntegerBitwiseXorConstantsRule),
        BuiltInRuleRegistration::new(25, BooleanNotConstantsRule),
        BuiltInRuleRegistration::new(26, BooleanEqualConstantsRule),
        BuiltInRuleRegistration::new(27, IntegerEqualConstantsRule),
        BuiltInRuleRegistration::new(28, IntegerLessThanConstantsRule),
        BuiltInRuleRegistration::new(29, IntegerLessOrEqualConstantsRule),
        BuiltInRuleRegistration::new(30, IntegerLessThanRangeConstantRule),
        BuiltInRuleRegistration::new(31, IntegerLessThanConstantRangeRule),
        BuiltInRuleRegistration::new(32, IntegerLessOrEqualRangeConstantRule),
        BuiltInRuleRegistration::new(33, IntegerLessOrEqualConstantRangeRule),
        BuiltInRuleRegistration::new(34, IntegerEqualRangeConstantRule),
        BuiltInRuleRegistration::new(35, IntegerEqualConstantRangeRule),
        BuiltInRuleRegistration::new(36, IntegerEqualRangeRangeRule),
        BuiltInRuleRegistration::new(37, IntegerLessThanRangeRangeRule),
        BuiltInRuleRegistration::new(38, IntegerLessOrEqualRangeRangeRule),
    ]
}

pub(crate) fn constant_evaluation_contract(
    rule_name: &'static [u8],
    safety_class: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        rule_name,
        SCCP_PASS_NAME,
        1,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        safety_class,
    )
}

pub(crate) struct IntegerConstantFold {
    pub(crate) source_operation: OperationId,
    pub(crate) result: ValueId,
    pub(crate) scalar_type: IntegerType,
    pub(crate) constant: IntegerValue,
    pub(crate) witness: IntegerEvaluationWitness,
}

pub(crate) fn propose_integer_constant_folds(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    fold: impl Fn(
        &PsiOptimizationUnit,
        MachineId,
        &optimization_unit::OptimizationNode,
        &ScalarConstantAnalysis,
    ) -> Result<Option<IntegerConstantFold>, RuleProposalError>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    required_analysis!(analyses, AnalysisKind::ScalarConstants => Some(AnalysisProduct::ScalarConstants(constants)));
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some(row) = fold(unit, function.machine, node, constants)? else {
                    continue;
                };
                let location = node_location(
                    function.machine,
                    block.id,
                    u32::try_from(node_index).expect("optimization node indices are u32"),
                );
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![node_realized_provenance(location, node)],
                        row.witness,
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: row.source_operation,
                            result: row.result,
                            scalar_type: row.scalar_type,
                            constant: row.constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

pub(crate) struct BooleanConstantEvaluation {
    pub(crate) source_operation: OperationId,
    pub(crate) result: ValueId,
    pub(crate) constant: bool,
    pub(crate) witness: IntegerEvaluationWitness,
}

pub(crate) fn propose_boolean_evaluations(
    unit: &PsiOptimizationUnit,
    contract: OptimizationRuleContract,
    evaluate: impl Fn(
        &optimization_unit::PsiOptimizationFunction,
        BlockId,
        &optimization_unit::OptimizationNode,
        u32,
    ) -> Option<BooleanConstantEvaluation>,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let node_index =
                    u32::try_from(node_index).expect("optimization node indices are u32");
                let Some(row) = evaluate(function, block.id, node, node_index) else {
                    continue;
                };
                let location = node_location(function.machine, block.id, node_index);
                candidates.push(
                    PsiRewriteCandidate::new_boolean_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![node_realized_provenance(location, node)],
                        row.witness,
                        -1,
                        BooleanConstantRewrite {
                            location,
                            source_operation: row.source_operation,
                            result: row.result,
                            constant: row.constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
}

pub(crate) fn range_comparison_contract(
    identity: &'static [u8],
    analyses: AnalysisSet,
) -> OptimizationRuleContract {
    crate::rules::standard_rule_contract(
        identity,
        SCCP_PASS_NAME,
        1,
        analyses,
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition]),
        OptimizationSafetyClass::ProofCertified,
    )
}

pub(crate) fn integer_constant(
    constants: &ScalarConstantAnalysis,
    machine: MachineId,
    value: ValueId,
) -> Option<(IntegerValue, ScalarConstantFactIdentity)> {
    constants.facts.iter().find_map(|fact| {
        (fact.valid_in.machine == machine && fact.value == value)
            .then_some(fact)
            .and_then(|fact| match fact.constant {
                ScalarConstantValue::Integer(value) => {
                    fact.identity.map(|identity| (value, identity))
                }
                ScalarConstantValue::Boolean(_) => None,
            })
    })
}

pub(crate) fn integer_value_type(
    function: &optimization_unit::PsiOptimizationFunction,
    value: ValueId,
) -> Option<semantic_vocabulary::IntegerType> {
    function
        .parameters
        .iter()
        .chain(function.block_parameters())
        .chain(function.node_definitions())
        .find_map(|definition| {
            (definition.value == value)
                .then_some(definition.scalar_type)
                .and_then(|scalar_type| match scalar_type {
                    semantic_vocabulary::ScalarType::Integer(integer) => Some(integer),
                    semantic_vocabulary::ScalarType::Boolean
                    | semantic_vocabulary::ScalarType::IeeeFloat(_) => None,
                })
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegerRangeComparisonKind {
    RangeEqualConstant,
    ConstantEqualRange,
    RangeLessThanConstant,
    ConstantLessThanRange,
    RangeLessOrEqualConstant,
    ConstantLessOrEqualRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntegerRangePairComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

pub(crate) fn node_location(machine: MachineId, block: BlockId, node: u32) -> NodeLocation {
    NodeLocation {
        machine,
        block,
        node,
    }
}
