//! Explicit state inputs derived from live edge contexts, not declarations.

use super::*;
use psi_facts::ScalarValue;
use psi_typed_trees::statement::{TransitionExit, TransitionTargetNode};

#[cfg(test)]
mod tests;

/// A missing row is unreachable. A present unknown value must
/// participate in the incoming meet; it is never an omitted predecessor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StateValues {
    state: SymbolHandle,
    values: Vec<(SymbolHandle, ScalarValue)>,
}

fn reachable(
    ctx: &FlowBuildContext,
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: SymbolHandle,
) -> bool {
    program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == state)
        || ctx
            .state_value_inputs
            .iter()
            .any(|input| input.state == state)
}

fn join(ctx: &mut FlowBuildContext, incoming: StateValues) {
    let state = incoming.state;
    let mut changed = false;
    if let Some(previous) = ctx
        .state_value_inputs
        .iter_mut()
        .find(|row| row.state == incoming.state)
    {
        for (parameter, value) in &mut previous.values {
            let next = incoming
                .values
                .iter()
                .find(|row| row.0 == *parameter)
                .map(|row| row.1.clone())
                .unwrap_or_default();
            if *value != next {
                changed |= *value != ScalarValue::Unknown;
                *value = ScalarValue::Unknown;
            }
        }
    } else {
        changed = true;
        ctx.state_value_inputs.push(incoming);
    }
    if changed && ctx.built_state_value_inputs.contains(&state) {
        ctx.state_value_inputs_changed_after_build = true;
    }
}

pub(super) fn unknown_inputs(program: &psi_typed_trees::TypedTrees) -> Vec<StateValues> {
    program
        .machines()
        .iter()
        .flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(|state| StateValues {
                    state: state.symbol,
                    values: program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| !parameter.is_self)
                        .map(|parameter| (parameter.symbol, ScalarValue::Unknown))
                        .collect(),
                })
        })
        .collect()
}

fn literal(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<ScalarValue> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_bignum().map(ScalarValue::Integer),
        ExpressionNode::Boolean(value) => Some(ScalarValue::Boolean(*value)),
        _ => None,
    }
}

fn value_at_place(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    ctx: &FlowBuildContext,
    contexts: HandleSpan<FlowSemanticContextRef>,
    place: psi_facts::PlaceHandle,
) -> ScalarValue {
    let Some(place) =
        canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
    else {
        return ScalarValue::Unknown;
    };
    crate::values::scalar_value_at_place(
        program,
        semantic,
        ctx.contexts
            .semantic_context_refs
            .span_or_empty(contexts)
            .iter()
            .map(|reference| semantic.contexts.get(reference.context)),
        &place,
    )
    .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn record_transition(
    program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    transition: &psi_typed_trees::statement::TableTransition,
    target: psi_typed_trees::statement::TransitionTargetHandle,
    contexts: HandleSpan<FlowSemanticContextRef>,
    operand_writes: Option<&[CanonicalPlace]>,
) {
    if transition.exit != TransitionExit::Ordinary
        || !reachable(ctx, program, machine, state.symbol)
    {
        return;
    }
    let (destination, arguments) = match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            let Some(destination) = program
                .machine_states(machine)
                .iter()
                .find(|candidate| candidate.symbol == path.symbol)
            else {
                return;
            };
            (
                destination,
                Some(program.statement_table.expression_handles(*arguments)),
            )
        }
        TransitionTargetNode::SelfTarget => (state, None),
        _ => return,
    };
    if program
        .machine_states(machine)
        .first()
        .is_some_and(|entry| entry.symbol == destination.symbol)
    {
        return;
    }
    let mut ordinal = 0;
    let mut values = Vec::new();
    for parameter in program.state_parameters(destination) {
        if parameter.is_self {
            continue;
        }
        let argument = arguments
            .and_then(|arguments| arguments.get(ordinal))
            .copied();
        ordinal += 1;
        let scalar = program
            .primitive_type_reference(parameter.type_reference)
            .is_some_and(|primitive| {
                primitive.accepts_integer_literal()
                    || primitive == psi_typed_trees::types::PrimitiveType::Bool
            });
        let value = if !scalar {
            ScalarValue::Unknown
        } else if let Some(value) = argument.and_then(|argument| literal(program, argument)) {
            value
        } else {
            let source = if arguments.is_none() {
                Some(semantic.append_symbol_place(parameter.symbol))
            } else {
                argument.and_then(|argument| {
                    crate::semantic_places::canonical_place_to_fact_place_in_state(
                        program,
                        semantic,
                        state.symbol,
                        statement_index,
                        argument,
                    )
                })
            };
            source
                .filter(|source| {
                    let Some(place) = canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(*source),
                    ) else {
                        return false;
                    };
                    // A dynamic selector has its own changing dependency; current
                    // AssignedValue facts do not yet retain that captured selector.
                    place.segments.iter().all(|segment| {
                        matches!(
                            segment,
                            psi_facts::PlaceSegment::Field { .. }
                                | psi_facts::PlaceSegment::Case { .. }
                                | psi_facts::PlaceSegment::FixedIndex { .. }
                        )
                    }) && operand_writes.is_some_and(|writes| {
                        writes.iter().all(|write| {
                            super::ownership::normalized_event_place_root(program, place.root)
                                != super::ownership::normalized_event_place_root(
                                    program, write.root,
                                )
                                || !canonical_place_segments_may_overlap(
                                    program,
                                    &place.segments,
                                    &write.segments,
                                )
                        })
                    })
                })
                .map(|source| value_at_place(program, semantic, ctx, contexts, source))
                .unwrap_or_default()
        };
        values.push((parameter.symbol, value));
    }
    join(
        ctx,
        StateValues {
            state: destination.symbol,
            values,
        },
    );
}

/// Ordinary invocations are not narrowed by the internal transition proof.
pub(super) fn record_invocation(
    program: &psi_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    call: &BorrowCallFact,
) {
    if !reachable(ctx, program, machine, state.symbol) {
        return;
    }
    let Some((owner, destination)) = program.machines().iter().find_map(|owner| {
        program
            .machine_states(owner)
            .iter()
            .find(|candidate| candidate.symbol == call.target_symbol)
            .map(|state| (owner, state))
    }) else {
        return;
    };
    if program
        .machine_states(owner)
        .first()
        .is_some_and(|entry| entry.symbol == destination.symbol)
    {
        return;
    }
    if owner.symbol == machine.symbol
        && matches!(
            find_call_site(
                program,
                machine.symbol,
                state.symbol,
                call.statement_index,
                call.call_ordinal
            ),
            Some(CallSite::TransitionNamed { .. })
        )
    {
        return;
    }
    join(
        ctx,
        StateValues {
            state: destination.symbol,
            values: program
                .state_parameters(destination)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| (parameter.symbol, ScalarValue::Unknown))
                .collect(),
        },
    );
}

pub(super) fn append_entry_context(
    _program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
) {
    let Some(input) = ctx
        .state_value_inputs
        .iter()
        .find(|input| input.state == state.symbol)
    else {
        return;
    };
    let point = ProgramPoint::State {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
    };
    for (parameter, value) in &input.values {
        if *value == ScalarValue::Unknown {
            continue;
        }
        let mut refs = HandleSpan::empty();
        let place = semantic.append_symbol_place(*parameter);
        let value = semantic.scalar_values.append(value.clone());
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::AssignedScalarValue { value },
        });
        semantic.append_ref(&mut refs, fact);
        semantic.append_context(point, refs);
    }
}
