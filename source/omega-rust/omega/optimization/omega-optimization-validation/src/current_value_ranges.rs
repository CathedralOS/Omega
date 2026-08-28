use std::collections::BTreeMap;

use omega_optimization_core::ValueRangeFactIdentity;
use omega_optimization_unit::{
    OptimizationFact, ProofQuestionOwner, PsiOptimizationFunction, PsiOptimizationUnit,
    ScalarConstantValue, ValueDefinitionSite, ValueRangeFact, ValueRangeRegion, ValueRangeScope,
    ValueRangeSupport, value_range_fact_identity,
};
use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
use psi_core::{
    BlockId, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    Proposition, ScalarTerm, ScalarType, ValueId,
};
use psi_terminal_semantics::CanonicalScalarGoal;

use crate::{
    OptimizationUnitValidationError, independent_reachable_dominators, scalar_value_definition,
    validate_psi_optimization_unit, validator_scalar_constant_facts,
};

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

/// Independently reconstruct one optimizer-produced current-revision range.
///
/// This path does not call the optimizer analysis. It re-derives scalar facts,
/// verifier proposition custody, interval bounds, current CFG dominance, and
/// the final fact identity from the optimization unit.
pub fn validate_current_value_range_fact(
    unit: &PsiOptimizationUnit,
    fact: &ValueRangeFact,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(unit)?;
    let expected = reconstruct_value_range_fact(unit, fact)
        .ok_or(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)?;
    if expected != *fact {
        return Err(OptimizationUnitValidationError::CurrentValueRangeFactMismatch);
    }
    Ok(())
}

/// Validate a range and prove that its authority reaches one current operation
/// entry. Node results become available after their defining node, while
/// block/function parameters are available from their respective entries.
pub fn validate_current_value_range_fact_at(
    unit: &PsiOptimizationUnit,
    fact: &ValueRangeFact,
    machine: MachineId,
    block: BlockId,
    node: u32,
) -> Result<(), OptimizationUnitValidationError> {
    validate_current_value_range_fact(unit, fact)?;
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(
            OptimizationUnitValidationError::CurrentValueRangeFactNotApplicable {
                machine,
                block,
                node,
            },
        )?;
    let query_block = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .ok_or(
            OptimizationUnitValidationError::CurrentValueRangeFactNotApplicable {
                machine,
                block,
                node,
            },
        )?;
    if usize::try_from(node)
        .ok()
        .is_none_or(|node| node >= query_block.nodes.len())
        || fact.valid_in.machine != machine
        || !value_available_at(function, fact.value, block, node)
        || !scope_applies_at(
            fact.valid_in.scope,
            &fact.valid_in.dominated_blocks,
            block,
            node,
        )
    {
        return Err(
            OptimizationUnitValidationError::CurrentValueRangeFactNotApplicable {
                machine,
                block,
                node,
            },
        );
    }
    Ok(())
}

fn reconstruct_value_range_fact(
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
            let proposition = goal.kernel_proposition().ok()??;
            let canonical =
                psi_terminal_codec::canonical_proposition_order_key(&proposition).ok()?;
            let accepted_fact = unit
                .accepted_obligation_facts
                .iter()
                .filter(|candidate| {
                    candidate.identity == accepted
                        && candidate.machine == function.machine
                        && candidate.operation == operation
                        && candidate.obligation == obligation
                        && candidate.proposition == canonical
                        && candidate.terminal_psi == unit.terminal_psi
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
                        && candidate.terminal_psi == unit.terminal_psi
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
pub(super) fn independently_reconstruct_value_range_fact_at(
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
            let Ok(Some(proposition)) = goal.kernel_proposition() else {
                continue;
            };
            let Ok(canonical) = psi_terminal_codec::canonical_proposition_order_key(&proposition)
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
                        && candidate.terminal_psi == unit.terminal_psi
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
                        && candidate.terminal_psi == unit.terminal_psi
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

fn scope_applies_at(
    scope: ValueRangeScope,
    dominated_blocks: &[BlockId],
    block: BlockId,
    node: u32,
) -> bool {
    match scope {
        ValueRangeScope::EntireValue => true,
        ValueRangeScope::DominatedOperationEntry {
            block: anchor,
            node: anchor_node,
            ..
        } => {
            if block == anchor {
                node >= anchor_node
            } else {
                dominated_blocks.binary_search(&block).is_ok()
            }
        }
    }
}

fn value_available_at(
    function: &PsiOptimizationFunction,
    value: ValueId,
    block: BlockId,
    node: u32,
) -> bool {
    let Some(definition) = scalar_value_definition(function, value) else {
        return false;
    };
    match definition.site {
        ValueDefinitionSite::FunctionParameter(_) => true,
        ValueDefinitionSite::BlockParameter {
            block: definition_block,
            ..
        } => {
            definition_block == block
                || independent_reachable_dominators(function)
                    .get(&block)
                    .is_some_and(|dominators| dominators.contains(&definition_block))
        }
        ValueDefinitionSite::Node {
            block: definition_block,
            node: definition_node,
        } => {
            if definition_block == block {
                definition_node < node
            } else {
                independent_reachable_dominators(function)
                    .get(&block)
                    .is_some_and(|dominators| dominators.contains(&definition_block))
            }
        }
    }
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

trait ExactlyOne: Iterator + Sized {
    fn exactly_one(mut self) -> Option<Self::Item> {
        let first = self.next()?;
        self.next().is_none().then_some(first)
    }
}

impl<I: Iterator> ExactlyOne for I {}
