use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationRuleContract;
use optimization_unit::{
    BooleanConstantRewrite, IntegerEvaluationWitness, NodeLocation, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationUnit, PsiRealizationSite, PsiRewriteCandidate,
    ValueRangeSupport,
};

use crate::rules::passes::support::literal_integer_constant;
use crate::{RuleProposalError, ScalarConstantAnalysis, ValueRangeAnalysis};

use super::super::super::constant_evaluation::integer_value_type;
use super::{IntegerRangeComparisonKind, evaluation};

pub(super) fn propose(
    unit: &PsiOptimizationUnit,
    constants: &ScalarConstantAnalysis,
    ranges: &ValueRangeAnalysis,
    contract: OptimizationRuleContract,
    kind: IntegerRangeComparisonKind,
) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
    let mut candidates = Vec::new();
    for function in &unit.functions {
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
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
                    _ => continue,
                };
                let (range_value, constant_operand) = match kind {
                    IntegerRangeComparisonKind::RangeEqualConstant
                    | IntegerRangeComparisonKind::RangeLessThanConstant
                    | IntegerRangeComparisonKind::RangeLessOrEqualConstant => (left, right),
                    IntegerRangeComparisonKind::ConstantEqualRange
                    | IntegerRangeComparisonKind::ConstantLessThanRange
                    | IntegerRangeComparisonKind::ConstantLessOrEqualRange => (right, left),
                };
                let Some((constant_value, constant_fact)) =
                    literal_integer_constant(constants, function.machine, constant_operand)
                else {
                    continue;
                };
                let Some(scalar_type) = integer_value_type(function, range_value) else {
                    continue;
                };
                if integer_value_type(function, constant_operand) != Some(scalar_type) {
                    continue;
                }
                let node_index =
                    u32::try_from(node_index).expect("optimization node indices are u32");
                let Some(range) = ranges.facts.iter().find(|fact| {
                    fact.valid_in.machine == function.machine
                        && fact.value == range_value
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
                }) else {
                    continue;
                };
                let Some(constant) = evaluation::evaluate(
                    kind,
                    scalar_type,
                    range.minimum,
                    range.maximum,
                    constant_value,
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
                        IntegerEvaluationWitness::RangeAgainstConstant {
                            range_fact: range.identity,
                            constant_fact,
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
