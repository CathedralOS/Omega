//! Typed case truths retain all completed-value alternatives.
use super::transport::ConditionalPredicate;
use super::{BoundPredicate, CapturedBinding, CheckFacts, FlowStateFact};
use crate::flow::{SelectionCondition, SelectionConditions};

/// A tag truth belongs to one completed observation and its alternatives.
/// Unknown or disagreeing suppliers subtract support rather than disappearing.
pub(super) fn predicates(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    required: &BoundPredicate,
    captures: &[CapturedBinding<'_>],
    selections: &[SelectionCondition],
    conditions: &mut SelectionConditions,
) -> Vec<ConditionalPredicate> {
    let mut result = Vec::new();
    for (position, binding) in required.bindings.iter().enumerate() {
        let facts::BooleanPredicateOperand::CaseSubject(subject) = &binding.operand else {
            continue;
        };
        let Some(data) = program.closed_data_definition(binding.type_reference) else {
            continue;
        };
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                continue;
            };
            let case = variant.member_identity();
            let mut yes = conditions.never();
            let mut no = conditions.never();
            let mut unknown = conditions.never();
            for (capture, &selection) in captures
                .iter()
                .zip(selections)
                .filter(|(capture, _)| capture.position == position)
            {
                let mut truth = None;
                let mut conflict = false;
                for alias in &capture.aliases {
                    let contexts = facts
                        .flow
                        .semantic_constraint_contexts(alias.epoch.constraints)
                        .collect::<Vec<_>>();
                    let value = crate::flow::CaseEvidence::prepare(
                        program,
                        &facts.semantic,
                        &contexts,
                        state.machine_symbol,
                        alias.epoch.state_symbol,
                        alias.epoch.statement_index,
                    )
                    .and_then(|evidence| {
                        evidence.case_truth(&alias.place, binding.type_reference, &case)
                    });
                    if let Some(value) = value {
                        conflict |= truth.is_some_and(|known| known != value);
                        truth = Some(value);
                    }
                }
                match (conflict, truth) {
                    (false, Some(true)) => yes = conditions.any(yes, selection),
                    (false, Some(false)) => no = conditions.any(no, selection),
                    _ => unknown = conditions.any(unknown, selection),
                }
            }
            for (positive, good, bad) in [(true, yes, no), (false, no, yes)] {
                let bad = conditions.any(bad, unknown);
                let not_bad = conditions.negate(&facts.flow.control, bad);
                let condition = conditions.all(good, not_bad);
                if !conditions.possible(&facts.flow.control, condition) {
                    continue;
                }
                let atom = checked_trees::CheckedBooleanExpression::StructuralCaseMembership {
                    subject: subject.clone(),
                    case: case.clone(),
                };
                result.push(ConditionalPredicate {
                    condition,
                    predicate: BoundPredicate {
                        machine: required.machine,
                        entry_state: required.entry_state,
                        expression: if positive {
                            atom
                        } else {
                            checked_trees::CheckedBooleanExpression::Not(Box::new(atom))
                        },
                        bindings: vec![binding.clone()],
                    },
                });
            }
        }
    }
    result
}
