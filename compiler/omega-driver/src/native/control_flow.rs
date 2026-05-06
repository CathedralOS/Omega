use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::machine::Machine;
use crate::ir::state::State;
use crate::ir::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowPlan {
    pub machines: Vec<MachineFlow>,
    pub states: Arena<StateFlow>,
    pub operations: Arena<Operation>,
    pub transitions: Arena<TransitionFlow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFlow {
    pub name: String,
    pub states: HandleSpan<StateFlow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFlow {
    pub name: String,
    pub index: usize,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<TransitionFlow>,
}

impl Default for StateFlow {
    fn default() -> Self {
        Self {
            name: String::new(),
            index: 0,
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
    Assignment,
    Call,
    Expression,
    LocalData,
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
    State { index: usize, name: String },
    Nested { receiver: String, state: String },
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
    let mut control_flow = ControlFlowPlan::default();

    for machine in &program.machines {
        let machine_flow = build_machine_flow(machine, &mut control_flow)?;
        control_flow.machines.push(machine_flow);
    }

    Ok(control_flow)
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
        .map(|(index, segment)| (segment.name.clone(), index))
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
                operations,
                transitions,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let states = control_flow.states.insert_many(states);

    Ok(MachineFlow {
        name: machine.name.clone(),
        states,
    })
}

#[derive(Debug, Clone)]
struct StateSegment<'program> {
    name: String,
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
        operations,
        transitions,
        next_segment_name: None,
    });

    let next_names = segments
        .iter()
        .skip(1)
        .map(|segment| segment.name.clone())
        .collect::<Vec<_>>();

    for (segment, next_name) in segments.iter_mut().zip(next_names.into_iter()) {
        segment.next_segment_name = Some(next_name);
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

fn operation_kind(statement: &Statement) -> OperationKind {
    match statement {
        Statement::Assignment(_) => OperationKind::Assignment,
        Statement::Call(_) => OperationKind::Call,
        Statement::Expression(_) => OperationKind::Expression,
        Statement::LocalData(_) => OperationKind::LocalData,
        Statement::Transition(_) => unreachable!("transitions are not operations"),
    }
}

fn segment_has_unconditional_transition(segment: &StateSegment<'_>) -> bool {
    segment
        .transitions
        .iter()
        .any(|transition| transition.guard == TransitionGuard::Always)
}

fn plan_transition(
    state_indexes: &[(String, usize)],
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
    state_indexes: &[(String, usize)],
    target: &TransitionTarget,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    match target {
        TransitionTarget::Named { path, .. }
            if path.len() == 1 || path.len() == 2 && path[0] == "self" =>
        {
            let name = path.last().expect("named transition has a state").clone();
            let index = state_indexes
                .iter()
                .find(|(state_name, _)| state_name == &name)
                .map(|(_, index)| *index)
                .ok_or_else(|| {
                    Diagnostic::error(format!("unknown state transition target `{name}`"))
                })?;

            Ok(PlannedTransitionTarget::State { index, name })
        }
        TransitionTarget::Named { path, .. } if path.len() == 2 => {
            Ok(PlannedTransitionTarget::Nested {
                receiver: path[0].clone(),
                state: path[1].clone(),
            })
        }
        TransitionTarget::Named { path, .. } => Err(Diagnostic::error(format!(
            "unsupported transition target `{}`",
            path.join(".")
        ))),
        TransitionTarget::SelfTarget => Ok(PlannedTransitionTarget::SelfTarget),
        TransitionTarget::Terminal => Ok(PlannedTransitionTarget::Terminal),
    }
}
