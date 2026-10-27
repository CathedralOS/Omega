//! Rebind one producer's predicates at its own read epoch. Every dependency
//! needs an alias valid for all destination candidates under that producer's
//! condition. Conditions combine; source premise contexts never do.

use super::bindings::Alias;
use super::{BoundObservation, BoundPredicate, CapturedBinding, CheckFacts, FlowStateFact};
use crate::flow::{SelectionCondition, SelectionConditions};

pub(super) struct ConditionalPredicate {
    pub predicate: BoundPredicate,
    pub condition: SelectionCondition,
}

pub(super) fn predicates(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    required: &BoundPredicate,
    captures: &[CapturedBinding<'_>],
    selections: &[SelectionCondition],
    conditions: &mut SelectionConditions,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<ConditionalPredicate> {
    let mut transported: Vec<ConditionalPredicate> = Vec::new();
    for (capture, &selection) in captures.iter().zip(selections) {
        for alias in &capture.aliases {
            let contexts = facts
                .flow
                .semantic_constraint_contexts(alias.epoch.constraints)
                .collect::<Vec<_>>();
            let mut predicates = crate::facts::boolean_predicates::retained(
                program,
                &facts.operators,
                &facts.semantic,
                &contexts,
                facts::ProgramPoint::Statement {
                    machine_symbol: state.machine_symbol,
                    state_symbol: alias.epoch.state_symbol,
                    statement_index: alias.epoch.statement_index,
                },
            );
            if let (Some(frames), Some(origin)) = (
                frames,
                facts
                    .flow
                    .control
                    .state(state.machine_symbol, alias.epoch.state_symbol),
            ) {
                predicates.extend(super::super::prover::call_guarantees::current_predicates(
                    program,
                    facts,
                    origin,
                    super::super::prover::call_guarantees::GuaranteeUse::StatementEntry(
                        alias.epoch.statement_index + 1,
                    ),
                    &contexts,
                    frames,
                ));
            }
            while let Some(mut predicate) = predicates.pop() {
                if let checked_trees::CheckedBooleanExpression::And { left, right } =
                    &predicate.expression
                {
                    predicates.push(BoundPredicate {
                        expression: *left.clone(),
                        ..predicate.clone()
                    });
                    predicates.push(BoundPredicate {
                        expression: *right.clone(),
                        ..predicate
                    });
                    continue;
                }
                let source_bindings = std::mem::take(&mut predicate.bindings);
                let complete = crate::facts::boolean_predicates::map_observations(
                    &mut predicate.expression,
                    &mut |operand| {
                        let observation = source_bindings
                            .iter()
                            .find(|binding| binding.operand == *operand)?;
                        let mut destination = None;
                        for position in 0..required.bindings.len() {
                            if required.bindings[position].operand.primitive_type()
                                != observation.operand.primitive_type()
                            {
                                continue;
                            }
                            let support = super::coverage::dependency(
                                program,
                                facts,
                                captures,
                                selections,
                                conditions,
                                &Alias {
                                    place: observation.place.clone(),
                                    epoch: alias.epoch,
                                },
                                position,
                            );
                            if super::coverage::implies(facts, conditions, selection, support) {
                                destination = Some(position);
                                break;
                            }
                        }
                        let Some(position) = destination else {
                            let value = crate::values::scalar_value_at_place(
                                program,
                                &facts.semantic,
                                contexts
                                    .iter()
                                    .map(|context| facts.semantic.contexts.get(*context)),
                                &observation.place,
                            )?;
                            *operand = super::super::call_values::observed_literal(
                                value,
                                observation.operand.primitive_type()?,
                            )?;
                            return Some(());
                        };
                        let binding = &required.bindings[position];
                        *operand = binding.operand.clone();
                        let rebound = BoundObservation {
                            operand: operand.clone(),
                            place: binding.place.clone(),
                            type_reference: binding.type_reference,
                        };
                        if !predicate.bindings.contains(&rebound) {
                            predicate.bindings.push(rebound);
                        }
                        Some(())
                    },
                );
                if complete.is_none() {
                    continue;
                }
                predicate.machine = required.machine;
                predicate.entry_state = required.entry_state;
                if let Some(known) = transported
                    .iter_mut()
                    .find(|known| known.predicate == predicate)
                {
                    known.condition = conditions.any(known.condition, selection);
                } else {
                    transported.push(ConditionalPredicate {
                        predicate,
                        condition: selection,
                    });
                }
            }
        }
    }
    transported
}
