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
    if ![
        (captured.observation_type, &captured.observation_projection),
        (captured.type_reference, &captured.projection),
    ]
    .into_iter()
    .enumerate()
    .all(|(position, (reference, projection))| {
        validation::project_type_reference_from_segments(program, reference, &projection.remaining)
            .is_some_and(|selected| {
                crate::flow::operand_binds_stable_copy(program, selected)
                    || (position == 1 && captured.producer_epoch.is_some())
            })
    }) {
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
    // A typed receiving read may detach contents from a producer that remains
    // a live descriptor. Its recorded read epoch owns that source identity;
    // later actuals observe the copy, not a new read of the descriptor. Without
    // that cut, a live view still owes preservation through the actual's epoch.
    let producer_epoch = captured.producer_epoch.unwrap_or(captured.operand);
    if crate::flow::operand_binds_live_view(program, captured.type_reference)
        && !crate::flow::operand_observations_agree(
            program,
            &facts.flow,
            captured.operand.row(),
            captured.producer_epoch.map_or(epoch, |read| read.row()),
            &produced.projected,
        )
    {
        return;
    }
    // Geometry alone does not identify the observed value. The source's
    // reference prefixes were evaluated before its selectors: a selector may
    // write payload without redirecting them, but replacing a descriptor makes
    // the completed scalar unavailable as an alias of the current source slot.
    // Keep its original local snapshot binding even when this optional relation
    // cannot be established.
    let Some(prefixes) = super::capture_coordinates::borrowed_bindings(
        program,
        captured.operand.row(),
        &captured.projection.remaining,
    ) else {
        return;
    };
    if prefixes.iter().any(|prefix| {
        !crate::flow::operand_entry_binding_origin_is_stable(
            program,
            &facts.flow,
            captured.operand.row(),
            prefix,
        )
    }) {
        return;
    }
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
                epoch: producer_epoch.row(),
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
            facts::ProgramPoint::Statement {
                machine_symbol: state.machine_symbol,
                state_symbol: capture.epoch.state_symbol,
                statement_index: capture.epoch.statement_index,
            },
        );
        let origin = facts
            .flow
            .control
            .state(state.machine_symbol, capture.epoch.state_symbol);
        if let (Some(frames), Some(origin)) = (frames, origin) {
            predicates.extend(super::prover::call_guarantees::current_predicates(
                program,
                facts,
                origin,
                super::prover::call_guarantees::GuaranteeUse::StatementEntry(
                    capture.epoch.statement_index + 1,
                ),
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

/// Rejoin a constructed value's scalar observation with its typed completion. The
/// active destination token owns availability; its original place/point names
/// one actual child capture even after an outer projection kept only a loan.
/// Neither source syntax nor an initializer can replace a missing capture.
pub(crate) fn cleanup_predicates(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    contexts: &[facts::FactContextHandle],
    required: &mut BoundPredicate,
    mut stable: impl FnMut(&BoundObservation) -> bool,
) -> Vec<BoundPredicate> {
    let mut captures = Vec::new();
    for (position, binding) in required.bindings.iter().enumerate() {
        if !stable(binding) {
            continue;
        }
        let mut origin = None;
        let mut ambiguous = false;
        let mut origin_depth = 0;
        for fact in contexts.iter().flat_map(|context| {
            facts
                .semantic
                .context_view(facts.semantic.contexts.get(*context))
                .facts()
        }) {
            let facts::FactPayload::AssignedValueCapture { source, point } = fact.payload else {
                continue;
            };
            let Some(destination) = fact.place.as_place().and_then(|place| {
                crate::flow::canonical_place_from_semantic_place(
                    program,
                    &facts.semantic,
                    facts.semantic.places.get(place),
                )
            }) else {
                continue;
            };
            if destination.root != binding.place.root {
                continue;
            }
            let Some(relative) = binding
                .place
                .segments
                .strip_prefix(destination.segments.as_slice())
            else {
                continue;
            };
            let depth = destination.segments.len();
            if origin.is_some() && depth < origin_depth {
                continue;
            }
            if depth > origin_depth {
                origin = None;
                ambiguous = false;
            }
            origin_depth = depth;
            let candidate = (source, point, relative.to_vec());
            if origin
                .as_ref()
                .is_some_and(|previous| *previous != candidate)
            {
                ambiguous = true;
            }
            origin = Some(candidate);
        }
        let Some((
            source,
            facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            relative,
        )) = origin
        else {
            continue;
        };
        if ambiguous || machine_symbol != state.machine_symbol {
            continue;
        }
        let source = facts.semantic.places.get(source);
        let Some(parent) = source.root.as_expression() else {
            continue;
        };
        let path = facts.semantic.place_segments.span_or_empty(source.segments);
        let mut matching = facts
            .flow
            .control
            .operand_children
            .iter()
            .filter_map(|(_, child)| {
                let operand =
                    crate::flow::CapturedOperand::recorded(&facts.flow.control, child.value)?;
                (child.parent == parent
                    && operand.state_symbol == state_symbol
                    && operand.statement_index == statement_index
                    && facts
                        .flow
                        .control
                        .operand_child_paths
                        .span_or_empty(child.path)
                        == path)
                    .then_some((child, operand))
            });
        let Some((child, operand)) = matching.next() else {
            continue;
        };
        let selected = validation::project_type_reference_from_segments(
            program,
            child.type_reference,
            &relative,
        );
        // Extending a non-tail child span copies its reference rows. Those
        // aliases still name one completed operand, not another evaluation.
        // Distinct operand handles remain ambiguous even when their expressions
        // or facts look equal; the declared type is part of the capture identity.
        if matching.any(|(other, _)| {
            other.value != child.value || other.type_reference != child.type_reference
        }) || selected.and_then(|reference| program.primitive_type_reference(reference))
            != binding.operand.primitive_type()
        {
            continue;
        }
        let Some(mut values) = crate::flow::captured_operand_values(
            program,
            &facts.flow,
            &facts.semantic,
            state,
            operand,
            child.type_reference,
            &relative,
        ) else {
            continue;
        };
        if values.len() != 1 {
            continue;
        }
        let captured = values.pop().expect("one completed scalar");
        let Some(observed) =
            crate::flow::captured_observation_places(program, &facts.flow, &captured)
        else {
            continue;
        };
        let [observed] = observed.as_slice() else {
            continue;
        };
        {
            let Some((machine, body)) =
                validation::find_state(program, captured.observation.state_symbol)
            else {
                continue;
            };
            if !validation::value_has_builtin_coordinates(
                program,
                machine,
                body,
                captured.observation_projection.expression,
            ) {
                continue;
            }
        }
        let mut copied = binding.clone();
        copied.place = observed.projected.clone();
        retain_bindings(
            program,
            facts,
            &captured,
            operand.row(),
            &[copied],
            position,
            &mut captures,
        );
    }
    if captures.is_empty() {
        return Vec::new();
    }
    let frames = validation::CallFrameResolver::new(program);
    transport(program, facts, state, required, &captures, frames.as_ref())
}
