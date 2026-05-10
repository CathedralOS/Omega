use omega_core::symbols::SymbolHandle;
use omega_typed_program::Program;
use omega_typed_program::expression::{ExpressionHandle, ExpressionTable};
use omega_typed_program::name::ProgramName;
use omega_typed_program::state::State;
use omega_typed_program::statement::{
    Assignment, Statement, StatementNode, TableTransition, Transition, TransitionGuard,
    TransitionGuardNode,
};

use omega_control_flow::{
    ControlFlowPlan, Operation, OperationExpressionRefs, OperationKind, StateKey,
    StateParameterFlow,
};

#[derive(Debug, Clone)]
pub(super) struct StateSegment<'program> {
    pub key: StateKey,
    pub name: ProgramName,
    pub parameters: Vec<StateParameterFlow>,
    pub operations: Vec<Operation>,
    pub transitions: Vec<SegmentTransition<'program>>,
    pub next_segment_name: Option<ProgramName>,
}

#[derive(Debug, Clone)]
pub(super) struct SegmentTransition<'program> {
    pub tree: &'program Transition,
    pub table: TableTransition,
}

pub(super) fn split_state_segments<'program>(
    machine_symbol: SymbolHandle,
    state: &'program State,
    program: &Program,
    control_flow: &mut ControlFlowPlan,
) -> Vec<StateSegment<'program>> {
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
                transitions.push(SegmentTransition {
                    tree: transition,
                    table: *table,
                });
            }
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
                        control_flow,
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
    segment
        .transitions
        .iter()
        .any(|transition| transition.tree.guard == TransitionGuard::Always)
}

fn segment_name(state_name: &ProgramName, segment_index: usize) -> ProgramName {
    if segment_index == 0 {
        state_name.clone()
    } else {
        ProgramName::generated(format!("{state_name}__segment_{segment_index}"))
    }
}

fn state_parameters_for_segment(state: &State, segment_index: usize) -> Vec<StateParameterFlow> {
    if segment_index > 0 {
        return Vec::new();
    }

    state
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| StateParameterFlow {
            symbol: parameter.symbol,
            name: parameter.name.clone(),
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
    control_flow: &mut ControlFlowPlan,
    statement_table: &omega_typed_program::statement::StatementTable,
) -> OperationExpressionRefs {
    match statement {
        StatementNode::Assignment(assignment) => OperationExpressionRefs::Assignment {
            target: control_flow
                .expressions
                .copy_from(source_expressions, assignment.target),
            value: control_flow
                .expressions
                .copy_from(source_expressions, assignment.value),
        },
        StatementNode::Call(call) => OperationExpressionRefs::Call {
            arguments: copy_statement_expression_span(
                control_flow,
                source_expressions,
                statement_table,
                call.arguments,
            ),
        },
        StatementNode::Expression(expression) => OperationExpressionRefs::Expression(
            control_flow
                .expressions
                .copy_from(source_expressions, *expression),
        ),
        StatementNode::LocalData(_) | StatementNode::Transition(_) => OperationExpressionRefs::None,
    }
}

pub(super) fn copy_statement_expression_span(
    control_flow: &mut ControlFlowPlan,
    source_expressions: &ExpressionTable,
    statement_table: &omega_typed_program::statement::StatementTable,
    expressions: omega_core::arena::HandleSpan<ExpressionHandle>,
) -> omega_core::arena::HandleSpan<ExpressionHandle> {
    let copied = statement_table
        .expression_handles(expressions)
        .iter()
        .map(|expression| {
            control_flow
                .expressions
                .copy_from(source_expressions, *expression)
        })
        .collect::<Vec<_>>();

    control_flow.expressions.insert_expression_handles(copied)
}

fn is_static_assignment(assignment: &Assignment) -> bool {
    use omega_typed_program::expression::Expression;

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
        (omega_typed_program::expression::Expression::Name(path), omega_typed_program::expression::Expression::Integer(_))
            if path.len() == 1
    )
}
