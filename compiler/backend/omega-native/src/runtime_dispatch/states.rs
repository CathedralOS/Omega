use crate::control_flow::StateKey;
use crate::runtime_flow::{RuntimeFlowPlan, RuntimeState, RuntimeTransitionTarget};
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::TransitionGuard;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDispatchPlan {
    pub states: Arena<DispatchState>,
    pub edges: Arena<DispatchEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchState {
    pub key: StateKey,
    pub machine: ProgramName,
    pub state: ProgramName,
    pub dispatch_index: u32,
    pub label: String,
    pub edges: HandleSpan<DispatchEdge>,
}

impl Default for DispatchState {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
            machine: ProgramName::default(),
            state: ProgramName::default(),
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
    let workers = WorkerPool::with_available_parallelism();

    build_state_dispatch_plan_with_workers(
        Arc::new(StateDispatchContext::from_runtime_flow(runtime_flow)),
        runtime_state_inputs(runtime_flow),
        workers.handle(),
    )
}

pub fn build_state_dispatch_plan_with_workers(
    context: Arc<StateDispatchContext>,
    runtime_states: Vec<RuntimeStateInput>,
    workers: WorkerPoolHandle,
) -> StateDispatchPlan {
    if runtime_states.is_empty() {
        return StateDispatchPlan::default();
    }

    let runtime_states = Arc::new(runtime_states);
    let state_count = runtime_states.len();
    let context_for_states = Arc::clone(&context);
    let dispatch_states = workers.map_ordered(state_count, move |index| {
        let runtime_state = runtime_states
            .get(index)
            .expect("state-dispatch worker index should be in range");

        build_dispatch_state(&context_for_states, runtime_state)
    });

    let mut plan = StateDispatchPlan::default();

    for dispatch_state in dispatch_states {
        let edges = plan.edges.insert_many(dispatch_state.edges);

        plan.states.insert(DispatchState {
            key: dispatch_state.key,
            machine: dispatch_state.machine,
            state: dispatch_state.state,
            dispatch_index: dispatch_state.dispatch_index,
            label: dispatch_state.label,
            edges,
        });
    }

    plan
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDispatchContext {
    edges: Vec<crate::runtime_flow::RuntimeEdge>,
    targets: Vec<StateDispatchTarget>,
}

impl StateDispatchContext {
    pub fn from_runtime_flow(runtime_flow: &RuntimeFlowPlan) -> Self {
        Self {
            edges: runtime_flow
                .edges
                .iter()
                .map(|(_, edge)| edge.clone())
                .collect(),
            targets: runtime_flow
                .states
                .iter()
                .map(|(handle, state)| StateDispatchTarget {
                    key: state.key,
                    machine: state.machine.clone(),
                    state: state.state.clone(),
                    dispatch_index: handle.arena_index(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct StateDispatchTarget {
    key: StateKey,
    machine: ProgramName,
    state: ProgramName,
    dispatch_index: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStateInput {
    handle: Handle<RuntimeState>,
    key: StateKey,
    machine: ProgramName,
    state: ProgramName,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CollectedDispatchState {
    key: StateKey,
    machine: ProgramName,
    state: ProgramName,
    dispatch_index: u32,
    label: String,
    edges: Vec<DispatchEdge>,
}

pub fn runtime_state_inputs(runtime_flow: &RuntimeFlowPlan) -> Vec<RuntimeStateInput> {
    runtime_flow
        .states
        .iter()
        .map(|(handle, runtime_state)| RuntimeStateInput {
            handle,
            key: runtime_state.key,
            machine: runtime_state.machine.clone(),
            state: runtime_state.state.clone(),
        })
        .collect()
}

fn build_dispatch_state(
    context: &StateDispatchContext,
    runtime_state: &RuntimeStateInput,
) -> CollectedDispatchState {
    let edges = context
        .edges
        .iter()
        .filter(|edge| edge.from == runtime_state.key)
        .map(|edge| DispatchEdge {
            target_dispatch_index: target_dispatch_index(context, &edge.target),
            target: edge.target.clone(),
            continuation_dispatch_index: target_dispatch_index(context, &edge.continuation),
            continuation: edge.continuation.clone(),
            guard: edge.guard.clone(),
            forms_cycle: edge.forms_cycle,
        })
        .chain(terminal_continuation_edges(context, runtime_state))
        .collect();

    CollectedDispatchState {
        key: runtime_state.key,
        machine: runtime_state.machine.clone(),
        state: runtime_state.state.clone(),
        dispatch_index: runtime_state.handle.arena_index(),
        label: dispatch_label(&runtime_state.machine, &runtime_state.state),
        edges,
    }
}

fn terminal_continuation_edges(
    context: &StateDispatchContext,
    runtime_state: &RuntimeStateInput,
) -> Vec<DispatchEdge> {
    let has_outgoing_edges = context
        .edges
        .iter()
        .any(|edge| edge.from == runtime_state.key);
    if has_outgoing_edges {
        return Vec::new();
    }

    let mut edges = Vec::new();
    for edge in &context.edges {
        let RuntimeTransitionTarget::State { key, .. } = &edge.target else {
            continue;
        };
        if key.machine != runtime_state.key.machine {
            continue;
        }
        let RuntimeTransitionTarget::State { .. } = &edge.continuation else {
            continue;
        };
        if edges.iter().any(|existing: &DispatchEdge| {
            existing.continuation_dispatch_index
                == target_dispatch_index(context, &edge.continuation)
        }) {
            continue;
        }

        edges.push(DispatchEdge {
            target_dispatch_index: target_dispatch_index(context, &edge.continuation),
            target: edge.continuation.clone(),
            continuation_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            guard: TransitionGuard::Always,
            forms_cycle: false,
        });
    }

    edges
}

fn target_dispatch_index(context: &StateDispatchContext, target: &RuntimeTransitionTarget) -> u32 {
    let RuntimeTransitionTarget::State { key, .. } = target else {
        return 0;
    };

    context
        .targets
        .iter()
        .find(|target| target.key == *key)
        .map(|target| target.dispatch_index)
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
