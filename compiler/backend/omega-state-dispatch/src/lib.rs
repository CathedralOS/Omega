mod context;
mod input;
mod model;

pub use context::StateDispatchContext;
pub use input::{runtime_state_inputs, RuntimeStateInput};
pub use model::{DispatchEdge, DispatchState, StateDispatchPlan};

use omega_control_flow::StateKey;
use omega_core::arena::Arena;
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
        let edges = plan
            .edges
            .insert_many(dispatch_state.edges.iter().map(|(_, edge)| edge.clone()));

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
    edges: Arena<DispatchEdge>,
}

fn build_dispatch_state(
    context: &StateDispatchContext,
    runtime_state: &RuntimeStateInput,
) -> CollectedDispatchState {
    let mut edges = Arena::new();

    for edge in context
        .edges
        .iter()
        .filter(|edge| edge.from == runtime_state.key)
    {
        edges.insert(DispatchEdge {
            statement_index: edge.statement_index,
            target_dispatch_index: target_dispatch_index(context, &edge.target),
            target: edge.target.clone(),
            continuation_dispatch_index: target_dispatch_index(context, &edge.continuation),
            continuation: edge.continuation.clone(),
            expressions: edge.expressions,
            forms_cycle: edge.forms_cycle,
        });
    }

    append_terminal_continuation_edges(context, runtime_state, &mut edges);

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

fn append_terminal_continuation_edges(
    context: &StateDispatchContext,
    runtime_state: &RuntimeStateInput,
    edges: &mut Arena<DispatchEdge>,
) {
    let has_outgoing_edges = context
        .edges
        .iter()
        .any(|edge| edge.from == runtime_state.key);
    if has_outgoing_edges {
        return;
    }

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
        if edges.iter().any(|(_, existing)| {
            existing.continuation_dispatch_index
                == target_dispatch_index(context, &edge.continuation)
        }) {
            continue;
        }

        edges.insert(DispatchEdge {
            statement_index: 0,
            target_dispatch_index: target_dispatch_index(context, &edge.continuation),
            target: edge.continuation.clone(),
            continuation_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            expressions: Default::default(),
            forms_cycle: false,
        });
    }
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
