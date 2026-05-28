mod branching;
mod operations;

use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::name::Identifier;
use omega_checked_trees::state::State;
use omega_checked_trees::statement::{StatementNode, TableTransition, TransitionGuardNode};
use omega_core::arena::{Arena, HandleSpan};
use omega_state_graph::{Operation, StateGraph, StateKey, StateParameterNode};

use self::branching::BranchCallTargetResolver;
use self::operations::{operation_expression_refs, operation_kind};

#[derive(Debug, Clone)]
pub(super) struct StateSegment {
    pub key: StateKey,
    pub name: Identifier,
    pub parameters: HandleSpan<StateParameterNode>,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<SegmentTransition>,
    pub next_segment_key: StateKey,
}

#[derive(Debug, Clone)]
pub(super) enum SegmentTransition {
    Tree {
        statement_index: usize,
        table: TableTransition,
    },
    ReturnExpression {
        statement_index: usize,
        expression: ExpressionHandle,
    },
    BranchCall {
        statement_index: usize,
        has_continuation_segment: bool,
    },
}

impl Default for SegmentTransition {
    fn default() -> Self {
        Self::Tree {
            statement_index: 0,
            table: TableTransition::default(),
        }
    }
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
            expressions: operation_expression_refs(
                table_statement,
                &program.expression_table,
                state_graph,
                &program.statement_table,
            ),
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

fn branch_call_transitions(
    statement_index: usize,
    has_continuation_segment: bool,
    segment_transitions: &mut Arena<SegmentTransition>,
) -> HandleSpan<SegmentTransition> {
    let mut transitions = HandleSpan::empty();
    segment_transitions.append_to_span(
        &mut transitions,
        SegmentTransition::BranchCall {
            statement_index,
            has_continuation_segment,
        },
    );
    transitions
}

pub(super) fn segment_has_unconditional_transition(
    segment: &StateSegment,
    segment_transitions: &Arena<SegmentTransition>,
) -> bool {
    segment_transitions
        .span_or_empty(segment.transitions)
        .iter()
        .any(|transition| match transition {
            SegmentTransition::Tree { table, .. } => {
                matches!(table.guard, TransitionGuardNode::Always)
            }
            SegmentTransition::ReturnExpression { .. } => true,
            SegmentTransition::BranchCall { .. } => true,
        })
}

fn segment_name(state_name: &Identifier, _segment_index: usize) -> Identifier {
    state_name.clone()
}

fn state_parameters_for_segment(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    state: &State,
    segment_index: usize,
) -> HandleSpan<StateParameterNode> {
    if segment_index > 0 {
        return HandleSpan::empty();
    }

    let mut parameters = HandleSpan::empty();
    for parameter in program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
    {
        state_graph.state_parameters.append_to_span(
            &mut parameters,
            StateParameterNode {
                symbol: parameter.symbol,
                name: parameter.name.clone(),
                type_reference: parameter.type_reference,
                type_symbol: program.type_reference_symbol(parameter.type_reference),
                type_name: Identifier::generated(
                    program.display_type_reference(parameter.type_reference),
                ),
                is_mutable_reference: matches!(
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    omega_checked_trees::types::TypeReferenceNode::Reference {
                        is_mutable: true,
                        ..
                    }
                ),
            },
        );
    }

    parameters
}

pub(super) fn copy_statement_expression_span(
    state_graph: &mut StateGraph,
    source_expressions: &ExpressionTable,
    statement_table: &omega_checked_trees::statement::StatementTable,
    expressions: omega_core::arena::HandleSpan<ExpressionHandle>,
) -> omega_core::arena::HandleSpan<ExpressionHandle> {
    state_graph.expressions.copy_expression_handles_from_slice(
        source_expressions,
        statement_table.expression_handles(expressions),
    )
}

pub(super) fn table_transition_guard_expression(transition: TableTransition) -> ExpressionHandle {
    match transition.guard {
        TransitionGuardNode::Always => ExpressionHandle::invalid(),
        TransitionGuardNode::When(expression) => expression,
    }
}
