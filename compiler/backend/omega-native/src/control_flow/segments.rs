use omega_core::symbols::SymbolHandle;
use omega_typed_program::name::ProgramName;
use omega_typed_program::state::State;
use omega_typed_program::statement::{Assignment, Statement, Transition, TransitionGuard};

use super::{Operation, OperationKind, StateKey};

#[derive(Debug, Clone)]
pub(super) struct StateSegment<'program> {
    pub key: StateKey,
    pub name: ProgramName,
    pub parameters: Vec<ProgramName>,
    pub operations: Vec<Operation>,
    pub transitions: Vec<&'program Transition>,
    pub next_segment_name: Option<ProgramName>,
}

pub(super) fn split_state_segments<'program>(
    machine_symbol: SymbolHandle,
    state: &'program State,
) -> Vec<StateSegment<'program>> {
    let state_symbol = state.symbol;
    let mut segments = Vec::new();
    let mut operations = Vec::new();
    let mut transitions = Vec::new();
    let mut segment_index = 0usize;
    let mut transition_section_started = false;

    for (statement_index, statement) in state.statements.iter().enumerate() {
        if let Statement::Transition(transition) = statement {
            transition_section_started = true;
            transitions.push(transition);
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
        .any(|transition| transition.guard == TransitionGuard::Always)
}

fn segment_name(state_name: &ProgramName, segment_index: usize) -> ProgramName {
    if segment_index == 0 {
        state_name.clone()
    } else {
        ProgramName::generated(format!("{state_name}__segment_{segment_index}"))
    }
}

fn state_parameters_for_segment(state: &State, segment_index: usize) -> Vec<ProgramName> {
    if segment_index > 0 {
        return Vec::new();
    }

    state
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .map(|parameter| parameter.name.clone())
        .collect()
}

fn operation_kind(statement: &Statement) -> OperationKind {
    match statement {
        Statement::Assignment(assignment) if is_static_assignment(assignment) => {
            OperationKind::StaticAssignment {
                target: assignment.target.clone(),
                value: assignment.value.clone(),
            }
        }
        Statement::Assignment(assignment) if is_constant_integer_assignment(assignment) => {
            OperationKind::ConstantIntegerAssignment
        }
        Statement::Assignment(assignment) => OperationKind::Assignment {
            target: assignment.target.clone(),
            value: assignment.value.clone(),
        },
        Statement::Call(call) => OperationKind::Call {
            receiver: call.receiver.clone(),
            target: call.target.clone(),
            arguments: call.arguments.clone(),
        },
        Statement::Expression(_) => OperationKind::Expression,
        Statement::LocalData(_) => OperationKind::LocalData,
        Statement::Transition(_) => unreachable!("transitions are not operations"),
    }
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

fn is_constant_integer_assignment(assignment: &Assignment) -> bool {
    matches!(
        (&assignment.target, &assignment.value),
        (omega_typed_program::expression::Expression::Name(path), omega_typed_program::expression::Expression::Integer(_))
            if path.len() == 1
    )
}
