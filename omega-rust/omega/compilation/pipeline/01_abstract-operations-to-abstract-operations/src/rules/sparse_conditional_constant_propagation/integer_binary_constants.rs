//! The 22 binary integer constant folds: exact, wrapping and saturating
//! arithmetic, quotient and remainder, shifts, and bitwise operations, each
//! one row of the table below over the one traversal in `propose`.
//!
//! `IntegerBinaryKind` is the closed rule vocabulary the table names: an
//! arithmetic or shift policy pair, or one bitwise operator.
//! `integer_binary_shapes::classify` recognizes a node's shape and the exact
//! operation it computes; `evaluate` folds two constants under that
//! operation; `witness` binds the constant facts (and, for proof-certified
//! kinds, the accepted obligation) the rewrite carries. The pass owner remains
//! the only local rule-order point.

use abstract_operations::{
    IntegerArithmetic, IntegerArithmeticOperator, IntegerArithmeticOperator as Operator,
    IntegerOverflowPolicy, IntegerOverflowPolicy as Policy, IntegerShiftDirection,
    IntegerShiftDirection as Direction, IntegerShiftPolicy,
};
use optimization_core::{
    AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass, ScalarConstantFactIdentity,
};
use optimization_unit::{
    IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use semantic_vocabulary::{IntegerType, IntegerValue, MachineId, OperationId, ValueId};

use super::integer_binary_shapes;
use crate::rules::accepted_obligations::accepted_obligation_fact;
use crate::rules::sparse_conditional_constant_propagation::{
    constant_evaluation_contract, integer_constant,
};
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

fn contract(rule_name: &[u8], safety: OptimizationSafetyClass) -> OptimizationRuleContract {
    constant_evaluation_contract(rule_name, safety)
}

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

/// The rule vocabulary: which constant-fold rule owns a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntegerBinaryKind {
    Arithmetic(IntegerOverflowPolicy, IntegerArithmeticOperator),
    Shift(IntegerOverflowPolicy, IntegerShiftDirection),
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

/// The exact operation one binary integer node computes.
#[derive(Debug, Clone, Copy)]
pub(super) enum IntegerBinaryOperation {
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

/// One binary integer node: its source, result, carrier and operands. A
/// shift's value is `left` and its count `right`.
pub(super) struct IntegerBinaryShape {
    pub(super) source: OperationId,
    pub(super) result: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) left: ValueId,
    pub(super) right: ValueId,
    pub(super) operation: IntegerBinaryOperation,
}

fn propose(
    unit: &PsiOptimizationUnit,
    analyses: RuleAnalysisView<'_>,
    contract: OptimizationRuleContract,
    kind: IntegerBinaryKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        analyses.get(AnalysisKind::ScalarConstants)
    else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ScalarConstants,
        ));
    };
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some(shape) = integer_binary_shapes::classify(&node.operation) else {
                    continue;
                };
                if shape.operation.kind() != kind {
                    continue;
                }
                let Some((left_value, left_fact)) =
                    integer_constant(constants, function.machine, shape.left)
                else {
                    continue;
                };
                let Some((right_value, right_fact)) =
                    integer_constant(constants, function.machine, shape.right)
                else {
                    continue;
                };
                let Some(constant) = evaluate(&shape, left_value, right_value) else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("optimization node indices are u32"),
                };
                candidates.push(
                    PsiRewriteCandidate::new_integer_evaluation(
                        unit.identity,
                        contract,
                        vec![block.id],
                        Vec::new(),
                        vec![ProvenanceRewrite {
                            input: PsiRealizationSite::Node(location),
                            disposition: ProvenanceDisposition::RealizedAt(
                                PsiRealizationSite::Node(location),
                            ),
                            sources: node.provenance.clone(),
                            fuel: node.fuel.clone(),
                        }],
                        witness(
                            unit,
                            function.machine,
                            shape.source,
                            contract.safety_class(),
                            left_fact,
                            right_fact,
                        )?,
                        -1,
                        IntegerConstantRewrite {
                            location,
                            source_operation: shape.source,
                            result: shape.result,
                            scalar_type: shape.scalar_type,
                            constant,
                        },
                    )
                    .map_err(RuleProposalError::InvalidCandidate)?,
                );
            }
        }
    }
    Ok(candidates)
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
