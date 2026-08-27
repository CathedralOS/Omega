mod branching;
mod operations;
mod parameters;
mod transitions;

use omega_state_graph::{Operation, StateGraph, StateKey, StateParameterNode};
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::machine::Machine;
use psi_checked_trees::name::Identifier;
use psi_checked_trees::state::State;
use psi_checked_trees::statement::StatementNode;

use self::branching::BranchCallTargetResolver;
use self::operations::{operation_expression_refs, operation_kind};
use self::parameters::state_parameters_for_segment;
use self::transitions::branch_call_transitions;
pub(super) use self::transitions::{
    SegmentTransition, copy_statement_expression_span, segment_has_unconditional_transition,
    table_transition_guard_expression,
};

#[derive(Debug, Clone)]
pub(super) struct StateSegment {
    pub key: StateKey,
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterNode>,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<SegmentTransition>,
    pub next_segment_key: StateKey,
}

pub(super) fn split_state_segments(
    machine: &Machine,
    state: &State,
    program: &CheckedTrees,
    state_graph: &mut StateGraph,
    segment_transitions: &mut Arena<SegmentTransition>,
    segments: &mut Vec<StateSegment>,
) {
    let machine_symbol = machine.symbol;
    let state_symbol = state.symbol;
    let table_statements = program.statement_table.statements(state.statement_nodes);
    let first_segment_index = segments.len();
    let mut operations = HandleSpan::empty();
    let mut transitions = HandleSpan::empty();
    let mut segment_index = 0usize;
    let mut transition_section_started = false;
    let mut branch_call_targets =
        BranchCallTargetResolver::with_capacity(program.machine_states.len());

    for (statement_index, table_statement) in table_statements.iter().enumerate() {
        // Compile-time assembly assertions have already been discharged by
        // checked-tree validation and emit no state-graph operation.
        if matches!(table_statement, StatementNode::AssemblyFact(_)) {
            continue;
        }
        if let StatementNode::Transition(table) = table_statement {
            transition_section_started = true;
            segment_transitions.append_to_span(
                &mut transitions,
                SegmentTransition::Tree {
                    statement_index,
                    table: *table,
                },
            );
            continue;
        }

        if let StatementNode::Expression(expression) = table_statement
            && state.return_type.is_valid()
            && statement_index + 1 == table_statements.len()
        {
            segment_transitions.append_to_span(
                &mut transitions,
                SegmentTransition::ReturnExpression {
                    statement_index,
                    expression: *expression,
                },
            );
            continue;
        }

        if let StatementNode::Call(table_call) = table_statement
            && branch_call_targets
                .branch_call_target(program, machine, table_call)
                .is_some()
        {
            segments.push(StateSegment {
                key: StateKey {
                    machine: machine_symbol,
                    state: state_symbol,
                    segment_index,
                },
                name: segment_name(&state.name, segment_index),
                parameters: state_parameters_for_segment(
                    state_graph,
                    program,
                    state,
                    segment_index,
                ),
                operations,
                transitions: branch_call_transitions(
                    statement_index,
                    statement_index + 1 < table_statements.len(),
                    segment_transitions,
                ),
                next_segment_key: StateKey::default(),
            });

            operations = HandleSpan::empty();
            transitions = HandleSpan::empty();
            segment_index += 1;
            transition_section_started = false;
            continue;
        }

        if transition_section_started {
            segments.push(StateSegment {
                key: StateKey {
                    machine: machine_symbol,
                    state: state_symbol,
                    segment_index,
                },
                name: segment_name(&state.name, segment_index),
                parameters: state_parameters_for_segment(
                    state_graph,
                    program,
                    state,
                    segment_index,
                ),
                operations,
                transitions,
                next_segment_key: StateKey::default(),
            });

            operations = HandleSpan::empty();
            transitions = HandleSpan::empty();
            segment_index += 1;
            transition_section_started = false;
        }

        let operation = Operation {
            statement_index,
            kind: operation_kind(program, table_statement),
            expressions: operation_expression_refs(program, table_statement, state_graph),
        };
        state_graph
            .operations
            .append_to_span(&mut operations, operation);
    }

    segments.push(StateSegment {
        key: StateKey {
            machine: machine_symbol,
            state: state_symbol,
            segment_index,
        },
        name: segment_name(&state.name, segment_index),
        parameters: state_parameters_for_segment(state_graph, program, state, segment_index),
        operations,
        transitions,
        next_segment_key: StateKey::default(),
    });

    let new_segment_count = segments.len() - first_segment_index;
    if new_segment_count > 1 {
        for offset in 0..new_segment_count - 1 {
            let segment_index = first_segment_index + offset;
            let next_key = segments[segment_index + 1].key;
            segments[segment_index].next_segment_key = next_key;
        }
    }
}

fn segment_name(state_name: &Identifier, _segment_index: usize) -> Identifier {
    state_name.clone()
}
