use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::machine::Machine;
use crate::ir::statement::{Statement, Transition, TransitionTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlFlowPlan {
    pub machines: Vec<MachineFlow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFlow {
    pub name: String,
    pub states: Vec<StateFlow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFlow {
    pub name: String,
    pub index: usize,
    pub operations: Vec<Operation>,
    pub transitions: Vec<TransitionFlow>,
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
    LocalData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionFlow {
    pub target: PlannedTransitionTarget,
    pub continuation: Option<PlannedTransitionTarget>,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedTransitionTarget {
    State { index: usize, name: String },
    Nested { receiver: String, state: String },
    SelfTarget,
    Terminal,
}

pub fn build_control_flow_plan(program: &Program) -> Result<ControlFlowPlan, Diagnostic> {
    let machines = program
        .machines
        .iter()
        .map(build_machine_flow)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ControlFlowPlan { machines })
}

fn build_machine_flow(machine: &Machine) -> Result<MachineFlow, Diagnostic> {
    let state_indexes = machine
        .states
        .iter()
        .enumerate()
        .map(|(index, state)| (state.name.as_str(), index))
        .collect::<Vec<_>>();

    let states = machine
        .states
        .iter()
        .enumerate()
        .map(|(index, state)| {
            let (operations, transitions) =
                split_state_statements(machine, &state_indexes, &state.statements)?;

            Ok(StateFlow {
                name: state.name.clone(),
                index,
                operations,
                transitions,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    Ok(MachineFlow {
        name: machine.name.clone(),
        states,
    })
}

fn split_state_statements(
    machine: &Machine,
    state_indexes: &[(&str, usize)],
    statements: &[Statement],
) -> Result<(Vec<Operation>, Vec<TransitionFlow>), Diagnostic> {
    let mut operations = Vec::new();
    let mut transitions = Vec::new();
    let mut transition_section_started = false;

    for (statement_index, statement) in statements.iter().enumerate() {
        match statement {
            Statement::Assignment(_) => {
                if transition_section_started {
                    return Err(non_trailing_transition_error(machine));
                }

                operations.push(Operation {
                    statement_index,
                    kind: OperationKind::Assignment,
                });
            }
            Statement::Call(_) => {
                if transition_section_started {
                    return Err(non_trailing_transition_error(machine));
                }

                operations.push(Operation {
                    statement_index,
                    kind: OperationKind::Call,
                });
            }
            Statement::LocalData(_) => {
                if transition_section_started {
                    return Err(non_trailing_transition_error(machine));
                }

                operations.push(Operation {
                    statement_index,
                    kind: OperationKind::LocalData,
                });
            }
            Statement::Transition(transition) => {
                transition_section_started = true;
                transitions.push(plan_transition(state_indexes, transition)?);
            }
        }
    }

    Ok((operations, transitions))
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
        condition: transition.condition.clone(),
    })
}

fn plan_transition_target(
    state_indexes: &[(&str, usize)],
    target: &TransitionTarget,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    match target {
        TransitionTarget::Named(path) if path.len() == 1 => {
            let name = path[0].clone();
            let index = state_indexes
                .iter()
                .find(|(state_name, _)| *state_name == name)
                .map(|(_, index)| *index)
                .ok_or_else(|| {
                    Diagnostic::error(format!("unknown state transition target `{name}`"))
                })?;

            Ok(PlannedTransitionTarget::State { index, name })
        }
        TransitionTarget::Named(path) if path.len() == 2 => Ok(PlannedTransitionTarget::Nested {
            receiver: path[0].clone(),
            state: path[1].clone(),
        }),
        TransitionTarget::Named(path) => Err(Diagnostic::error(format!(
            "unsupported transition target `{}`",
            path.join(".")
        ))),
        TransitionTarget::SelfTarget => Ok(PlannedTransitionTarget::SelfTarget),
        TransitionTarget::Terminal => Ok(PlannedTransitionTarget::Terminal),
    }
}

fn non_trailing_transition_error(machine: &Machine) -> Diagnostic {
    Diagnostic::error(format!(
        "machine `{}` has executable statements after a transition; transitions must be trailing",
        machine.name
    ))
}
