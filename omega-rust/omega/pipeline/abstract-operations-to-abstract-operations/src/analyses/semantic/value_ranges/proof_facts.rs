//! Dominated ranges reconstructed from exact accepted operation proofs.

use optimization_unit::{
    OptimizationFact, ProofQuestionOwner, PsiOptimizationUnit, ValueRangeFact, ValueRangeRegion,
    ValueRangeScope, ValueRangeSupport,
};
use semantic_vocabulary::ScalarType;

use crate::analyses::control_flow::DominatorAnalysis;

use super::super::shared::scalar_value_definition;
use super::{
    control_flow::{dominated_blocks, reachable_blocks},
    intervals::IntervalExtraction,
};

pub(super) fn extend(
    unit: &PsiOptimizationUnit,
    dominators: &DominatorAnalysis,
    facts: &mut Vec<ValueRangeFact>,
) {
    for function in &unit.functions {
        let reachable = reachable_blocks(function);
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
                continue;
            }
            for (node_index, node) in block.nodes.iter().enumerate() {
                let Some((operation, obligation, goal)) =
                    super::proof_goals::for_operation(&node.operation)
                else {
                    continue;
                };
                if function
                    .blocks
                    .iter()
                    .flat_map(|candidate| &candidate.nodes)
                    .filter(|candidate| {
                        super::proof_goals::for_operation(&candidate.operation)
                            .is_some_and(|(candidate, _, _)| candidate == operation)
                    })
                    .count()
                    != 1
                    || function
                        .facts
                        .iter()
                        .filter(|fact| {
                            matches!(
                                fact,
                                OptimizationFact::OperationObligationReference {
                                    obligation: candidate_obligation,
                                    support,
                                } if *candidate_obligation == obligation && *support == operation
                            )
                        })
                        .count()
                        != 1
                {
                    continue;
                }
                let Ok(proposition) = goal.kernel_proposition() else {
                    continue;
                };
                let Ok(canonical) = terminal_codec::canonical_proposition_order_key(&proposition)
                else {
                    continue;
                };
                let mut accepted_matches = unit.accepted_obligation_facts.iter().filter(|fact| {
                    fact.machine == function.machine
                        && fact.operation == operation
                        && fact.obligation == obligation
                        && fact.proposition == canonical
                        && fact.psi == unit.psi
                        && fact.has_canonical_identity()
                });
                let Some(accepted) = accepted_matches.next() else {
                    continue;
                };
                if accepted_matches.next().is_some() {
                    continue;
                }
                let mut question_matches = unit.proof_questions.iter().filter(|question| {
                    question.owner
                        == (ProofQuestionOwner::Operation {
                            machine: function.machine,
                            operation,
                        })
                        && question.obligation == obligation
                        && question.proposition == canonical
                        && question.canonical_certificate
                        && question.terminal_psi == unit.psi
                        && question.proof_bundle_fingerprint == accepted.proof_bundle_fingerprint
                        && question.has_canonical_identity()
                });
                let Some(question) = question_matches.next() else {
                    continue;
                };
                if question_matches.next().is_some() {
                    continue;
                }
                let IntervalExtraction::Bounds(bounds) = super::intervals::extract(&proposition)
                else {
                    continue;
                };
                let node_index =
                    u32::try_from(node_index).expect("optimization node position fits u32");
                for ((value, scalar_type), bounds) in bounds {
                    if scalar_value_definition(function, value).is_none_or(|definition| {
                        definition.scalar_type != ScalarType::Integer(scalar_type)
                    }) {
                        continue;
                    }
                    let minimum = bounds.lower.unwrap_or_else(|| scalar_type.minimum_value());
                    let maximum = bounds.upper.unwrap_or_else(|| scalar_type.maximum_value());
                    if minimum == scalar_type.minimum_value()
                        && maximum == scalar_type.maximum_value()
                    {
                        continue;
                    }
                    facts.push(super::facts::new(
                        value,
                        scalar_type,
                        minimum,
                        maximum,
                        ValueRangeSupport::AcceptedOperationProof {
                            accepted: accepted.identity,
                            question: question.identity,
                            operation,
                        },
                        ValueRangeRegion {
                            revision: unit.identity,
                            machine: function.machine,
                            value,
                            scope: ValueRangeScope::DominatedOperationEntry {
                                block: block.id,
                                node: node_index,
                                operation,
                            },
                            dominated_blocks: dominated_blocks(
                                dominators,
                                function.machine,
                                block.id,
                            ),
                        },
                    ));
                }
            }
        }
    }
}
