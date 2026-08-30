use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::{
    OptimizationFact, ProofQuestionOwner, PsiOptimizationFunction, PsiOptimizationUnit,
    ValueDefinition, ValueRangeFact, ValueRangeRegion, ValueRangeScope, ValueRangeSupport,
    value_range_fact_identity,
};
use omega_optimization_validation::validate_current_value_range_fact_at;
use psi_core::{
    BlockId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_terminal_semantics::CanonicalScalarGoal;

use crate::analyses::control_flow::{DominatorAnalysis, dominators};

use super::{
    shared::{scalar_operation_successors, scalar_value_definition},
    sparse_conditional_constants::{ScalarConstant, scalar_constants},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRangeAnalysis {
    pub facts: Vec<ValueRangeFact>,
}

impl ValueRangeAnalysis {
    pub fn fact_applies_at(
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

#[derive(Debug, Clone, Copy, Default)]
struct PartialIntegerBounds {
    lower: Option<IntegerValue>,
    upper: Option<IntegerValue>,
}

enum IntervalExtraction {
    Unsupported,
    Contradiction,
    Bounds(BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>),
}

pub(in crate::analyses) fn value_ranges(unit: &PsiOptimizationUnit) -> ValueRangeAnalysis {
    let mut facts = Vec::new();
    let dominator_facts = dominators(unit, false);
    for constant in scalar_constants(unit).facts {
        let ScalarConstant::Integer(value) = constant.constant else {
            continue;
        };
        let Some(identity) = constant.identity else {
            continue;
        };
        let Some(function) = unit
            .functions
            .iter()
            .find(|function| function.machine == constant.valid_in.machine)
        else {
            continue;
        };
        let Some(ValueDefinition {
            scalar_type: ScalarType::Integer(scalar_type),
            ..
        }) = scalar_value_definition(function, constant.value)
        else {
            continue;
        };
        let support = ValueRangeSupport::ScalarConstant(identity);
        let valid_in = ValueRangeRegion {
            revision: unit.identity,
            machine: function.machine,
            value: constant.value,
            scope: ValueRangeScope::EntireValue,
            dominated_blocks: Vec::new(),
        };
        facts.push(new_value_range_fact(
            constant.value,
            scalar_type,
            value,
            value,
            support,
            valid_in,
        ));
    }

    for function in &unit.functions {
        let reachable = reachable_blocks(function);
        for block in &function.blocks {
            if !reachable.contains(&block.id) {
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
                    psi_terminal_codec::canonical_proposition_order_key(&proposition)
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
                let IntervalExtraction::Bounds(bounds) = extract_integer_intervals(&proposition)
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
                    let support = ValueRangeSupport::AcceptedOperationProof {
                        accepted: accepted.identity,
                        question: question.identity,
                        operation,
                    };
                    let valid_in = ValueRangeRegion {
                        revision: unit.identity,
                        machine: function.machine,
                        value,
                        scope: ValueRangeScope::DominatedOperationEntry {
                            block: block.id,
                            node: node_index,
                            operation,
                        },
                        dominated_blocks: dominated_blocks(
                            &dominator_facts,
                            function.machine,
                            block.id,
                        ),
                    };
                    facts.push(new_value_range_fact(
                        value,
                        scalar_type,
                        minimum,
                        maximum,
                        support,
                        valid_in,
                    ));
                }
            }
        }
    }
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

fn dominated_blocks(
    dominators: &DominatorAnalysis,
    machine: MachineId,
    anchor: BlockId,
) -> Vec<BlockId> {
    let mut blocks = dominators
        .functions
        .iter()
        .find(|(candidate, _)| *candidate == machine)
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .filter_map(|(block, values)| values.contains(&anchor).then_some(*block))
        .collect::<Vec<_>>();
    blocks.sort_unstable();
    blocks.dedup();
    blocks
}

fn reachable_blocks(function: &PsiOptimizationFunction) -> BTreeSet<BlockId> {
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if !reachable.insert(block) {
            continue;
        }
        let Some(block) = blocks.get(&block) else {
            continue;
        };
        let Some(terminal) = block.nodes.last() else {
            continue;
        };
        pending.extend(
            scalar_operation_successors(&terminal.operation)
                .into_iter()
                .map(|edge| edge.target),
        );
    }
    reachable
}

fn proof_range_goal(
    operation: &O,
) -> Option<(OperationId, psi_core::ObligationId, CanonicalScalarGoal)> {
    let value_term = |id, scalar_type| ScalarTerm::value(id, ScalarType::Integer(scalar_type));
    match operation {
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            value_type,
            count_type,
            count,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::ExactShiftCount {
                value_type: *value_type,
                count_type: *count_type,
                count: value_term(*count, *count_type),
            },
        )),
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            scalar_type,
            left,
            right,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: *scalar_type,
                left: value_term(*left, *scalar_type),
                right: value_term(*right, *scalar_type),
            },
        )),
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        }
        | O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        }
        | O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        }
        | O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: *scalar_type,
                divisor: value_term(*right, *scalar_type),
            },
        )),
        _ => None,
    }
}

fn extract_integer_intervals(proposition: &Proposition) -> IntervalExtraction {
    if proposition.validate().is_err() {
        return IntervalExtraction::Unsupported;
    }
    let mut bounds = BTreeMap::new();
    if !extract_integer_interval_conjunct(proposition, &mut bounds) {
        return IntervalExtraction::Contradiction;
    }
    for ((_, scalar_type), interval) in &bounds {
        let minimum = interval
            .lower
            .unwrap_or_else(|| scalar_type.minimum_value());
        let maximum = interval
            .upper
            .unwrap_or_else(|| scalar_type.maximum_value());
        if integer_value_cmp(*scalar_type, minimum, maximum).is_none_or(|order| order.is_gt()) {
            return IntervalExtraction::Contradiction;
        }
    }
    if bounds.is_empty() {
        IntervalExtraction::Unsupported
    } else {
        IntervalExtraction::Bounds(bounds)
    }
}

fn extract_integer_interval_conjunct(
    proposition: &Proposition,
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
) -> bool {
    match proposition {
        Proposition::Truth => true,
        Proposition::Falsehood => false,
        Proposition::Conjunction(conjuncts) => conjuncts
            .iter()
            .all(|conjunct| extract_integer_interval_conjunct(conjunct, bounds)),
        Proposition::Equal(left, right) => {
            let Some((value, scalar_type, literal)) =
                value_and_literal(left, right).or_else(|| value_and_literal(right, left))
            else {
                return true;
            };
            merge_lower(bounds, value, scalar_type, literal)
                && merge_upper(bounds, value, scalar_type, literal)
        }
        Proposition::LessOrEqual(left, right) => {
            if let Some((value, scalar_type, literal)) = value_and_literal(left, right) {
                merge_upper(bounds, value, scalar_type, literal)
            } else if let Some((value, scalar_type, literal)) = value_and_literal(right, left) {
                merge_lower(bounds, value, scalar_type, literal)
            } else {
                true
            }
        }
        Proposition::LessThan(left, right) => {
            if let Some((value, scalar_type, literal)) = value_and_literal(left, right) {
                let Some(upper) = predecessor(scalar_type, literal) else {
                    return false;
                };
                merge_upper(bounds, value, scalar_type, upper)
            } else if let Some((value, scalar_type, literal)) = value_and_literal(right, left) {
                let Some(lower) = successor(scalar_type, literal) else {
                    return false;
                };
                merge_lower(bounds, value, scalar_type, lower)
            } else {
                true
            }
        }
        Proposition::Atom(_)
        | Proposition::IntegerMathEqual(_, _)
        | Proposition::IntegerMathLessThan(_, _)
        | Proposition::IntegerMathLessOrEqual(_, _)
        | Proposition::IeeeFloatComparison { .. }
        | Proposition::ByteSequenceEqual { .. }
        | Proposition::StructuralCaseMembership { .. }
        | Proposition::Disjunction(_)
        | Proposition::Implication { .. }
        | Proposition::ContentConservation(_) => true,
    }
}

fn value_and_literal(
    value: &ScalarTerm,
    literal: &ScalarTerm,
) -> Option<(ValueId, IntegerType, IntegerValue)> {
    let ScalarTerm::Value {
        id,
        scalar_type: ScalarType::Integer(value_type),
    } = value
    else {
        return None;
    };
    let ScalarTerm::Integer {
        scalar_type: literal_type,
        value,
    } = literal
    else {
        return None;
    };
    (*value_type == *literal_type
        && value_type.carrier() == IntegerCarrier::Fixed
        && value_type.admits(*value))
    .then_some((*id, *value_type, *value))
}

fn merge_lower(
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
    value: ValueId,
    scalar_type: IntegerType,
    lower: IntegerValue,
) -> bool {
    let interval = bounds.entry((value, scalar_type)).or_default();
    interval.lower = match interval.lower {
        None => Some(lower),
        Some(current) => match integer_value_cmp(scalar_type, current, lower) {
            Some(order) if order.is_lt() => Some(lower),
            Some(_) => Some(current),
            None => return false,
        },
    };
    true
}

fn merge_upper(
    bounds: &mut BTreeMap<(ValueId, IntegerType), PartialIntegerBounds>,
    value: ValueId,
    scalar_type: IntegerType,
    upper: IntegerValue,
) -> bool {
    let interval = bounds.entry((value, scalar_type)).or_default();
    interval.upper = match interval.upper {
        None => Some(upper),
        Some(current) => match integer_value_cmp(scalar_type, current, upper) {
            Some(order) if order.is_gt() => Some(upper),
            Some(_) => Some(current),
            None => return false,
        },
    };
    true
}

fn integer_value_cmp(
    scalar_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<std::cmp::Ordering> {
    if !scalar_type.admits(left) || !scalar_type.admits(right) {
        return None;
    }
    match (scalar_type.sign(), left, right) {
        (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
            Some(left.cmp(&right))
        }
        (IntegerSign::Unsigned, IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
            Some(left.cmp(&right))
        }
        _ => None,
    }
}

fn predecessor(scalar_type: IntegerType, value: IntegerValue) -> Option<IntegerValue> {
    let one = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    scalar_type.exact_sub(value, one)
}

fn successor(scalar_type: IntegerType, value: IntegerValue) -> Option<IntegerValue> {
    let one = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    };
    scalar_type.exact_add(value, one)
}

fn new_value_range_fact(
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

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::PropositionId;

    fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
        constructor(raw).expect("nonzero test identity")
    }

    fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
        IntegerType::new(sign, bits).expect("fixed test integer")
    }

    fn value(raw: u64, scalar_type: IntegerType) -> ScalarTerm {
        ScalarTerm::value(id(raw, ValueId::new), ScalarType::Integer(scalar_type))
    }

    fn literal(scalar_type: IntegerType, value: IntegerValue) -> ScalarTerm {
        ScalarTerm::integer(scalar_type, value).expect("admitted test literal")
    }

    #[test]
    fn interval_extraction_is_closed_conjunctive_and_nonconvex_safe() {
        let signed = integer(IntegerSign::Signed, 8);
        let count = id(1, ValueId::new);
        let proposition = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(signed, IntegerValue::Signed(0)), value(1, signed)),
            Proposition::LessOrEqual(value(1, signed), literal(signed, IntegerValue::Signed(63))),
        ]);
        let IntervalExtraction::Bounds(bounds) = extract_integer_intervals(&proposition) else {
            panic!("signed shift bounds produce one interval")
        };
        assert_eq!(
            bounds[&(count, signed)].lower,
            Some(IntegerValue::Signed(0))
        );
        assert_eq!(
            bounds[&(count, signed)].upper,
            Some(IntegerValue::Signed(63))
        );

        let unsigned = integer(IntegerSign::Unsigned, 8);
        let divisor = id(2, ValueId::new);
        let IntervalExtraction::Bounds(bounds) =
            extract_integer_intervals(&Proposition::LessOrEqual(
                literal(unsigned, IntegerValue::Unsigned(1)),
                value(2, unsigned),
            ))
        else {
            panic!("unsigned nonzero proof produces a lower bound")
        };
        assert_eq!(
            bounds[&(divisor, unsigned)].lower,
            Some(IntegerValue::Unsigned(1))
        );
        assert_eq!(bounds[&(divisor, unsigned)].upper, None);

        let signed_nonzero = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(value(1, signed), literal(signed, IntegerValue::Signed(-1))),
            Proposition::LessOrEqual(literal(signed, IntegerValue::Signed(1)), value(1, signed)),
        ]);
        assert!(matches!(
            extract_integer_intervals(&signed_nonzero),
            IntervalExtraction::Unsupported
        ));
    }

    #[test]
    fn interval_extraction_rejects_empty_strict_and_conflicting_domains() {
        let unsigned = integer(IntegerSign::Unsigned, 8);
        let minimum = literal(unsigned, unsigned.minimum_value());
        let maximum = literal(unsigned, unsigned.maximum_value());
        assert!(matches!(
            extract_integer_intervals(&Proposition::LessThan(value(1, unsigned), minimum)),
            IntervalExtraction::Contradiction
        ));
        assert!(matches!(
            extract_integer_intervals(&Proposition::LessThan(maximum, value(1, unsigned))),
            IntervalExtraction::Contradiction
        ));
        assert!(matches!(
            extract_integer_intervals(&Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    literal(unsigned, IntegerValue::Unsigned(1)),
                    value(1, unsigned),
                ),
                Proposition::LessOrEqual(
                    value(1, unsigned),
                    literal(unsigned, IntegerValue::Unsigned(0)),
                ),
            ])),
            IntervalExtraction::Contradiction
        ));
    }

    #[test]
    fn unsupported_conjunct_does_not_hide_an_independent_supported_bound() {
        let unsigned = integer(IntegerSign::Unsigned, 8);
        let value_id = id(1, ValueId::new);
        let proposition = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(
                literal(unsigned, IntegerValue::Unsigned(1)),
                value(1, unsigned),
            ),
            Proposition::Atom(id(9, PropositionId::new)),
        ]);
        let IntervalExtraction::Bounds(bounds) = extract_integer_intervals(&proposition) else {
            panic!("supported conjunct remains a valid consequence")
        };
        assert_eq!(
            bounds[&(value_id, unsigned)].lower,
            Some(IntegerValue::Unsigned(1))
        );
    }
}
