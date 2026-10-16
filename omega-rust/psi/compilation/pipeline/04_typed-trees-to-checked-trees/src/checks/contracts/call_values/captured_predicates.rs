//! Transport completed value evidence into an invocation's observation namespace.
//!
//! Reaching stores already retain each child's producer and completion epoch.
//! A copied field can therefore use that epoch's predicates without pretending
//! its former source still holds the value. Every predicate observation must
//! join an actual captured value at an agreeing epoch; an unmatched dependency
//! drops the whole predicate. Current-storage guards remain independently
//! available through the ordinary invocation contexts.

use crate::facts::boolean_predicates::{BoundObservation, BoundPredicate};
use crate::flow::CapturedValue;
use checked_trees::{CheckFacts, FlowOperandFact, FlowStateFact};
use validation::CanonicalPlace;

pub(super) struct CapturedBinding<'facts> {
    position: usize,
    place: CanonicalPlace,
    epoch: &'facts FlowOperandFact,
}

pub(super) fn retain_bindings<'facts>(
    program: &dyn validation::CheckingInput,
    facts: &'facts CheckFacts,
    captured: &CapturedValue<'facts>,
    epoch: &'facts FlowOperandFact,
    bindings: &[BoundObservation],
    first_position: usize,
    retained: &mut Vec<CapturedBinding<'facts>>,
) {
    for (offset, binding) in bindings.iter().enumerate() {
        retained.push(CapturedBinding {
            position: first_position + offset,
            place: binding.place.clone(),
            epoch,
        });
    }
    if !crate::flow::operand_binds_stable_copy(program, captured.observation_type)
        || !crate::flow::operand_binds_stable_copy(program, captured.type_reference)
    {
        return;
    }
    let Some(observed) = crate::flow::captured_observation_places(program, &facts.flow, captured)
    else {
        return;
    };
    let Some(produced) = crate::flow::captured_value_places(program, &facts.flow, captured) else {
        return;
    };
    let ([observed], [produced]) = (observed.as_slice(), produced.as_slice()) else {
        return;
    };
    let Some(source_type) = validation::canonical_place_type_reference(
        program,
        captured.operand.state_symbol,
        captured.operand.statement_index,
        &produced.projected,
    ) else {
        return;
    };
    for (offset, binding) in bindings.iter().enumerate() {
        if binding.place == observed.projected
            && program.primitive_type_reference(source_type) == binding.operand.primitive_type()
        {
            retained.push(CapturedBinding {
                position: first_position + offset,
                place: produced.projected.clone(),
                epoch: captured.operand,
            });
        }
    }
}

pub(super) fn transport(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    required: &mut BoundPredicate,
    captures: &[CapturedBinding<'_>],
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<BoundPredicate> {
    join_identities(program, facts, required, captures);
    let mut transported = Vec::new();
    for capture in captures {
        let contexts = facts
            .flow
            .semantic_constraint_contexts(capture.epoch.constraints)
            .collect::<Vec<_>>();
        let mut predicates = crate::facts::boolean_predicates::retained(
            program,
            &facts.operators,
            &facts.semantic,
            &contexts,
        );
        if let Some(frames) = frames {
            predicates.extend(super::super::prover::call_guarantees::current_predicates(
                program,
                facts,
                state,
                capture.epoch.statement_index + 1,
                &contexts,
                frames,
            ));
        }
        while let Some(mut predicate) = predicates.pop() {
            // A true conjunction establishes each child independently. An
            // unrelated sibling's storage is not a dependency of that child.
            // Other connectives stay whole: neither side of a disjunction is
            // independently guaranteed merely because the disjunction is true.
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
                    let destination = captures.iter().find(|candidate| {
                        candidate.place == observation.place
                            && required.bindings[candidate.position]
                                .operand
                                .primitive_type()
                                == observation.operand.primitive_type()
                            && crate::flow::operand_observations_agree(
                                program,
                                &facts.flow,
                                capture.epoch,
                                candidate.epoch,
                                &observation.place,
                            )
                    })?;
                    let binding = &required.bindings[destination.position];
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
            if complete.is_some() && !transported.contains(&predicate) {
                transported.push(predicate);
            }
        }
    }
    transported
}

/// Substitution identity is stronger than language equality: two copies of the
/// same Float still feed identical comparisons when the value is NaN. Normalize
/// authenticated aliases to one observed place without inventing IEEE equality.
fn join_identities(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    required: &mut BoundPredicate,
    captures: &[CapturedBinding<'_>],
) {
    for (position, capture) in captures.iter().enumerate() {
        for other in &captures[..position] {
            let source = required.bindings[capture.position].clone();
            let destination = required.bindings[other.position].clone();
            if capture.position == other.position
                || source.place == destination.place
                || capture.place != other.place
                || source.operand.primitive_type() != destination.operand.primitive_type()
                || !crate::flow::operand_observations_agree(
                    program,
                    &facts.flow,
                    capture.epoch,
                    other.epoch,
                    &capture.place,
                )
            {
                continue;
            }
            // All occurrences of one observed place were epoch-checked by the
            // invocation binder. Replacing the whole class makes this join
            // transitive without conflating a source's distinct write epochs.
            for binding in &mut required.bindings {
                if binding.place == source.place
                    && binding.operand.primitive_type() == source.operand.primitive_type()
                {
                    binding.place = destination.place.clone();
                }
            }
        }
    }
}
