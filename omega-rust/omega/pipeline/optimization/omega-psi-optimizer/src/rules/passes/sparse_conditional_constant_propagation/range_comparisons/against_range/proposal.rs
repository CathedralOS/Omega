use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::OptimizationRuleContract;
use omega_optimization_unit::{
    BooleanConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
    ValueRangeSupport,
};

use crate::{RuleProposalError, ValueRangeAnalysis};

use super::super::super::constant_evaluation::integer_value_type;
use super::{IntegerRangePairComparisonKind, evaluation};

pub(super) fn propose(
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
                let Some(constant) = evaluation::evaluate(
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
