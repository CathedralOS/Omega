use super::*;
use psi_arena::{Handle, HandleSpan};
use psi_checked_trees::machine::Machine;
use psi_checked_trees::signature::StateSignature;
use psi_checked_trees::state::State;
use psi_checked_trees::statement::StatementNode;
use psi_checked_trees::trait_definition::TraitDefinition;

const MACHINE: u32 = 1;
const STATE: u32 = 2;
const RECEIVER: u32 = 3;
const TARGET: u32 = 4;
const BOUNDARY_TRAIT: u32 = 5;
const FIRST_SIGNATURE: u32 = 6;
const SECOND_SIGNATURE: u32 = 7;
const OTHER_TRAIT: u32 = 8;
const OTHER_SIGNATURE: u32 = 9;

struct Fixture {
    program: CheckedTrees,
    key: StateKey,
    flow_state: Handle<psi_checked_trees::FlowStateFact>,
    call: Handle<psi_checked_trees::FlowCallFact>,
    first_edge: Handle<FlowBoundaryEdgeFact>,
    second_edge: Handle<FlowBoundaryEdgeFact>,
}

fn symbol(index: u32) -> SymbolHandle {
    SymbolHandle::from_arena_index(index)
}

fn fixture() -> Fixture {
    let mut program = CheckedTrees::default();
    let mut state = State {
        symbol: symbol(STATE),
        ..Default::default()
    };
    program
        .typed
        .statement_table
        .push_statement(&mut state.statement_nodes, StatementNode::default());
    let mut machine = Machine {
        symbol: symbol(MACHINE),
        ..Default::default()
    };
    program.typed.push_machine_state(&mut machine, state);
    program.typed.push_machine(machine);

    let mut boundary = TraitDefinition {
        symbol: symbol(BOUNDARY_TRAIT),
        is_boundary: true,
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut boundary,
        StateSignature {
            symbol: symbol(FIRST_SIGNATURE),
            ..Default::default()
        },
    );
    program.typed.push_trait_machine_signature(
        &mut boundary,
        StateSignature {
            symbol: symbol(SECOND_SIGNATURE),
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(boundary);
    let mut other = TraitDefinition {
        symbol: symbol(OTHER_TRAIT),
        is_boundary: true,
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut other,
        StateSignature {
            symbol: symbol(OTHER_SIGNATURE),
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(other);

    let mut boundary_edges = HandleSpan::empty();
    let first_edge = program.facts.flow.boundaries.edges.append_to_span(
        &mut boundary_edges,
        FlowBoundaryEdgeFact {
            statement_index: 0,
            call_ordinal: 1,
            receiver_symbol: symbol(RECEIVER),
            target_symbol: symbol(TARGET),
            boundary_trait_symbol: symbol(BOUNDARY_TRAIT),
            boundary_signature_symbol: symbol(FIRST_SIGNATURE),
        },
    );
    let second_edge = program.facts.flow.boundaries.edges.append_to_span(
        &mut boundary_edges,
        FlowBoundaryEdgeFact {
            statement_index: 0,
            call_ordinal: 1,
            receiver_symbol: symbol(RECEIVER),
            target_symbol: symbol(TARGET),
            boundary_trait_symbol: symbol(BOUNDARY_TRAIT),
            boundary_signature_symbol: symbol(SECOND_SIGNATURE),
        },
    );
    let call = program
        .facts
        .flow
        .control
        .calls
        .append(psi_checked_trees::FlowCallFact {
            statement_index: 0,
            call_ordinal: 1,
            receiver_symbol: symbol(RECEIVER),
            target_symbol: symbol(TARGET),
            boundary_edges,
            ..Default::default()
        });
    let flow_state = program
        .facts
        .flow
        .control
        .states
        .append(psi_checked_trees::FlowStateFact {
            machine_symbol: symbol(MACHINE),
            state_symbol: symbol(STATE),
            boundary_edges,
            calls: HandleSpan::from_parts(call, 1),
            ..Default::default()
        });

    Fixture {
        program,
        key: StateKey {
            machine: symbol(MACHINE),
            state: symbol(STATE),
            segment_index: 0,
        },
        flow_state,
        call,
        first_edge,
        second_edge,
    }
}

fn error(fixture: &Fixture) -> String {
    state_boundary_summary(&mut StateGraph::default(), &fixture.program, fixture.key)
        .expect_err("malformed boundary carrier must fail closed")
        .message
}

#[test]
fn exact_boundary_carrier_copies_empty_and_multiple_edges_in_order() {
    let honest = fixture();
    let mut state_graph = StateGraph::default();
    let summary = state_boundary_summary(&mut state_graph, &honest.program, honest.key)
        .expect("exact boundary carrier");
    let edges = state_graph
        .semantics
        .boundaries
        .edges
        .span(summary.edges)
        .expect("copied edge span");
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].boundary_signature_symbol, symbol(FIRST_SIGNATURE));
    assert_eq!(edges[1].boundary_signature_symbol, symbol(SECOND_SIGNATURE));
    assert_eq!(edges[0].receiver_symbol, symbol(RECEIVER));
    assert_eq!(edges[0].target_symbol, symbol(TARGET));

    let mut empty = fixture();
    empty
        .program
        .facts
        .flow
        .control
        .calls
        .get_mut(empty.call)
        .boundary_edges = HandleSpan::empty();
    let state = empty
        .program
        .facts
        .flow
        .control
        .states
        .get_mut(empty.flow_state);
    state.boundary_edges = HandleSpan::empty();
    let summary = state_boundary_summary(&mut StateGraph::default(), &empty.program, empty.key)
        .expect("exact empty boundary carrier");
    assert!(summary.edges.is_empty());
}

#[test]
fn flow_state_and_spans_fail_closed() {
    let mut missing = fixture();
    missing
        .program
        .facts
        .flow
        .control
        .states
        .get_mut(missing.flow_state)
        .state_symbol = symbol(90);
    assert!(error(&missing).contains("FlowState coordinate"));

    let mut duplicate = fixture();
    let row = duplicate
        .program
        .facts
        .flow
        .control
        .states
        .get(duplicate.flow_state)
        .clone();
    duplicate.program.facts.flow.control.states.append(row);
    assert!(error(&duplicate).contains("FlowState coordinate"));

    let mut invalid_state_span = fixture();
    invalid_state_span
        .program
        .facts
        .flow
        .control
        .states
        .get_mut(invalid_state_span.flow_state)
        .boundary_edges = HandleSpan::from_parts(Handle::from_arena_index(90), 1);
    assert!(error(&invalid_state_span).contains("state edge span"));

    let mut invalid_call_span = fixture();
    invalid_call_span
        .program
        .facts
        .flow
        .control
        .calls
        .get_mut(invalid_call_span.call)
        .boundary_edges = HandleSpan::from_parts(Handle::from_arena_index(90), 1);
    assert!(error(&invalid_call_span).contains("call edge span"));
}

#[test]
fn exact_call_coordinate_and_state_carrier_fail_closed() {
    let mut mismatch = fixture();
    mismatch
        .program
        .facts
        .flow
        .boundaries
        .edges
        .get_mut(mismatch.first_edge)
        .receiver_symbol = symbol(90);
    assert!(error(&mismatch).contains("exact call coordinate"));

    let mut out_of_range = fixture();
    out_of_range
        .program
        .facts
        .flow
        .control
        .calls
        .get_mut(out_of_range.call)
        .statement_index = 1;
    assert!(error(&out_of_range).contains("out of range"));

    let mut duplicate_call = fixture();
    let duplicate = duplicate_call
        .program
        .facts
        .flow
        .control
        .calls
        .get(duplicate_call.call)
        .clone();
    duplicate_call
        .program
        .facts
        .flow
        .control
        .calls
        .append(duplicate);
    duplicate_call
        .program
        .facts
        .flow
        .control
        .states
        .get_mut(duplicate_call.flow_state)
        .calls = HandleSpan::from_parts(duplicate_call.call, 2);
    assert!(error(&duplicate_call).contains("call coordinate is duplicated"));

    let mut detached = fixture();
    let second = detached
        .program
        .facts
        .flow
        .boundaries
        .edges
        .get(detached.second_edge)
        .clone();
    let first = detached
        .program
        .facts
        .flow
        .boundaries
        .edges
        .get(detached.first_edge)
        .clone();
    let reversed = detached
        .program
        .facts
        .flow
        .boundaries
        .edges
        .insert_many([second, first]);
    detached
        .program
        .facts
        .flow
        .control
        .states
        .get_mut(detached.flow_state)
        .boundary_edges = reversed;
    assert!(error(&detached).contains("detached, reordered, or incomplete"));

    let mut duplicate_edge = fixture();
    let first = duplicate_edge
        .program
        .facts
        .flow
        .boundaries
        .edges
        .get(duplicate_edge.first_edge)
        .clone();
    *duplicate_edge
        .program
        .facts
        .flow
        .boundaries
        .edges
        .get_mut(duplicate_edge.second_edge) = first;
    assert!(error(&duplicate_edge).contains("edge row is duplicated"));
}

#[test]
fn boundary_trait_and_owned_signature_fail_closed() {
    let mut non_boundary = fixture();
    let first_trait = non_boundary.program.typed.roots.traits.start();
    non_boundary
        .program
        .typed
        .tables
        .traits
        .get_mut(first_trait)
        .is_boundary = false;
    assert!(error(&non_boundary).contains("non-boundary trait"));

    let mut missing_trait = fixture();
    missing_trait
        .program
        .facts
        .flow
        .boundaries
        .edges
        .get_mut(missing_trait.first_edge)
        .boundary_trait_symbol = symbol(90);
    assert!(error(&missing_trait).contains("trait is missing"));

    let mut cross_owned = fixture();
    cross_owned
        .program
        .facts
        .flow
        .boundaries
        .edges
        .get_mut(cross_owned.first_edge)
        .boundary_signature_symbol = symbol(OTHER_SIGNATURE);
    assert!(error(&cross_owned).contains("cross-owned"));
}
