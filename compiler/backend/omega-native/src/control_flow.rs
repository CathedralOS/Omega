use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::Program;
use omega_typed_program::expression::Expression;
use omega_typed_program::machine::Machine;
use omega_typed_program::state::State;
use omega_typed_program::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowPlan {
    pub machines: Arena<MachineFlow>,
    pub states: Arena<StateFlow>,
    pub operations: Arena<Operation>,
    pub transitions: Arena<TransitionFlow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFlow {
    pub name: String,
    pub contains: Vec<ContainedFlow>,
    pub states: HandleSpan<StateFlow>,
}

impl Default for MachineFlow {
    fn default() -> Self {
        Self {
            name: String::new(),
            contains: Vec::new(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContainedFlow {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFlow {
    pub name: String,
    pub index: usize,
    pub parameters: Vec<String>,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<TransitionFlow>,
}

impl Default for StateFlow {
    fn default() -> Self {
        Self {
            name: String::new(),
            index: 0,
            parameters: Vec::new(),
            operations: HandleSpan::empty(),
            transitions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub statement_index: usize,
    pub kind: OperationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    Assignment {
        target: Expression,
        value: Expression,
    },
    Call {
        receiver: Option<String>,
        target: String,
        arguments: Vec<Expression>,
    },
    ConstantIntegerAssignment,
    Expression,
    LocalData,
    StaticAssignment {
        target: Expression,
        value: Expression,
    },
}

impl Default for Operation {
    fn default() -> Self {
        Self {
            statement_index: 0,
            kind: OperationKind::LocalData,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionFlow {
    pub target: PlannedTransitionTarget,
    pub continuation: Option<PlannedTransitionTarget>,
    pub guard: TransitionGuard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedTransitionTarget {
    State {
        index: usize,
        name: String,
        arguments: Vec<Expression>,
    },
    Nested {
        receiver: String,
        state: String,
        arguments: Vec<Expression>,
    },
    SelfTarget,
    Terminal,
}

impl Default for TransitionFlow {
    fn default() -> Self {
        Self {
            target: PlannedTransitionTarget::Terminal,
            continuation: None,
            guard: TransitionGuard::Always,
        }
    }
}

pub fn build_control_flow_plan(program: &Program) -> Result<ControlFlowPlan, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_control_flow_plan_with_workers(Arc::new(program.clone()), workers.handle())
}

pub fn build_control_flow_plan_with_workers(
    program: Arc<Program>,
    workers: WorkerPoolHandle,
) -> Result<ControlFlowPlan, Diagnostic> {
    if program.machines.is_empty() {
        return Ok(ControlFlowPlan::default());
    }

    let machine_count = program.machines.len();
    let machine_flows = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines
            .get(index)
            .expect("control-flow worker index should be in range");
        let mut local_flow = ControlFlowPlan::default();
        let machine_flow = build_machine_flow(machine, &mut local_flow)?;

        Ok((local_flow, machine_flow))
    });

    let mut control_flow = ControlFlowPlan::default();
    for machine_flow in machine_flows {
        let (local_flow, machine_flow) = machine_flow?;

        merge_machine_flow(&mut control_flow, &local_flow, &machine_flow);
    }

    Ok(control_flow)
}

fn merge_machine_flow(
    target: &mut ControlFlowPlan,
    source: &ControlFlowPlan,
    machine_flow: &MachineFlow,
) {
    let states = source
        .states
        .span_or_empty(machine_flow.states)
        .iter()
        .map(|state| {
            let operations = target.operations.insert_many(
                source
                    .operations
                    .span_or_empty(state.operations)
                    .iter()
                    .cloned(),
            );
            let transitions = target.transitions.insert_many(
                source
                    .transitions
                    .span_or_empty(state.transitions)
                    .iter()
                    .cloned(),
            );

            StateFlow {
                operations,
                transitions,
                ..state.clone()
            }
        });
    let states = target.states.insert_many(states);

    target.machines.insert(MachineFlow {
        states,
        ..machine_flow.clone()
    });
}

fn build_machine_flow(
    machine: &Machine,
    control_flow: &mut ControlFlowPlan,
) -> Result<MachineFlow, Diagnostic> {
    let segments = machine
        .states
        .iter()
        .map(split_state_segments)
        .collect::<Vec<_>>();
    let state_indexes = segments
        .iter()
        .flat_map(|state_segments| state_segments.iter())
        .enumerate()
        .map(|(index, segment)| (segment.name.as_str(), index))
        .collect::<Vec<_>>();

    let states = segments
        .iter()
        .flat_map(|state_segments| state_segments.iter())
        .enumerate()
        .map(|(index, segment)| {
            let mut transitions = segment
                .transitions
                .iter()
                .map(|transition| plan_transition(&state_indexes, transition))
                .collect::<Result<Vec<_>, Diagnostic>>()?;

            if let Some(next_segment_name) = &segment.next_segment_name {
                if !segment_has_unconditional_transition(segment) {
                    let next_index = state_indexes
                        .iter()
                        .find(|(name, _)| name == next_segment_name)
                        .map(|(_, index)| *index)
                        .ok_or_else(|| {
                            Diagnostic::error(format!(
                                "internal control-flow segment `{next_segment_name}` was not indexed"
                            ))
                        })?;

                    transitions.push(TransitionFlow {
                        target: PlannedTransitionTarget::State {
                            index: next_index,
                            name: next_segment_name.clone(),
                            arguments: Vec::new(),
                        },
                        continuation: None,
                        guard: TransitionGuard::Always,
                    });
                }
            }

            let operations = control_flow
                .operations
                .insert_many(segment.operations.iter().cloned());
            let transitions = control_flow.transitions.insert_many(transitions);

            Ok(StateFlow {
                name: segment.name.clone(),
                index,
                parameters: segment.parameters.clone(),
                operations,
                transitions,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let states = control_flow.states.insert_many(states);

    Ok(MachineFlow {
        name: machine.name.clone(),
        contains: machine
            .contains
            .iter()
            .map(|contained| ContainedFlow {
                name: contained.name.clone(),
                type_name: contained.type_name.clone(),
            })
            .collect(),
        states,
    })
}

#[derive(Debug, Clone)]
struct StateSegment<'program> {
    name: String,
    parameters: Vec<String>,
    operations: Vec<Operation>,
    transitions: Vec<&'program Transition>,
    next_segment_name: Option<String>,
}

fn split_state_segments(state: &State) -> Vec<StateSegment<'_>> {
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

fn segment_name(state_name: &str, segment_index: usize) -> String {
    if segment_index == 0 {
        state_name.to_owned()
    } else {
        format!("{state_name}__segment_{segment_index}")
    }
}

fn state_parameters_for_segment(state: &State, segment_index: usize) -> Vec<String> {
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

fn is_static_assignment(assignment: &omega_typed_program::statement::Assignment) -> bool {
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

fn is_constant_integer_assignment(assignment: &omega_typed_program::statement::Assignment) -> bool {
    matches!(
        (&assignment.target, &assignment.value),
        (omega_typed_program::expression::Expression::Name(path), omega_typed_program::expression::Expression::Integer(_))
            if path.len() == 1
    )
}

fn segment_has_unconditional_transition(segment: &StateSegment<'_>) -> bool {
    segment
        .transitions
        .iter()
        .any(|transition| transition.guard == TransitionGuard::Always)
}

fn plan_transition(
    state_indexes: &[(&str, usize)],
    transition: &Transition,
) -> Result<TransitionFlow, Diagnostic> {
    Ok(TransitionFlow {
        target: plan_transition_target(state_indexes, &transition.target)?,
        continuation: transition
            .continuation
            .as_ref()
            .map(|target| plan_transition_target(state_indexes, target))
            .transpose()?,
        guard: transition.guard.clone(),
    })
}

fn plan_transition_target(
    state_indexes: &[(&str, usize)],
    target: &TransitionTarget,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    match target {
        TransitionTarget::Named {
            path, arguments, ..
        } if path.len() == 1 || path.len() == 2 && path[0] == "self" => {
            let name = path.last().expect("named transition has a state").clone();
            let index = state_indexes
                .iter()
                .find(|(state_name, _)| *state_name == name)
                .map(|(_, index)| *index)
                .ok_or_else(|| {
                    Diagnostic::error(format!("unknown state transition target `{name}`"))
                })?;

            Ok(PlannedTransitionTarget::State {
                index,
                name,
                arguments: arguments.clone(),
            })
        }
        TransitionTarget::Named {
            path, arguments, ..
        } if path.len() == 2 => Ok(PlannedTransitionTarget::Nested {
            receiver: path[0].clone(),
            state: path[1].clone(),
            arguments: arguments.clone(),
        }),
        TransitionTarget::Named { path, .. } => Err(Diagnostic::error(format!(
            "unsupported transition target `{}`",
            path.join(".")
        ))),
        TransitionTarget::SelfTarget => Ok(PlannedTransitionTarget::SelfTarget),
        TransitionTarget::Terminal => Ok(PlannedTransitionTarget::Terminal),
    }
}
