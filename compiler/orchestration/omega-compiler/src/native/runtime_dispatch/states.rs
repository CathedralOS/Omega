use crate::native::runtime_flow::{RuntimeFlowPlan, RuntimeTransitionTarget};
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::statement::TransitionGuard;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDispatchPlan {
    pub states: Arena<DispatchState>,
    pub edges: Arena<DispatchEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchState {
    pub machine: String,
    pub state: String,
    pub dispatch_index: u32,
    pub label: String,
    pub edges: HandleSpan<DispatchEdge>,
}

impl Default for DispatchState {
    fn default() -> Self {
        Self {
            machine: String::new(),
            state: String::new(),
            dispatch_index: 0,
            label: String::new(),
            edges: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchEdge {
    pub target: RuntimeTransitionTarget,
    pub target_dispatch_index: u32,
    pub continuation: RuntimeTransitionTarget,
    pub continuation_dispatch_index: u32,
    pub guard: TransitionGuard,
    pub forms_cycle: bool,
}

impl Default for DispatchEdge {
    fn default() -> Self {
        Self {
            target: RuntimeTransitionTarget::None,
            target_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            continuation_dispatch_index: 0,
            guard: TransitionGuard::Always,
            forms_cycle: false,
        }
    }
}

pub fn build_state_dispatch_plan(runtime_flow: &RuntimeFlowPlan) -> StateDispatchPlan {
    let mut plan = StateDispatchPlan::default();

    for (state_handle, runtime_state) in runtime_flow.states.iter() {
        let dispatch_edges = runtime_flow
            .edges
            .iter()
            .filter(|(_, edge)| {
                edge.from_machine == runtime_state.machine && edge.from_state == runtime_state.state
            })
            .map(|(_, edge)| DispatchEdge {
                target_dispatch_index: target_dispatch_index(runtime_flow, &edge.target),
                target: edge.target.clone(),
                continuation_dispatch_index: target_dispatch_index(
                    runtime_flow,
                    &edge.continuation,
                ),
                continuation: edge.continuation.clone(),
                guard: edge.guard.clone(),
                forms_cycle: edge.forms_cycle,
            })
            .chain(terminal_continuation_edges(runtime_flow, runtime_state))
            .collect::<Vec<_>>();
        let edges = plan.edges.insert_many(dispatch_edges);

        plan.states.insert(DispatchState {
            machine: runtime_state.machine.clone(),
            state: runtime_state.state.clone(),
            dispatch_index: state_handle.arena_index(),
            label: dispatch_label(&runtime_state.machine, &runtime_state.state),
            edges,
        });
    }

    plan
}

fn terminal_continuation_edges(
    runtime_flow: &RuntimeFlowPlan,
    runtime_state: &crate::native::runtime_flow::RuntimeState,
) -> Vec<DispatchEdge> {
    let has_outgoing_edges = runtime_flow.edges.iter().any(|(_, edge)| {
        edge.from_machine == runtime_state.machine && edge.from_state == runtime_state.state
    });
    if has_outgoing_edges {
        return Vec::new();
    }

    let mut edges = Vec::new();
    for (_, edge) in runtime_flow.edges.iter() {
        let RuntimeTransitionTarget::State { machine, .. } = &edge.target else {
            continue;
        };
        if machine != &runtime_state.machine {
            continue;
        }
        let RuntimeTransitionTarget::State { .. } = &edge.continuation else {
            continue;
        };
        if edges.iter().any(|existing: &DispatchEdge| {
            existing.continuation_dispatch_index
                == target_dispatch_index(runtime_flow, &edge.continuation)
        }) {
            continue;
        }

        edges.push(DispatchEdge {
            target_dispatch_index: target_dispatch_index(runtime_flow, &edge.continuation),
            target: edge.continuation.clone(),
            continuation_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            guard: TransitionGuard::Always,
            forms_cycle: false,
        });
    }

    edges
}

fn target_dispatch_index(runtime_flow: &RuntimeFlowPlan, target: &RuntimeTransitionTarget) -> u32 {
    let RuntimeTransitionTarget::State { machine, state } = target else {
        return 0;
    };

    runtime_flow
        .states
        .iter()
        .find(|(_, runtime_state)| {
            runtime_state.machine == *machine && runtime_state.state == *state
        })
        .map(|(handle, _)| handle.arena_index())
        .unwrap_or(0)
}

fn dispatch_label(machine: &str, state: &str) -> String {
    let mut label = String::from("omega_state_");
    label.push_str(&sanitize_label_part(machine));
    label.push('_');
    label.push_str(&sanitize_label_part(state));
    label
}

fn sanitize_label_part(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}
