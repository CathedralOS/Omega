//! The three range-against-range comparisons -- equal, less-than and
//! less-or-equal between two closed integer ranges: one row each over the
//! shared traversal in `propose` and the closed `evaluate`.

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisKind, AnalysisSet, OptimizationRuleContract, OptimizationSafetyClass,
};
use optimization_unit::{
    IntegerEvaluationWitness, PsiOptimizationUnit, PsiRewriteCandidate, ValueRangeSupport,
};
use semantic_vocabulary::{IntegerType, IntegerValue};

use super::IntegerRangePairComparisonKind;
use crate::rules::sparse_conditional_constant_propagation::{
    integer_value_type, range_comparison_contract,
};
use crate::{
    AnalysisProduct, PsiOptimizationRule, RuleAnalysisView, RuleProposalError, ValueRangeAnalysis,
};

fn contract(identity: &'static [u8], _safety: OptimizationSafetyClass) -> OptimizationRuleContract {
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
                    && matches!(
                        fact.support,
                        ValueRangeSupport::AcceptedOperationProof { .. }
                    )
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
pub(in crate::rules) fn evaluate(
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
            let both_equal_singletons = scalar_type.compare(left_minimum, left_maximum)?.is_eq()
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
