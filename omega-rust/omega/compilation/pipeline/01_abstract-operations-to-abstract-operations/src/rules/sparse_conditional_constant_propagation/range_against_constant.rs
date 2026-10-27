//! The six range-against-constant comparisons -- equal, less-than and
//! less-or-equal with the range on either side -- decided from a closed
//! integer range and a scalar constant: one row each over the shared
//! traversal in `propose` and the closed `evaluate`.

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisKind, AnalysisSet, OptimizationRuleContract, OptimizationSafetyClass,
};
use optimization_unit::{
    IntegerEvaluationWitness, PsiOptimizationUnit, PsiRewriteCandidate, ValueRangeSupport,
};
use semantic_vocabulary::{IntegerType, IntegerValue};

use super::IntegerRangeComparisonKind;
use crate::rules::sparse_conditional_constant_propagation::{
    integer_value_type, range_comparison_contract,
};
use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError,
    ScalarConstantAnalysis, ValueRangeAnalysis,
};

fn contract(identity: &'static [u8], _safety: OptimizationSafetyClass) -> OptimizationRuleContract {
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
                && matches!(
                    fact.support,
                    ValueRangeSupport::AcceptedOperationProof { .. }
                )
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

pub(in crate::rules) fn evaluate(
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
        .or_else(|| (minimum_to_constant.is_gt() || maximum_to_constant.is_lt()).then_some(false)),
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
