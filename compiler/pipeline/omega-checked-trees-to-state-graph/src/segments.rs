use omega_checked_trees::Program;
use omega_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, NamePath,
};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::state::State;
use omega_checked_trees::statement::{
    StatementNode, TableAssignment, TableCall, TableTransition, TransitionGuard,
    TransitionGuardNode,
};
use omega_core::symbols::SymbolHandle;

use omega_core::arena::{Arena, HandleSpan};
use omega_state_graph::{
    Operation, OperationExpressionRefs, OperationKind, StateGraph, StateKey, StateParameterNode,
};

#[derive(Debug, Clone)]
pub(super) struct StateSegment {
    pub key: StateKey,
    pub name: ProgramName,
    pub parameters: HandleSpan<StateParameterNode>,
    pub operations: Arena<Operation>,
    pub transitions: Arena<SegmentTransition>,
    pub next_segment_name: Option<ProgramName>,
}

#[derive(Debug, Clone)]
pub(super) enum SegmentTransition {
    Tree {
        table: TableTransition,
    },
    BranchCall {
        table: TableCall,
        has_continuation_segment: bool,
    },
}

impl Default for SegmentTransition {
    fn default() -> Self {
        Self::Tree {
            table: TableTransition::default(),
        }
    }
}

pub(super) fn split_state_segments(
    machine: &Machine,
    state: &State,
    program: &Program,
    state_graph: &mut StateGraph,
) -> Vec<StateSegment> {
    let machine_symbol = machine.symbol;
    let state_symbol = state.symbol;
    let mut segments = Vec::new();
    let mut operations = Arena::new();
    let mut transitions = Arena::new();
    let mut segment_index = 0usize;
    let mut transition_section_started = false;

    let table_statements = program.statement_table.statements(state.statement_nodes);

    for (statement_index, table_statement) in table_statements.iter().enumerate() {
        if let StatementNode::Transition(table) = table_statement {
            transition_section_started = true;
            transitions.insert(SegmentTransition::Tree { table: *table });
            continue;
        }

        if let StatementNode::Call(table_call) = table_statement
            && branch_call_target(program, machine, table_call).is_some()
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
                    table_call.clone(),
                    statement_index + 1 < table_statements.len(),
                ),
                next_segment_name: None,
            });

            operations = Arena::new();
            transitions = Arena::new();
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
                next_segment_name: None,
            });

            operations = Arena::new();
            transitions = Arena::new();
            segment_index += 1;
            transition_section_started = false;
        }

        operations.insert(Operation {
            statement_index,
            kind: operation_kind(program, table_statement),
            expressions: operation_expression_refs(
                table_statement,
                &program.expression_table,
                state_graph,
                &program.statement_table,
            ),
        });
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
        next_segment_name: None,
    });

    if segments.len() > 1 {
        for segment_index in 0..segments.len() - 1 {
            let next_name = segments[segment_index + 1].name.clone();
            segments[segment_index].next_segment_name = Some(next_name);
        }
    }

    segments
}

fn branch_call_transitions(
    table: TableCall,
    has_continuation_segment: bool,
) -> Arena<SegmentTransition> {
    let mut transitions = Arena::new();
    transitions.insert(SegmentTransition::BranchCall {
        table,
        has_continuation_segment,
    });
    transitions
}

pub(super) fn segment_has_unconditional_transition(segment: &StateSegment) -> bool {
    segment
        .transitions
        .iter()
        .map(|(_, transition)| transition)
        .any(|transition| match transition {
            SegmentTransition::Tree { table } => matches!(table.guard, TransitionGuardNode::Always),
            SegmentTransition::BranchCall { .. } => true,
        })
}

fn segment_name(state_name: &ProgramName, segment_index: usize) -> ProgramName {
    if segment_index == 0 {
        state_name.clone()
    } else {
        ProgramName::generated(format!("{state_name}__segment_{segment_index}"))
    }
}

fn state_parameters_for_segment(
    state_graph: &mut StateGraph,
    program: &Program,
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
                type_symbol: program.type_reference_symbol(parameter.type_reference),
                type_name: ProgramName::generated(
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

fn operation_kind(program: &Program, table_statement: &StatementNode) -> OperationKind {
    match table_statement {
        StatementNode::Assignment(assignment) if is_static_assignment(program, *assignment) => {
            OperationKind::StaticAssignment
        }
        StatementNode::Assignment(assignment)
            if is_constant_integer_assignment(program, *assignment) =>
        {
            OperationKind::ConstantIntegerAssignment
        }
        StatementNode::Assignment(_) => OperationKind::Assignment,
        StatementNode::Call(call) => OperationKind::Call {
            receiver_symbol: call.receiver_symbol,
            target_symbol: call.target_symbol,
            receiver: statement_call_receiver_path(program, call),
            target: call.target.clone(),
        },
        StatementNode::Expression(_) => OperationKind::Expression,
        StatementNode::LocalData(_) => OperationKind::LocalData,
        StatementNode::Transition(_) => unreachable!("transitions are not operations"),
    }
}

fn statement_call_receiver_path(program: &Program, call: &TableCall) -> Option<NamePath> {
    let receiver = program.statement_table.name_path_members(call.receiver);
    if receiver.is_empty() {
        return None;
    }

    Some(NamePath::resolved(
        receiver.iter().cloned().collect(),
        call.receiver_symbol,
        call.receiver_symbol,
    ))
}

fn operation_expression_refs(
    statement: &StatementNode,
    source_expressions: &ExpressionTable,
    state_graph: &mut StateGraph,
    statement_table: &omega_checked_trees::statement::StatementTable,
) -> OperationExpressionRefs {
    match statement {
        StatementNode::Assignment(assignment) => OperationExpressionRefs::Assignment {
            target: state_graph
                .expressions
                .copy_from(source_expressions, assignment.target),
            value: state_graph
                .expressions
                .copy_from(source_expressions, assignment.value),
        },
        StatementNode::Call(call) => OperationExpressionRefs::Call {
            arguments: copy_statement_expression_span(
                state_graph,
                source_expressions,
                statement_table,
                call.arguments,
            ),
        },
        StatementNode::Expression(expression) => OperationExpressionRefs::Expression(
            state_graph
                .expressions
                .copy_from(source_expressions, *expression),
        ),
        StatementNode::LocalData(_) | StatementNode::Transition(_) => OperationExpressionRefs::None,
    }
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

fn is_static_assignment(program: &Program, assignment: TableAssignment) -> bool {
    let target_is_place = matches!(
        program.expression_table.expression(assignment.target),
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_)
    );
    let value_is_static = match program.expression_table.expression(assignment.value) {
        ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => true,
        ExpressionNode::Indexed(_) => true,
        ExpressionNode::Name(path) => {
            program
                .expression_table
                .name_path_members(path.members)
                .len()
                > 1
        }
        _ => false,
    };

    target_is_place && value_is_static
}

pub(super) fn transition_guard_from_table(
    program: &Program,
    transition: TableTransition,
) -> TransitionGuard {
    match transition.guard {
        TransitionGuardNode::Always => TransitionGuard::Always,
        TransitionGuardNode::When(expression) => {
            TransitionGuard::When(program.expression_table.to_tree(expression))
        }
    }
}

pub(super) fn table_transition_guard_expression(
    transition: TableTransition,
) -> Option<ExpressionHandle> {
    match transition.guard {
        TransitionGuardNode::Always => None,
        TransitionGuardNode::When(expression) => Some(expression),
    }
}

fn is_constant_integer_assignment(program: &Program, assignment: TableAssignment) -> bool {
    matches!(
        program.expression_table.expression(assignment.target),
        ExpressionNode::Name(path) if program.expression_table.name_path_members(path.members).len() == 1
    ) && matches!(
        program.expression_table.expression(assignment.value),
        ExpressionNode::Integer(_)
    )
}

fn branch_call_target<'program>(
    program: &'program Program,
    current_machine: &'program Machine,
    call: &'program TableCall,
) -> Option<&'program State> {
    branch_call_target_with_visited(program, current_machine, call, &mut Vec::new())
}

fn branch_call_target_with_visited<'program>(
    program: &'program Program,
    current_machine: &'program Machine,
    call: &'program TableCall,
    visiting: &mut Vec<(SymbolHandle, SymbolHandle)>,
) -> Option<&'program State> {
    let receiver = program.statement_table.name_path_members(call.receiver);
    let target_machine = if receiver.is_empty() || call.receiver_symbol == current_machine.symbol {
        current_machine
    } else {
        let contained_symbol = program
            .machine_contained_objects(current_machine)
            .iter()
            .find(|contained| contained.symbol == call.receiver_symbol)
            .map(|contained| contained.type_symbol)
            .or_else(|| {
                program
                    .data_definitions()
                    .iter()
                    .find(|data_definition| data_definition.name == current_machine.name)
                    .and_then(|data_definition| {
                        program
                            .data_members(data_definition)
                            .iter()
                            .find_map(|member| {
                                let omega_checked_trees::data::DataMember::Field(field) = member
                                else {
                                    return None;
                                };
                                if field.symbol != call.receiver_symbol {
                                    return None;
                                }
                                let field_type_name =
                                    type_reference_name_handle(program, field.type_reference);
                                program
                                    .machines()
                                    .iter()
                                    .find(|candidate| candidate.name == field_type_name)
                                    .map(|candidate| candidate.symbol)
                            })
                    })
            })?;
        program
            .machines()
            .iter()
            .find(|machine| machine.symbol == contained_symbol)?
    };

    let target_state = if call.target_symbol.is_valid() {
        program
            .machine_states(target_machine)
            .iter()
            .find(|state| state.symbol == call.target_symbol)
    } else {
        program
            .machine_states(target_machine)
            .iter()
            .find(|state| state.name == call.target)
    }?;

    state_has_branching_flow(program, target_machine, target_state, visiting)
        .then_some(target_state)
}

fn type_reference_name_handle(
    program: &Program,
    type_reference: omega_checked_trees::types::TypeReferenceHandle,
) -> omega_checked_trees::name::ProgramName {
    match program.type_reference_table.type_reference(type_reference) {
        omega_checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_name_handle(program, *referee)
        }
        omega_checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_name_handle(program, *base_type)
        }
        omega_checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
            type_reference_name_handle(program, *element_type)
        }
        omega_checked_trees::types::TypeReferenceNode::Slice { element_type } => {
            type_reference_name_handle(program, *element_type)
        }
        omega_checked_trees::types::TypeReferenceNode::Generic { base_name, .. } => {
            base_name.clone()
        }
        omega_checked_trees::types::TypeReferenceNode::Named { name, .. } => name.clone(),
        omega_checked_trees::types::TypeReferenceNode::Unit => {
            omega_checked_trees::name::ProgramName::default()
        }
    }
}

fn state_has_branching_flow(
    program: &Program,
    current_machine: &Machine,
    state: &State,
    visiting: &mut Vec<(SymbolHandle, SymbolHandle)>,
) -> bool {
    let visit_key = (current_machine.symbol, state.symbol);
    if visiting.contains(&visit_key) {
        return false;
    }
    visiting.push(visit_key);

    let has_branching_flow = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| match statement {
            StatementNode::Transition(_) => true,
            StatementNode::Call(call) => {
                branch_call_target_with_visited(program, current_machine, call, visiting).is_some()
            }
            _ => false,
        });

    visiting.pop();
    has_branching_flow
}
