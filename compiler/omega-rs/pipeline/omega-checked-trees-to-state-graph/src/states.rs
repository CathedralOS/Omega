use omega_state_graph::{
    PlannedTransitionTarget, StateGraph, StateKey, StateNode, TransitionEdge,
    TransitionExpressionRefs,
};
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::machine::Machine;
use psi_diagnostics::Diagnostic;

use crate::borrows::state_borrow_summary;
use crate::boundaries::state_boundary_summary;
use crate::contracts::state_contract_summary;
use crate::machine_metadata::{
    state_blocking_summary, state_service_reach, state_suspension_summary,
};
use crate::ownership::state_ownership_summary;
use crate::segments::{SegmentTransition, StateSegment, segment_has_unconditional_transition};
use crate::transitions::plan_transition;
use crate::values::state_value_summary;

pub(crate) fn append_machine_states(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    machine: &Machine,
    segments: &[StateSegment],
    segment_transitions: &psi_arena::Arena<SegmentTransition>,
) -> Result<HandleSpan<StateNode>, Diagnostic> {
    validate_machine_segments(state_graph, program, machine, segments, segment_transitions)?;
    let mut states = HandleSpan::empty();

    for (index, segment) in segments.iter().enumerate() {
        let transitions = append_segment_transitions(
            state_graph,
            program,
            index,
            segment,
            segments,
            segment_transitions,
        )?;
        let contracts = state_contract_summary(state_graph, program, segment, segment_transitions);
        let values = state_value_summary(state_graph, program, segment.key);
        let boundaries = state_boundary_summary(state_graph, program, segment.key)?;
        let borrow = state_borrow_summary(state_graph, program, segment.key);
        let ownership = state_ownership_summary(state_graph, program, segment.key);
        state_graph.states.append_to_span(
            &mut states,
            StateNode {
                key: segment.key,
                name: segment.name.clone(),
                index,
                service_reach: state_service_reach(program, segment.key.state),
                suspension: state_suspension_summary(program, segment.key.state),
                blocking: state_blocking_summary(program, segment.key.state),
                parameters: segment.parameters,
                contracts,
                values,
                boundaries,
                borrow,
                ownership,
                operations: segment.operations,
                transitions,
            },
        );
    }

    Ok(states)
}

fn append_segment_transitions(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    segment_index: usize,
    segment: &StateSegment,
    segments: &[StateSegment],
    segment_transitions: &psi_arena::Arena<SegmentTransition>,
) -> Result<HandleSpan<TransitionEdge>, Diagnostic> {
    let mut transitions = HandleSpan::empty();

    for transition in segment_transitions.span_or_empty(segment.transitions) {
        let transition = plan_transition(segment.key, segments, transition, program, state_graph)?;
        state_graph
            .transitions
            .append_to_span(&mut transitions, transition);
    }

    if segment.next_segment_key.is_valid()
        && !segment_has_unconditional_transition(segment, segment_transitions)
    {
        let next_segment_key = segment.next_segment_key;
        let next_index = segment_index
            .checked_add(1)
            .ok_or_else(|| Diagnostic::error("internal state-graph segment index overflowed"))?;
        let next_segment = segments.get(next_index).ok_or_else(|| {
            Diagnostic::error(format!(
                "internal state-graph segment #{} was not adjacent",
                next_segment_key.segment_index
            ))
        })?;
        if next_segment.key != next_segment_key {
            return Err(Diagnostic::error(
                "internal state-graph adjacent segment key drifted after validation",
            ));
        }

        state_graph.transitions.append_to_span(
            &mut transitions,
            TransitionEdge {
                statement_index: 0,
                target: PlannedTransitionTarget::State {
                    index: next_index,
                    key: next_segment.key,
                    name: next_segment.name.clone(),
                },
                continuation: PlannedTransitionTarget::None,
                expressions: TransitionExpressionRefs::default(),
            },
        );
    }

    Ok(transitions)
}

fn validate_machine_segments(
    state_graph: &StateGraph,
    program: &CheckedTrees,
    machine: &Machine,
    segments: &[StateSegment],
    segment_transitions: &psi_arena::Arena<SegmentTransition>,
) -> Result<(), Diagnostic> {
    let mut machines = program
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine.symbol);
    let exact_machine = machines
        .next()
        .ok_or_else(|| Diagnostic::error("state-segment exact typed machine is missing"))?;
    if machines.next().is_some() {
        return Err(Diagnostic::error(
            "state-segment exact typed machine is duplicated",
        ));
    }
    if exact_machine != machine {
        return Err(Diagnostic::error(
            "state-segment selected machine disagrees with its exact typed row",
        ));
    }

    let typed_states = program.machine_states(exact_machine);
    for (index, state) in typed_states.iter().enumerate() {
        if !state.symbol.is_valid() {
            return Err(Diagnostic::error(
                "state-segment typed state identity is invalid",
            ));
        }
        if typed_states[..index]
            .iter()
            .any(|candidate| candidate.symbol == state.symbol)
        {
            return Err(Diagnostic::error(
                "state-segment typed state identity is duplicated",
            ));
        }
        if program.machines().iter().any(|candidate_machine| {
            candidate_machine.symbol != exact_machine.symbol
                && program
                    .machine_states(candidate_machine)
                    .iter()
                    .any(|candidate_state| candidate_state.symbol == state.symbol)
        }) {
            return Err(Diagnostic::error(
                "state-segment typed state identity belongs to more than one machine",
            ));
        }
    }

    let mut cursor = 0usize;
    for state in typed_states {
        let block_start = cursor;
        let mut expected_segment_index = 0usize;
        while let Some(segment) = segments.get(cursor)
            && segment.key.state == state.symbol
        {
            if segment.key.machine != exact_machine.symbol {
                return Err(Diagnostic::error(
                    "state-segment key belongs to another machine",
                ));
            }
            if segment.key.segment_index != expected_segment_index {
                return Err(Diagnostic::error(
                    "state-segment indices are not contiguous from zero",
                ));
            }
            if segment.name != state.name {
                return Err(Diagnostic::error(
                    "state-segment name disagrees with its exact typed state",
                ));
            }
            validate_segment_spans(state_graph, segment_transitions, segment)?;

            let next_is_same_state = segments
                .get(cursor + 1)
                .is_some_and(|next| next.key.state == state.symbol);
            if next_is_same_state {
                let next = &segments[cursor + 1];
                if segment.next_segment_key != next.key {
                    return Err(Diagnostic::error(
                        "state-segment next key does not identify the adjacent same-state segment",
                    ));
                }
            } else if segment.next_segment_key != StateKey::default() {
                return Err(Diagnostic::error(
                    "state-segment final row retains an unexpected next key",
                ));
            }

            cursor += 1;
            expected_segment_index = expected_segment_index
                .checked_add(1)
                .ok_or_else(|| Diagnostic::error("state-segment index overflowed"))?;
        }
        if cursor == block_start {
            return Err(Diagnostic::error(
                "state-segment typed state block is missing or out of carrier order",
            ));
        }
    }
    if cursor != segments.len() {
        return Err(Diagnostic::error(
            "state-segment carrier contains an absent, duplicated, or reordered state block",
        ));
    }
    Ok(())
}

fn validate_segment_spans(
    state_graph: &StateGraph,
    segment_transitions: &psi_arena::Arena<SegmentTransition>,
    segment: &StateSegment,
) -> Result<(), Diagnostic> {
    if !segment.parameters.is_empty()
        && state_graph
            .state_parameters
            .span_or_empty(segment.parameters)
            .is_empty()
    {
        return Err(Diagnostic::error("state-segment parameter span is invalid"));
    }
    if !segment.operations.is_empty()
        && state_graph
            .operations
            .span_or_empty(segment.operations)
            .is_empty()
    {
        return Err(Diagnostic::error("state-segment operation span is invalid"));
    }
    if !segment.transitions.is_empty()
        && segment_transitions
            .span_or_empty(segment.transitions)
            .is_empty()
    {
        return Err(Diagnostic::error(
            "state-segment transition span is invalid",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_state_graph::{Operation, StateParameterNode};
    use psi_arena::{Arena, Handle};
    use psi_checked_trees::name::Identifier;
    use psi_checked_trees::state::State;
    use psi_symbols::SymbolHandle;

    const MACHINE: u32 = 1;
    const FIRST_STATE: u32 = 2;
    const SECOND_STATE: u32 = 3;

    struct SegmentFixture {
        program: CheckedTrees,
        graph: StateGraph,
        transitions: Arena<SegmentTransition>,
        segments: Vec<StateSegment>,
    }

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_arena_index(index)
    }

    fn segment(state: u32, segment_index: usize, name: &str) -> StateSegment {
        StateSegment {
            key: StateKey {
                machine: symbol(MACHINE),
                state: symbol(state),
                segment_index,
            },
            name: Identifier::generated(name),
            parameters: HandleSpan::empty(),
            operations: HandleSpan::empty(),
            transitions: HandleSpan::empty(),
            next_segment_key: StateKey::default(),
        }
    }

    fn segment_fixture() -> SegmentFixture {
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: symbol(MACHINE),
            name: Identifier::generated("Root::run"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: symbol(FIRST_STATE),
                name: Identifier::generated("first"),
                ..Default::default()
            },
        );
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: symbol(SECOND_STATE),
                name: Identifier::generated("second"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);

        let mut segments = vec![
            segment(FIRST_STATE, 0, "first"),
            segment(FIRST_STATE, 1, "first"),
            segment(SECOND_STATE, 0, "second"),
        ];
        let next_key = segments[1].key;
        segments[0].next_segment_key = next_key;
        SegmentFixture {
            program,
            graph: StateGraph::default(),
            transitions: Arena::default(),
            segments,
        }
    }

    fn machine(program: &CheckedTrees) -> Machine {
        program
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol(MACHINE))
            .expect("fixture machine")
            .clone()
    }

    fn validate(fixture: &SegmentFixture) -> Result<(), Diagnostic> {
        validate_machine_segments(
            &fixture.graph,
            &fixture.program,
            &machine(&fixture.program),
            &fixture.segments,
            &fixture.transitions,
        )
    }

    fn error_message(result: Result<(), Diagnostic>) -> String {
        result
            .expect_err("invalid state-segment carrier must fail closed")
            .message
    }

    #[test]
    fn exact_ordered_partition_is_accepted_and_fallthrough_uses_adjacent_row() {
        let mut fixture = segment_fixture();
        validate(&fixture).expect("exact segment carrier");
        let transitions = append_segment_transitions(
            &mut fixture.graph,
            &fixture.program,
            0,
            &fixture.segments[0],
            &fixture.segments,
            &fixture.transitions,
        )
        .expect("exact adjacent fallthrough");
        let edge = &fixture.graph.transitions.span_or_empty(transitions)[0];
        assert!(matches!(
            edge.target,
            PlannedTransitionTarget::State { key, index: 1, .. }
                if key == fixture.segments[1].key
        ));
    }

    #[test]
    fn typed_machine_and_state_identities_must_be_unique() {
        let mut duplicate_machine = segment_fixture();
        let machine = machine(&duplicate_machine.program);
        duplicate_machine.program.typed.push_machine(machine);
        assert!(error_message(validate(&duplicate_machine)).contains("machine is duplicated"));

        let mut duplicate_state = segment_fixture();
        let machine_index = duplicate_state
            .program
            .machines()
            .iter()
            .position(|machine| machine.symbol == symbol(MACHINE))
            .expect("machine index");
        let mut owner = duplicate_state.program.machines()[machine_index].clone();
        duplicate_state.program.typed.push_machine_state(
            &mut owner,
            State {
                symbol: symbol(SECOND_STATE),
                name: Identifier::generated("second"),
                ..Default::default()
            },
        );
        duplicate_state.program.typed.machines_mut()[machine_index] = owner;
        assert!(error_message(validate(&duplicate_state)).contains("state identity is duplicated"));

        let mut cross_owned = segment_fixture();
        let mut foreign = Machine {
            symbol: symbol(40),
            name: Identifier::generated("Other::first"),
            ..Default::default()
        };
        cross_owned.program.typed.push_machine_state(
            &mut foreign,
            State {
                symbol: symbol(FIRST_STATE),
                name: Identifier::generated("first"),
                ..Default::default()
            },
        );
        cross_owned.program.typed.push_machine(foreign);
        assert!(error_message(validate(&cross_owned)).contains("belongs to more than one machine"));
    }

    #[test]
    fn carrier_rejects_missing_duplicated_reordered_and_cross_machine_blocks() {
        let mut missing = segment_fixture();
        missing.segments.pop();
        assert!(error_message(validate(&missing)).contains("block is missing"));

        let mut duplicated = segment_fixture();
        duplicated.segments.push(segment(FIRST_STATE, 0, "first"));
        assert!(
            error_message(validate(&duplicated))
                .contains("absent, duplicated, or reordered state block")
        );

        let mut reordered = segment_fixture();
        reordered.segments.swap(0, 2);
        assert!(error_message(validate(&reordered)).contains("block is missing"));

        let mut cross_machine = segment_fixture();
        cross_machine.segments[0].key.machine = symbol(99);
        assert!(error_message(validate(&cross_machine)).contains("another machine"));
    }

    #[test]
    fn carrier_rejects_noncontiguous_indices_and_name_drift() {
        let mut nonzero = segment_fixture();
        nonzero.segments[0].key.segment_index = 1;
        assert!(error_message(validate(&nonzero)).contains("not contiguous from zero"));

        let mut gap = segment_fixture();
        gap.segments[1].key.segment_index = 2;
        let next_key = gap.segments[1].key;
        gap.segments[0].next_segment_key = next_key;
        assert!(error_message(validate(&gap)).contains("not contiguous from zero"));

        let mut duplicate = segment_fixture();
        duplicate.segments[1].key.segment_index = 0;
        let next_key = duplicate.segments[1].key;
        duplicate.segments[0].next_segment_key = next_key;
        assert!(error_message(validate(&duplicate)).contains("not contiguous from zero"));

        let mut name_drift = segment_fixture();
        name_drift.segments[1].name = Identifier::generated("other");
        assert!(error_message(validate(&name_drift)).contains("name disagrees"));
    }

    #[test]
    fn carrier_rejects_missing_wrong_and_final_next_keys() {
        let mut missing = segment_fixture();
        missing.segments[0].next_segment_key = StateKey::default();
        assert!(error_message(validate(&missing)).contains("adjacent same-state segment"));

        let mut wrong = segment_fixture();
        let next_key = wrong.segments[2].key;
        wrong.segments[0].next_segment_key = next_key;
        assert!(error_message(validate(&wrong)).contains("adjacent same-state segment"));

        let mut final_row = segment_fixture();
        let next_key = final_row.segments[2].key;
        final_row.segments[1].next_segment_key = next_key;
        assert!(error_message(validate(&final_row)).contains("unexpected next key"));
    }

    #[test]
    fn carrier_rejects_invalid_parameter_operation_and_transition_spans() {
        let mut parameters = segment_fixture();
        parameters.segments[0].parameters =
            HandleSpan::from_parts(Handle::<StateParameterNode>::from_arena_index(999), 1);
        assert!(error_message(validate(&parameters)).contains("parameter span is invalid"));

        let mut operations = segment_fixture();
        operations.segments[0].operations =
            HandleSpan::from_parts(Handle::<Operation>::from_arena_index(999), 1);
        assert!(error_message(validate(&operations)).contains("operation span is invalid"));

        let mut transitions = segment_fixture();
        transitions.segments[0].transitions =
            HandleSpan::from_parts(Handle::<SegmentTransition>::from_arena_index(999), 1);
        assert!(error_message(validate(&transitions)).contains("transition span is invalid"));
    }
}
