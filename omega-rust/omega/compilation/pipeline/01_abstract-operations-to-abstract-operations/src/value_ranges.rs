/*
Canonical constant and accepted-proof value-range analysis
(`ValueRangeAnalysis`, `value_ranges`). Joins entire-value ranges derived from
scalar constants with dominated control-flow ranges reconstructed from accepted
operation proofs, sorted canonically and queried through
`validate_current_value_range_fact_at`.
*/

pub(crate) mod constant_facts {
    use optimization_unit::{
        PsiOptimizationUnit, ScalarConstantValue, ValueDefinition, ValueRangeFact,
        ValueRangeRegion, ValueRangeScope, ValueRangeSupport,
    };
    use semantic_vocabulary::ScalarType;

    use crate::semantic_analyses::{
        scalar_value_definition, sparse_conditional_constants::scalar_constants,
    };

    pub(crate) fn collect(unit: &PsiOptimizationUnit) -> Vec<ValueRangeFact> {
        let mut facts = Vec::new();
        for constant in scalar_constants(unit).facts {
            let ScalarConstantValue::Integer(value) = constant.constant else {
                continue;
            };
            let Some(identity) = constant.identity else {
                continue;
            };
            let Some(function) = unit.function(constant.valid_in.machine) else {
                continue;
            };
            let Some(ValueDefinition {
                scalar_type: ScalarType::Integer(scalar_type),
                ..
            }) = scalar_value_definition(function, constant.value)
            else {
                continue;
            };
            facts.push(super::facts::new(
                constant.value,
                scalar_type,
                value,
                value,
                ValueRangeSupport::ScalarConstant(identity),
                ValueRangeRegion {
                    revision: unit.identity,
                    machine: function.machine,
                    value: constant.value,
                    scope: ValueRangeScope::EntireValue,
                    dominated_blocks: Vec::new(),
                },
            ));
        }
        facts
    }
}
pub(crate) mod control_flow {
    use semantic_vocabulary::{BlockId, MachineId};

    use crate::analyses::control_flow::DominatorAnalysis;

    pub(crate) fn dominated_blocks(
        dominators: &DominatorAnalysis,
        machine: MachineId,
        anchor: BlockId,
    ) -> Vec<BlockId> {
        let mut blocks = dominators
            .function(machine)
            .into_iter()
            .flat_map(|(_, rows)| rows)
            .filter_map(|(block, values)| values.contains(&anchor).then_some(*block))
            .collect::<Vec<_>>();
        blocks.sort_unstable();
        blocks.dedup();
        blocks
    }
}
pub(crate) mod facts {
    use optimization_unit::{
        ValueRangeFact, ValueRangeRegion, ValueRangeSupport, value_range_fact_identity,
    };
    use semantic_vocabulary::{IntegerType, IntegerValue, ValueId};

    pub(crate) fn new(
        value: ValueId,
        scalar_type: IntegerType,
        minimum: IntegerValue,
        maximum: IntegerValue,
        support: ValueRangeSupport,
        valid_in: ValueRangeRegion,
    ) -> ValueRangeFact {
        let identity =
            value_range_fact_identity(value, scalar_type, minimum, maximum, &support, &valid_in)
                .expect("internally derived value range has a canonical identity");
        ValueRangeFact {
            identity,
            value,
            scalar_type,
            minimum,
            maximum,
            support,
            valid_in,
        }
    }
}
pub(crate) mod proof_facts {
    use optimization_unit::{
        IntervalExtraction, OptimizationFact, ProofQuestionOwner, PsiOptimizationUnit,
        ValueRangeFact, ValueRangeRegion, ValueRangeScope, ValueRangeSupport,
    };
    use semantic_vocabulary::ScalarType;

    use crate::analyses::control_flow::DominatorAnalysis;

    use super::control_flow::dominated_blocks;
    use crate::semantic_analyses::{scalar_value_definition, semantically_reachable_blocks};

    pub(crate) fn extend(
        unit: &PsiOptimizationUnit,
        dominators: &DominatorAnalysis,
        facts: &mut Vec<ValueRangeFact>,
    ) {
        for function in &unit.functions {
            let reachable = semantically_reachable_blocks(function);
            for block in &function.blocks {
                if !reachable.contains(&block.id) {
                    continue;
                }
                for (node_index, node) in block.nodes.iter().enumerate() {
                    let Some((operation, obligation, goal)) =
                        optimization_unit::proof_range_goal(&node.operation)
                    else {
                        continue;
                    };
                    if function
                        .blocks
                        .iter()
                        .flat_map(|candidate| &candidate.nodes)
                        .filter(|candidate| {
                            optimization_unit::proof_range_goal(&candidate.operation)
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
                    let Ok(canonical) =
                        terminal_codec::canonical_proposition_order_key(&proposition)
                    else {
                        continue;
                    };
                    let Some(accepted) = unit
                        .accepted_obligation_fact(
                            function.machine,
                            optimization_unit::PsiProvenance::Operation(operation),
                            Some(obligation),
                        )
                        .filter(|fact| fact.proposition == canonical)
                    else {
                        continue;
                    };
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
                            && question.proof_bundle_fingerprint
                                == accepted.proof_bundle_fingerprint
                            && question.has_canonical_identity()
                    });
                    let Some(question) = question_matches.next() else {
                        continue;
                    };
                    if question_matches.next().is_some() {
                        continue;
                    }
                    let IntervalExtraction::Bounds(bounds) =
                        optimization_unit::extract_integer_intervals(&proposition)
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
}

use optimization_unit::{PsiOptimizationUnit, ValueRangeFact};
use optimization_unit_semantics::validate_current_value_range_fact_at;
use semantic_vocabulary::{BlockId, MachineId};

use crate::analyses::control_flow::dominators;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValueRangeAnalysis {
    pub facts: Vec<ValueRangeFact>,
}

impl ValueRangeAnalysis {
    pub(crate) fn fact_applies_at(
        &self,
        fact: &ValueRangeFact,
        unit: &PsiOptimizationUnit,
        machine: MachineId,
        block: BlockId,
        node: u32,
    ) -> bool {
        self.facts.iter().any(|candidate| candidate == fact)
            && validate_current_value_range_fact_at(unit, fact, machine, block, node).is_ok()
    }
}

pub(crate) fn value_ranges(unit: &PsiOptimizationUnit) -> ValueRangeAnalysis {
    let mut facts = constant_facts::collect(unit);
    proof_facts::extend(unit, &dominators(unit, false), &mut facts);
    facts.sort_by_key(|fact| {
        (
            fact.valid_in.machine,
            fact.value,
            fact.valid_in.scope,
            fact.identity,
        )
    });
    ValueRangeAnalysis { facts }
}
