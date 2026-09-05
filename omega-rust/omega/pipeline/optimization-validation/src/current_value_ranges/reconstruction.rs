//! Independent constant/proof range reconstruction and canonical fact identity.

use super::availability::{scope_applies_at, value_available_at};
use super::intervals::{IntervalExtraction, extract_integer_intervals};
use super::proof_goals::proof_range_goal;
use super::*;

pub(super) fn reconstruct_value_range_fact(
    unit: &PsiOptimizationUnit,
    supplied: &ValueRangeFact,
) -> Option<ValueRangeFact> {
    if supplied.valid_in.revision != unit.identity || supplied.valid_in.value != supplied.value {
        return None;
    }
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == supplied.valid_in.machine)?;
    match supplied.support {
        ValueRangeSupport::ScalarConstant(identity) => {
            if supplied.valid_in.scope != ValueRangeScope::EntireValue
                || !supplied.valid_in.dominated_blocks.is_empty()
            {
                return None;
            }
            let definition = scalar_value_definition(function, supplied.value)?;
            let ScalarType::Integer(scalar_type) = definition.scalar_type else {
                return None;
            };
            let (_, constant, _) = validator_scalar_constant_facts(unit.identity, function)
                .into_iter()
                .find(|(value, constant, candidate)| {
                    *value == supplied.value
                        && *candidate == identity
                        && matches!(constant, ScalarConstantValue::Integer(_))
                })?;
            let ScalarConstantValue::Integer(value) = constant else {
                return None;
            };
            new_fact(
                supplied.value,
                scalar_type,
                value,
                value,
                ValueRangeSupport::ScalarConstant(identity),
                supplied.valid_in.clone(),
            )
        }
        ValueRangeSupport::AcceptedOperationProof {
            accepted,
            question,
            operation,
        } => {
            let ValueRangeScope::DominatedOperationEntry {
                block,
                node,
                operation: scope_operation,
            } = supplied.valid_in.scope
            else {
                return None;
            };
            if operation != scope_operation {
                return None;
            }
            let anchor = function
                .blocks
                .iter()
                .find(|candidate| candidate.id == block)?
                .nodes
                .get(usize::try_from(node).ok()?)?;
            let (current_operation, obligation, goal) = proof_range_goal(&anchor.operation)?;
            if current_operation != operation
                || function
                    .blocks
                    .iter()
                    .flat_map(|candidate| &candidate.nodes)
                    .filter(|candidate| {
                        proof_range_goal(&candidate.operation)
                            .is_some_and(|(candidate, _, _)| candidate == operation)
                    })
                    .count()
                    != 1
                || function
                    .facts
                    .iter()
                    .filter(|candidate| {
                        matches!(candidate,
                            OptimizationFact::OperationObligationReference {
                                obligation: candidate_obligation,
                                support,
                            } if *candidate_obligation == obligation && *support == operation)
                    })
                    .count()
                    != 1
            {
                return None;
            }
            let proposition = goal.kernel_proposition().ok()?;
            let canonical = terminal_codec::canonical_proposition_order_key(&proposition).ok()?;
            let accepted_fact = unit
                .accepted_obligation_facts
                .iter()
                .filter(|candidate| {
                    candidate.identity == accepted
                        && candidate.machine == function.machine
                        && candidate.operation == operation
                        && candidate.obligation == obligation
                        && candidate.proposition == canonical
                        && candidate.psi == unit.psi
                        && candidate.has_canonical_identity()
                })
                .exactly_one()?;
            let proof_question = unit
                .proof_questions
                .iter()
                .filter(|candidate| {
                    candidate.identity == question
                        && candidate.owner
                            == ProofQuestionOwner::Operation {
                                machine: function.machine,
                                operation,
                            }
                        && candidate.obligation == obligation
                        && candidate.proposition == canonical
                        && candidate.canonical_certificate
                        && candidate.terminal_psi == unit.psi
                        && candidate.proof_bundle_fingerprint
                            == accepted_fact.proof_bundle_fingerprint
                        && candidate.has_canonical_identity()
                })
                .exactly_one()?;
            let _ = proof_question;
            let IntervalExtraction::Bounds(bounds) = extract_integer_intervals(&proposition) else {
                return None;
            };
            let definition = scalar_value_definition(function, supplied.value)?;
            if definition.scalar_type != ScalarType::Integer(supplied.scalar_type) {
                return None;
            }
            let partial = bounds.get(&(supplied.value, supplied.scalar_type))?;
            let minimum = partial
                .lower
                .unwrap_or_else(|| supplied.scalar_type.minimum_value());
            let maximum = partial
                .upper
                .unwrap_or_else(|| supplied.scalar_type.maximum_value());
            if minimum == supplied.scalar_type.minimum_value()
                && maximum == supplied.scalar_type.maximum_value()
            {
                return None;
            }
            let dominators = independent_reachable_dominators(function);
            let dominated_blocks = dominators
                .iter()
                .filter_map(|(candidate, values)| values.contains(&block).then_some(*candidate))
                .collect::<Vec<_>>();
            let valid_in = ValueRangeRegion {
                revision: unit.identity,
                machine: function.machine,
                value: supplied.value,
                scope: ValueRangeScope::DominatedOperationEntry {
                    block,
                    node,
                    operation,
                },
                dominated_blocks,
            };
            new_fact(
                supplied.value,
                supplied.scalar_type,
                minimum,
                maximum,
                ValueRangeSupport::AcceptedOperationProof {
                    accepted,
                    question,
                    operation,
                },
                valid_in,
            )
        }
    }
}

/// Independently find one proof-derived range by identity and prove that it is
/// usable at the requested current operation entry. Candidate validation uses
/// this instead of importing the optimizer's analysis implementation.
pub(crate) fn independently_reconstruct_value_range_fact_at(
    unit: &PsiOptimizationUnit,
    identity: ValueRangeFactIdentity,
    machine: MachineId,
    value: ValueId,
    query_block: BlockId,
    query_node: u32,
) -> Option<ValueRangeFact> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)?;
    let dominators = independent_reachable_dominators(function);
    for block in &function.blocks {
        if !dominators.contains_key(&block.id) {
            continue;
        }
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some((operation, obligation, goal)) = proof_range_goal(&node.operation) else {
                continue;
            };
            if function
                .blocks
                .iter()
                .flat_map(|candidate| &candidate.nodes)
                .filter(|candidate| {
                    proof_range_goal(&candidate.operation)
                        .is_some_and(|(candidate, _, _)| candidate == operation)
                })
                .count()
                != 1
                || function
                    .facts
                    .iter()
                    .filter(|candidate| {
                        matches!(candidate,
                            OptimizationFact::OperationObligationReference {
                                obligation: candidate_obligation,
                                support,
                            } if *candidate_obligation == obligation && *support == operation)
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
            let Some(accepted) = unit
                .accepted_obligation_facts
                .iter()
                .filter(|candidate| {
                    candidate.machine == machine
                        && candidate.operation == operation
                        && candidate.obligation == obligation
                        && candidate.proposition == canonical
                        && candidate.psi == unit.psi
                        && candidate.has_canonical_identity()
                })
                .exactly_one()
            else {
                continue;
            };
            let Some(question) = unit
                .proof_questions
                .iter()
                .filter(|candidate| {
                    candidate.owner == ProofQuestionOwner::Operation { machine, operation }
                        && candidate.obligation == obligation
                        && candidate.proposition == canonical
                        && candidate.canonical_certificate
                        && candidate.terminal_psi == unit.psi
                        && candidate.proof_bundle_fingerprint == accepted.proof_bundle_fingerprint
                        && candidate.has_canonical_identity()
                })
                .exactly_one()
            else {
                continue;
            };
            let IntervalExtraction::Bounds(bounds) = extract_integer_intervals(&proposition) else {
                continue;
            };
            let Some((scalar_type, partial)) =
                bounds
                    .iter()
                    .find_map(|((candidate, scalar_type), partial)| {
                        (*candidate == value).then_some((*scalar_type, *partial))
                    })
            else {
                continue;
            };
            if scalar_value_definition(function, value)
                .is_none_or(|definition| definition.scalar_type != ScalarType::Integer(scalar_type))
            {
                continue;
            }
            let minimum = partial.lower.unwrap_or_else(|| scalar_type.minimum_value());
            let maximum = partial.upper.unwrap_or_else(|| scalar_type.maximum_value());
            if minimum == scalar_type.minimum_value() && maximum == scalar_type.maximum_value() {
                continue;
            }
            let node_index =
                u32::try_from(node_index).expect("optimization-unit node position fits u32");
            let valid_in = ValueRangeRegion {
                revision: unit.identity,
                machine,
                value,
                scope: ValueRangeScope::DominatedOperationEntry {
                    block: block.id,
                    node: node_index,
                    operation,
                },
                dominated_blocks: dominators
                    .iter()
                    .filter_map(|(candidate, values)| {
                        values.contains(&block.id).then_some(*candidate)
                    })
                    .collect(),
            };
            let Some(fact) = new_fact(
                value,
                scalar_type,
                minimum,
                maximum,
                ValueRangeSupport::AcceptedOperationProof {
                    accepted: accepted.identity,
                    question: question.identity,
                    operation,
                },
                valid_in,
            ) else {
                continue;
            };
            if fact.identity == identity
                && value_available_at(function, value, query_block, query_node)
                && scope_applies_at(
                    fact.valid_in.scope,
                    &fact.valid_in.dominated_blocks,
                    query_block,
                    query_node,
                )
            {
                return Some(fact);
            }
        }
    }
    None
}

fn new_fact(
    value: ValueId,
    scalar_type: IntegerType,
    minimum: IntegerValue,
    maximum: IntegerValue,
    support: ValueRangeSupport,
    valid_in: ValueRangeRegion,
) -> Option<ValueRangeFact> {
    Some(ValueRangeFact {
        identity: value_range_fact_identity(
            value,
            scalar_type,
            minimum,
            maximum,
            &support,
            &valid_in,
        )?,
        value,
        scalar_type,
        minimum,
        maximum,
        support,
        valid_in,
    })
}

trait ExactlyOne: Iterator + Sized {
    fn exactly_one(mut self) -> Option<Self::Item> {
        let first = self.next()?;
        self.next().is_none().then_some(first)
    }
}

impl<I: Iterator> ExactlyOne for I {}
