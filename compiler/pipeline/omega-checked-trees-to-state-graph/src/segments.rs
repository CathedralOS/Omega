use omega_core::symbols::SymbolHandle;
use omega_checked_trees::Program;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::state::State;
use omega_checked_trees::statement::{
    Assignment, Call, Statement, StatementNode, TableCall, TableTransition, Transition,
    TransitionGuard, TransitionGuardNode,
};
use omega_checked_trees::types::TypeReference;

use omega_state_graph::{
    Operation, OperationExpressionRefs, OperationKind, StateGraph, StateKey, StateParameterNode,
};

#[derive(Debug, Clone)]
pub(super) struct StateSegment<'program> {
    pub key: StateKey,
    pub name: ProgramName,
    pub parameters: Vec<StateParameterNode>,
    pub operations: Vec<Operation>,
    pub transitions: Vec<SegmentTransition<'program>>,
    pub next_segment_name: Option<ProgramName>,
}

#[derive(Debug, Clone)]
pub(super) enum SegmentTransition<'program> {
    Tree {
        tree: &'program Transition,
        table: TableTransition,
    },
    BranchCall {
        call: &'program Call,
        table: TableCall,
        has_continuation_segment: bool,
    },
}

pub(super) fn split_state_segments<'program>(
    machine: &'program Machine,
    state: &'program State,
    program: &Program,
    state_graph: &mut StateGraph,
) -> Vec<StateSegment<'program>> {
    let machine_symbol = machine.symbol;
    let state_symbol = state.symbol;
    let mut segments = Vec::new();
    let mut operations = Vec::new();
    let mut transitions = Vec::new();
    let mut segment_index = 0usize;
    let mut transition_section_started = false;

    let table_statements = program.statement_table.statements(state.statement_nodes);

    for (statement_index, statement) in state.statements.iter().enumerate() {
        let table_statement = table_statements.get(statement_index);
        if let Statement::Transition(transition) = statement {
            transition_section_started = true;
            if let Some(StatementNode::Transition(table)) = table_statement {
                transitions.push(SegmentTransition::Tree {
                    tree: transition,
                    table: *table,
                });
            }
            continue;
        }

        if let Statement::Call(call) = statement
            && let Some(StatementNode::Call(table_call)) = table_statement
            && branch_call_target(program, machine, call).is_some()
        {
            segments.push(StateSegment {
                key: StateKey {
                    machine: machine_symbol,
                    state: state_symbol,
                    segment_index,
                },
                name: segment_name(&state.name, segment_index),
                parameters: state_parameters_for_segment(state, segment_index),
                operations,
                transitions: vec![SegmentTransition::BranchCall {
                    call,
                    table: table_call.clone(),
                    has_continuation_segment: statement_index + 1 < state.statements.len(),
                }],
                next_segment_name: None,
            });

            operations = Vec::new();
            transitions = Vec::new();
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
                parameters: state_parameters_for_segment(state, segment_index),
                operations,
                transitions,
                next_segment_name: None,
            });

            operations = Vec::new();
            transitions = Vec::new();
            segment_index += 1;
            transition_section_started = false;
        }

        operations.push(Operation {
            statement_index,
            kind: operation_kind(statement),
            expressions: table_statement
                .map(|statement| {
                    operation_expression_refs(
                        statement,
                        &program.expression_table,
                        state_graph,
                        &program.statement_table,
                    )
                })
                .unwrap_or_default(),
        });
    }

    segments.push(StateSegment {
        key: StateKey {
            machine: machine_symbol,
            state: state_symbol,
            segment_index,
        },
        name: segment_name(&state.name, segment_index),
        parameters: state_parameters_for_segment(state, segment_index),
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

pub(super) fn segment_has_unconditional_transition(segment: &StateSegment<'_>) -> bool {
    segment.transitions.iter().any(|transition| match transition {
        SegmentTransition::Tree { tree, .. } => tree.guard == TransitionGuard::Always,
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

fn state_parameters_for_segment(state: &State, segment_index: usize) -> Vec<StateParameterNode> {
    if segment_index > 0 {
        return Vec::new();
    }

    state
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| StateParameterNode {
            symbol: parameter.symbol,
            name: parameter.name.clone(),
            type_symbol: type_reference_symbol(&parameter.type_reference),
            type_name: ProgramName::generated(parameter.type_reference.display_name()),
            is_mutable_reference: matches!(
                parameter.type_reference,
                TypeReference::Reference {
                    is_mutable: true,
                    ..
                }
            ),
        })
        .collect()
}

fn operation_kind(statement: &Statement) -> OperationKind {
    match statement {
        Statement::Assignment(assignment) if is_static_assignment(assignment) => {
            OperationKind::StaticAssignment
        }
        Statement::Assignment(assignment) if is_constant_integer_assignment(assignment) => {
            OperationKind::ConstantIntegerAssignment
        }
        Statement::Assignment(_) => OperationKind::Assignment,
        Statement::Call(call) => OperationKind::Call {
            receiver_symbol: call.receiver_symbol,
            target_symbol: call.target_symbol,
            receiver: call.receiver.clone(),
            target: call.target.clone(),
        },
        Statement::Expression(_) => OperationKind::Expression,
        Statement::LocalData(_) => OperationKind::LocalData,
        Statement::Transition(_) => unreachable!("transitions are not operations"),
    }
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

fn is_static_assignment(assignment: &Assignment) -> bool {
    use omega_checked_trees::expression::Expression;

    let target_is_place = matches!(
        assignment.target,
        Expression::Name(_) | Expression::Indexed(_)
    );
    let value_is_static = match &assignment.value {
        Expression::Integer(_) | Expression::String(_) | Expression::StructLiteral(_) => true,
        Expression::Indexed(_) => true,
        Expression::Name(path) => path.len() > 1,
        _ => false,
    };

    target_is_place && value_is_static
}

pub(super) fn table_transition_guard_expression(
    transition: TableTransition,
) -> Option<ExpressionHandle> {
    match transition.guard {
        TransitionGuardNode::Always => None,
        TransitionGuardNode::When(expression) => Some(expression),
    }
}

fn is_constant_integer_assignment(assignment: &Assignment) -> bool {
    matches!(
        (&assignment.target, &assignment.value),
        (omega_checked_trees::expression::Expression::Name(path), omega_checked_trees::expression::Expression::Integer(_))
            if path.len() == 1
    )
}

fn branch_call_target<'program>(
    program: &'program Program,
    current_machine: &'program Machine,
    call: &'program Call,
) -> Option<&'program State> {
    branch_call_target_with_visited(program, current_machine, call, &mut Vec::new())
}

fn branch_call_target_with_visited<'program>(
    program: &'program Program,
    current_machine: &'program Machine,
    call: &'program Call,
    visiting: &mut Vec<(SymbolHandle, SymbolHandle)>,
) -> Option<&'program State> {
    let target_machine = if call.receiver.is_none()
        || call
            .receiver
            .as_ref()
            .is_some_and(|receiver| receiver.len() == 1 && receiver[0] == "self")
    {
        current_machine
    } else {
        let receiver_name = call.receiver.as_ref().and_then(|receiver| receiver.last());
        let contained_symbol = program
            .machine_contained_objects(current_machine)
            .iter()
            .find(|contained| {
                contained.symbol == call.receiver_symbol
                    || receiver_name.is_some_and(|receiver_name| contained.name == *receiver_name)
            })
            .map(|contained| contained.type_symbol)
            .or_else(|| {
                program
                    .data_definitions()
                    .iter()
                    .find(|data_definition| data_definition.name == current_machine.name)
                    .and_then(|data_definition| {
                        data_definition.members.iter().find_map(|member| {
                            let omega_checked_trees::data::DataMember::Field(field) = member else {
                                return None;
                            };
                            let matches_receiver = field.symbol == call.receiver_symbol
                                || receiver_name.is_some_and(|name| field.name == *name);
                            if !matches_receiver {
                                return None;
                            }
                            let field_type_name = type_reference_name(&field.type_reference);
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
        target_machine
            .states
            .iter()
            .find(|state| state.symbol == call.target_symbol)
    } else {
        target_machine
            .states
            .iter()
            .find(|state| state.name == call.target)
    }?;

    state_has_branching_flow(program, target_machine, target_state, visiting).then_some(target_state)
}

fn type_reference_name(
    type_reference: &omega_checked_trees::types::TypeReference,
) -> omega_checked_trees::name::ProgramName {
    match type_reference {
        omega_checked_trees::types::TypeReference::Reference { referee, .. } => {
            type_reference_name(referee)
        }
        omega_checked_trees::types::TypeReference::Constrained { base_type, .. } => {
            type_reference_name(base_type)
        }
        omega_checked_trees::types::TypeReference::FixedArray { element_type, .. } => {
            type_reference_name(element_type)
        }
        omega_checked_trees::types::TypeReference::Slice { element_type } => {
            type_reference_name(element_type)
        }
        omega_checked_trees::types::TypeReference::Generic { base_name, .. } => base_name.clone(),
        omega_checked_trees::types::TypeReference::Named { name, .. } => name.clone(),
        omega_checked_trees::types::TypeReference::Unit => {
            omega_checked_trees::name::ProgramName::default()
        }
    }
}

fn type_reference_symbol(
    type_reference: &omega_checked_trees::types::TypeReference,
) -> SymbolHandle {
    match type_reference {
        omega_checked_trees::types::TypeReference::Reference { referee, .. } => {
            type_reference_symbol(referee)
        }
        omega_checked_trees::types::TypeReference::Constrained { base_type, .. } => {
            type_reference_symbol(base_type)
        }
        omega_checked_trees::types::TypeReference::FixedArray { element_type, .. } => {
            type_reference_symbol(element_type)
        }
        omega_checked_trees::types::TypeReference::Slice { element_type } => {
            type_reference_symbol(element_type)
        }
        omega_checked_trees::types::TypeReference::Generic { base_symbol, .. } => *base_symbol,
        omega_checked_trees::types::TypeReference::Named { symbol, .. } => *symbol,
        omega_checked_trees::types::TypeReference::Unit => SymbolHandle::invalid(),
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

    let has_branching_flow = state.statements.iter().any(|statement| match statement {
        Statement::Transition(_) => true,
        Statement::Call(call) => {
            branch_call_target_with_visited(program, current_machine, call, visiting).is_some()
        }
        _ => false,
    });

    visiting.pop();
    has_branching_flow
}
