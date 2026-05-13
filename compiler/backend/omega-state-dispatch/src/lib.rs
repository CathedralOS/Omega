mod context;
mod input;
mod model;

pub use context::StateDispatchContext;
pub use input::{RuntimeStateInput, runtime_state_inputs};
pub use model::{DispatchEdge, DispatchState, StateDispatchPlan};

use omega_checked_trees::statement::TransitionGuard;
use omega_control_flow::StateKey;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_state_graph::{RuntimeFlowPlan, RuntimeTransitionTarget};
use std::sync::Arc;

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
            dispatch_index: dispatch_state.dispatch_index,
            label: dispatch_state.label,
            edges,
        });
    }

    plan
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CollectedDispatchState {
    key: StateKey,
    dispatch_index: u32,
    label: String,
    edges: Vec<DispatchEdge>,
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
            expressions: edge.expressions,
            forms_cycle: edge.forms_cycle,
        })
        .chain(terminal_continuation_edges(context, runtime_state))
        .collect();

    CollectedDispatchState {
        key: runtime_state.key,
        dispatch_index: runtime_state.handle.arena_index(),
        label: dispatch_label(runtime_state.key),
        edges,
    }
}

fn dispatch_label(key: StateKey) -> String {
    format!(
        "omega_state_symbol{}_symbol{}",
        key.machine.arena_index(),
        key.state.arena_index()
    )
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
            expressions: Default::default(),
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
