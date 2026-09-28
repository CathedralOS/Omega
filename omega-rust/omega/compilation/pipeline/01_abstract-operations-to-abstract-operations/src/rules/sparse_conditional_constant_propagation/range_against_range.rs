//! The three range-against-range comparisons -- equal, less-than and
//! less-or-equal between two closed integer ranges: one row each over the
//! shared traversal in `propose` and the closed `evaluate`.

use abstract_operations::AbstractOperation as O;
use optimization_core::{
    AnalysisKind, AnalysisSet, OptimizationRuleContract, OptimizationSafetyClass,
};
use optimization_unit::{
    BooleanConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
    ValueRangeSupport,
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
    let Some(AnalysisProduct::ValueRanges(ranges)) = analyses.get(AnalysisKind::ValueRanges) else {
        return Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::ValueRanges,
        ));
    };
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
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
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
                    _ => continue,
                };
                let Some(scalar_type) = integer_value_type(function, left) else {
                    continue;
                };
                if integer_value_type(function, right) != Some(scalar_type) {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node indices are u32");
                let proof_range = |value| {
                    ranges.facts.iter().find(|fact| {
                        fact.valid_in.machine == function.machine
                            && fact.value == value
                            && fact.scalar_type == scalar_type
                            && matches!(
                                fact.support,
                                ValueRangeSupport::AcceptedOperationProof { .. }
                            )
                            && ranges.fact_applies_at(
                                fact,
                                unit,
                                function.machine,
                                block.id,
                                node_index,
                            )
                    })
                };
                let (Some(left_range), Some(right_range)) = (proof_range(left), proof_range(right))
                else {
                    continue;
                };
                let Some(constant) = evaluate(
                    kind,
                    scalar_type,
                    left == right,
                    left_range.minimum,
                    left_range.maximum,
                    right_range.minimum,
                    right_range.maximum,
                ) else {
                    continue;
                };
                let location = NodeLocation {
                    machine: function.machine,
                    block: block.id,
                    node: node_index,
                };
                candidates.push(
                    PsiRewriteCandidate::new_boolean_evaluation(
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
                        IntegerEvaluationWitness::RangeAgainstRange {
                            left_range_fact: left_range.identity,
                            right_range_fact: right_range.identity,
                        },
                        -1,
                        BooleanConstantRewrite {
                            location,
                            source_operation: psi_operation,
                            result,
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
