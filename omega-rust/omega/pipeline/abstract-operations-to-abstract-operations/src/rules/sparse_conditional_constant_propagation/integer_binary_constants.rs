//! The 22 binary integer constant folds: exact, wrapping and saturating
//! arithmetic, quotient and remainder, shifts, and bitwise operations, each
//! one row of the table below over the one traversal in `propose`.
//!
//! `IntegerBinaryKind` is the closed operation vocabulary the table names;
//! `integer_binary_shapes::classify` recognizes a node's shape; `evaluate`
//! folds two constants under the kind's semantics; `witness` binds the
//! constant facts (and, for proof-certified kinds, the accepted obligation)
//! the rewrite carries. The pass owner remains the only local rule-order point.

use optimization_core::{
    AnalysisKind, OptimizationRuleContract, OptimizationSafetyClass, ScalarConstantFactIdentity,
};
use optimization_unit::{
    IntegerConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
};
use semantic_vocabulary::{IntegerType, IntegerValue, MachineId, OperationId, ValueId};

use super::integer_binary_shapes;
use crate::rules::sparse_conditional_constant_propagation::{
    constant_evaluation_contract, integer_constant,
};
use crate::rules::support::accepted_obligation_fact;
use crate::{AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

fn contract(rule_name: &[u8], safety: OptimizationSafetyClass) -> OptimizationRuleContract {
    constant_evaluation_contract(rule_name, safety)
}

constant_fold_rules! {
    ExactIntegerAddConstantsRule = (b"omega.psi-rule.exact-integer-add-constants.v1", ProofCertified, IntegerBinaryKind::ExactAdd);
    ExactIntegerDivideConstantsRule = (b"omega.psi-rule.exact-integer-divide-constants.v1", ProofCertified, IntegerBinaryKind::ExactDivide);
    ExactIntegerMultiplyConstantsRule = (b"omega.psi-rule.exact-integer-multiply-constants.v1", ProofCertified, IntegerBinaryKind::ExactMultiply);
    ExactIntegerRemainderConstantsRule = (b"omega.psi-rule.exact-integer-remainder-constants.v1", ProofCertified, IntegerBinaryKind::ExactRemainder);
    ExactIntegerShiftLeftConstantsRule = (b"omega.psi-rule.exact-integer-shift-left-constants.v1", ProofCertified, IntegerBinaryKind::ExactShiftLeft);
    ExactIntegerShiftRightConstantsRule = (b"omega.psi-rule.exact-integer-shift-right-constants.v1", ProofCertified, IntegerBinaryKind::ExactShiftRight);
    ExactIntegerSubtractConstantsRule = (b"omega.psi-rule.exact-integer-subtract-constants.v1", ProofCertified, IntegerBinaryKind::ExactSubtract);
    IntegerBitwiseAndConstantsRule = (b"omega.psi-rule.integer-bitwise-and-constants.v1", ExactOperationSemantics, IntegerBinaryKind::BitwiseAnd);
    IntegerBitwiseOrConstantsRule = (b"omega.psi-rule.integer-bitwise-or-constants.v1", ExactOperationSemantics, IntegerBinaryKind::BitwiseOr);
    IntegerBitwiseXorConstantsRule = (b"omega.psi-rule.integer-bitwise-xor-constants.v1", ExactOperationSemantics, IntegerBinaryKind::BitwiseXor);
    SaturatingIntegerAddConstantsRule = (b"omega.psi-rule.saturating-integer-add-constants.v1", ExactOperationSemantics, IntegerBinaryKind::SaturatingAdd);
    SaturatingIntegerDivideConstantsRule = (b"omega.psi-rule.saturating-integer-divide-constants.v1", ProofCertified, IntegerBinaryKind::SaturatingDivide);
    SaturatingIntegerMultiplyConstantsRule = (b"omega.psi-rule.saturating-integer-multiply-constants.v1", ExactOperationSemantics, IntegerBinaryKind::SaturatingMultiply);
    SaturatingIntegerRemainderConstantsRule = (b"omega.psi-rule.saturating-integer-remainder-constants.v1", ProofCertified, IntegerBinaryKind::SaturatingRemainder);
    SaturatingIntegerSubtractConstantsRule = (b"omega.psi-rule.saturating-integer-subtract-constants.v1", ExactOperationSemantics, IntegerBinaryKind::SaturatingSubtract);
    WrappingIntegerAddConstantsRule = (b"omega.psi-rule.wrapping-integer-add-constants.v1", ExactOperationSemantics, IntegerBinaryKind::WrappingAdd);
    WrappingIntegerDivideConstantsRule = (b"omega.psi-rule.wrapping-integer-divide-constants.v1", ProofCertified, IntegerBinaryKind::WrappingDivide);
    WrappingIntegerMultiplyConstantsRule = (b"omega.psi-rule.wrapping-integer-multiply-constants.v1", ExactOperationSemantics, IntegerBinaryKind::WrappingMultiply);
    WrappingIntegerRemainderConstantsRule = (b"omega.psi-rule.wrapping-integer-remainder-constants.v1", ProofCertified, IntegerBinaryKind::WrappingRemainder);
    WrappingIntegerShiftLeftConstantsRule = (b"omega.psi-rule.wrapping-integer-shift-left-constants.v1", ExactOperationSemantics, IntegerBinaryKind::WrappingShiftLeft);
    WrappingIntegerShiftRightConstantsRule = (b"omega.psi-rule.wrapping-integer-shift-right-constants.v1", ExactOperationSemantics, IntegerBinaryKind::WrappingShiftRight);
    WrappingIntegerSubtractConstantsRule = (b"omega.psi-rule.wrapping-integer-subtract-constants.v1", ExactOperationSemantics, IntegerBinaryKind::WrappingSubtract);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntegerBinaryKind {
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    WrappingAdd,
    WrappingSubtract,
    WrappingMultiply,
    SaturatingAdd,
    SaturatingSubtract,
    SaturatingMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    ExactShiftLeft,
    ExactShiftRight,
    WrappingShiftLeft,
    WrappingShiftRight,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

pub(super) struct IntegerBinaryShape {
    pub(super) source: OperationId,
    pub(super) result: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) left: ValueId,
    pub(super) right: ValueId,
    pub(super) count_type: Option<IntegerType>,
    pub(super) kind: IntegerBinaryKind,
}

impl IntegerBinaryShape {
    pub(super) fn scalar(
        source: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        left: ValueId,
        right: ValueId,
        kind: IntegerBinaryKind,
    ) -> Self {
        Self {
            source,
            result,
            scalar_type,
            left,
            right,
            count_type: None,
            kind,
        }
    }

    pub(super) fn shift(
        source: OperationId,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value: ValueId,
        count: ValueId,
        kind: IntegerBinaryKind,
    ) -> Self {
        Self {
            source,
            result,
            scalar_type: value_type,
            left: value,
            right: count,
            count_type: Some(count_type),
            kind,
        }
    }
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
                if shape.kind != kind {
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
    match shape.kind {
        IntegerBinaryKind::ExactAdd => shape.scalar_type.exact_add(left, right),
        IntegerBinaryKind::ExactSubtract => shape.scalar_type.exact_sub(left, right),
        IntegerBinaryKind::ExactMultiply => shape.scalar_type.exact_mul(left, right),
        IntegerBinaryKind::WrappingAdd => shape.scalar_type.wrapping_add(left, right),
        IntegerBinaryKind::WrappingSubtract => shape.scalar_type.wrapping_sub(left, right),
        IntegerBinaryKind::WrappingMultiply => shape.scalar_type.wrapping_mul(left, right),
        IntegerBinaryKind::SaturatingAdd => shape.scalar_type.saturating_add(left, right),
        IntegerBinaryKind::SaturatingSubtract => shape.scalar_type.saturating_sub(left, right),
        IntegerBinaryKind::SaturatingMultiply => shape.scalar_type.saturating_mul(left, right),
        IntegerBinaryKind::ExactDivide => shape.scalar_type.exact_div(left, right),
        IntegerBinaryKind::ExactRemainder => shape.scalar_type.exact_rem(left, right),
        IntegerBinaryKind::WrappingDivide => shape.scalar_type.wrapping_div(left, right),
        IntegerBinaryKind::WrappingRemainder => shape.scalar_type.wrapping_rem(left, right),
        IntegerBinaryKind::SaturatingDivide => shape.scalar_type.saturating_div(left, right),
        IntegerBinaryKind::SaturatingRemainder => shape.scalar_type.saturating_rem(left, right),
        IntegerBinaryKind::ExactShiftLeft => shape.scalar_type.exact_shift_left(
            left,
            shape.count_type.expect("shift count type"),
            right,
        ),
        IntegerBinaryKind::ExactShiftRight => shape.scalar_type.exact_shift_right(
            left,
            shape.count_type.expect("shift count type"),
            right,
        ),
        IntegerBinaryKind::WrappingShiftLeft => shape.scalar_type.wrapping_shift_left(
            left,
            shape.count_type.expect("shift count type"),
            right,
        ),
        IntegerBinaryKind::WrappingShiftRight => shape.scalar_type.wrapping_shift_right(
            left,
            shape.count_type.expect("shift count type"),
            right,
        ),
        IntegerBinaryKind::BitwiseAnd => shape.scalar_type.bitwise_and(left, right),
        IntegerBinaryKind::BitwiseOr => shape.scalar_type.bitwise_or(left, right),
        IntegerBinaryKind::BitwiseXor => shape.scalar_type.bitwise_xor(left, right),
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
