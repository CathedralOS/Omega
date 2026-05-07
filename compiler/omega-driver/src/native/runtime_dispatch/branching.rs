use crate::ir::statement::TransitionGuard;
use crate::native::control_flow::{MachineFlow, OperationKind, PlannedTransitionTarget};
use crate::native::plan::NativePlan;
use crate::native::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::native::runtime_flow::RuntimeTransitionTarget;
use crate::native::state_calls::StateCallLowering;
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeBranchingCallPlan {
    pub calls: Arena<RuntimeBranchingCall>,
    pub edges: Arena<RuntimeBranchingCallEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchingCall {
    pub dispatch_index: u32,
    pub source_machine: String,
    pub source_state: String,
    pub statement_index: usize,
    pub target_machine: String,
    pub target_state: String,
    pub argument_count: usize,
    pub edges: HandleSpan<RuntimeBranchingCallEdge>,
}

impl Default for RuntimeBranchingCall {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_machine: String::new(),
            source_state: String::new(),
            statement_index: 0,
            target_machine: String::new(),
            target_state: String::new(),
            argument_count: 0,
            edges: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchingCallEdge {
    pub order: usize,
    pub target: RuntimeTransitionTarget,
    pub continuation: RuntimeTransitionTarget,
    pub guard: TransitionGuard,
    pub lowering: RuntimeBranchTargetLowering,
}

impl Default for RuntimeBranchingCallEdge {
    fn default() -> Self {
        Self {
            order: 0,
            target: RuntimeTransitionTarget::None,
            continuation: RuntimeTransitionTarget::None,
            guard: TransitionGuard::Always,
            lowering: RuntimeBranchTargetLowering::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeBranchTargetLowering {
    Terminal,
    InlineLeaf,
    InlineStraightLine,
    InlineBranching,
    #[default]
    Unknown,
}

pub fn build_runtime_branching_call_plan(native_plan: &NativePlan) -> RuntimeBranchingCallPlan {
    let mut plan = RuntimeBranchingCallPlan::default();

    for (_, body) in native_plan.runtime_bodies.bodies.iter() {
        let Some(operations) = native_plan.runtime_bodies.operations.span(body.operations) else {
            continue;
        };

        for operation in operations {
            let RuntimeDispatchBodyOperationKind::StateCall {
                target_machine,
                target_state,
                argument_count,
                lowering: StateCallLowering::InlineBranching,
            } = &operation.kind
            else {
                continue;
            };

            let branch_edges = build_branch_edges(native_plan, target_machine, target_state);
            let edges = plan.edges.insert_many(branch_edges);
            plan.calls.insert(RuntimeBranchingCall {
                dispatch_index: body.dispatch_index,
                source_machine: operation.source_machine.clone(),
                source_state: operation.source_state.clone(),
                statement_index: operation.statement_index,
                target_machine: target_machine.clone(),
                target_state: target_state.clone(),
                argument_count: *argument_count,
                edges,
            });
        }
    }

    plan
}

fn build_branch_edges(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
) -> Vec<RuntimeBranchingCallEdge> {
    let Some(machine) = machine_flow(native_plan, machine_name) else {
        return Vec::new();
    };
    let Some(state) = native_plan
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.name == state_name))
    else {
        return Vec::new();
    };
    let Some(transitions) = native_plan.control_flow.transitions.span(state.transitions) else {
        return Vec::new();
    };

    transitions
        .iter()
        .enumerate()
        .map(|(order, transition)| {
            let target = runtime_transition_target(machine, state_name, &transition.target);
            RuntimeBranchingCallEdge {
                order,
                lowering: branch_target_lowering(native_plan, &target),
                target,
                continuation: transition
                    .continuation
                    .as_ref()
                    .map(|continuation| {
                        runtime_transition_target(machine, state_name, continuation)
                    })
                    .unwrap_or(RuntimeTransitionTarget::None),
                guard: transition.guard.clone(),
            }
        })
        .collect()
}

fn branch_target_lowering(
    native_plan: &NativePlan,
    target: &RuntimeTransitionTarget,
) -> RuntimeBranchTargetLowering {
    let RuntimeTransitionTarget::State { machine, state } = target else {
        return match target {
            RuntimeTransitionTarget::Terminal | RuntimeTransitionTarget::None => {
                RuntimeBranchTargetLowering::Terminal
            }
            RuntimeTransitionTarget::Unknown { .. } => RuntimeBranchTargetLowering::Unknown,
            RuntimeTransitionTarget::State { .. } => unreachable!(),
        };
    };

    let Some(target_machine) = machine_flow(native_plan, machine) else {
        return RuntimeBranchTargetLowering::Unknown;
    };
    let Some(target_state) = native_plan
        .control_flow
        .states
        .span(target_machine.states)
        .and_then(|states| states.iter().find(|candidate| candidate.name == *state))
    else {
        return RuntimeBranchTargetLowering::Unknown;
    };

    if native_plan
        .control_flow
        .transitions
        .span(target_state.transitions)
        .is_some_and(|transitions| !transitions.is_empty())
    {
        return RuntimeBranchTargetLowering::InlineBranching;
    }

    let has_state_call = native_plan
        .control_flow
        .operations
        .span(target_state.operations)
        .is_some_and(|operations| {
            operations.iter().any(|operation| {
                matches!(operation.kind, OperationKind::Call { .. })
                    && !state_statement_has_host_call(
                        native_plan,
                        machine,
                        state,
                        operation.statement_index,
                    )
            })
        });

    if has_state_call {
        RuntimeBranchTargetLowering::InlineStraightLine
    } else {
        RuntimeBranchTargetLowering::InlineLeaf
    }
}

fn machine_flow<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
) -> Option<&'plan MachineFlow> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .map(|(_, machine)| machine)
}

fn state_statement_has_host_call(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    native_plan.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.machine == machine_name
            && host_call.state == state_name
            && host_call.statement_index == statement_index
    })
}

fn runtime_transition_target(
    machine: &MachineFlow,
    current_state: &str,
    target: &PlannedTransitionTarget,
) -> RuntimeTransitionTarget {
    match target {
        PlannedTransitionTarget::State { name, .. } => RuntimeTransitionTarget::State {
            machine: machine.name.clone(),
            state: name.clone(),
        },
        PlannedTransitionTarget::Nested {
            receiver, state, ..
        } => machine
            .contains
            .iter()
            .find(|contained| contained.name == *receiver)
            .map(|contained| RuntimeTransitionTarget::State {
                machine: contained.type_name.clone(),
                state: state.clone(),
            })
            .unwrap_or_else(|| RuntimeTransitionTarget::Unknown {
                name: format!("{receiver}.{state}"),
            }),
        PlannedTransitionTarget::SelfTarget => RuntimeTransitionTarget::State {
            machine: machine.name.clone(),
            state: current_state.to_owned(),
        },
        PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
    }
}
