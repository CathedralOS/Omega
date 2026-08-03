mod context;
mod model;

pub use context::StateDispatchContext;
pub use model::{DispatchEdge, DispatchState, StateDispatchPlan};

use omega_control_flow::StateKey;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_state_graph::{RuntimeFlowPlan, RuntimeTransitionTarget};
use psi_arena::Arena;
use std::sync::Arc;

pub fn build_state_dispatch_plan(runtime_flow: &RuntimeFlowPlan) -> StateDispatchPlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_dispatch_plan_with_workers(
        Arc::new(StateDispatchContext::from_runtime_flow(Arc::new(
            runtime_flow.clone(),
        ))),
        workers.handle(),
    )
}

pub fn build_state_dispatch_plan_with_workers(
    context: Arc<StateDispatchContext>,
    workers: WorkerPoolHandle,
) -> StateDispatchPlan {
    if context.targets.is_empty() {
        return StateDispatchPlan::default();
    }

    let state_count = context.targets.len();
    let context_for_states = Arc::clone(&context);
    let dispatch_states = workers.map_ordered(state_count, move |index| {
        let dispatch_target = context_for_states
            .targets
            .storage_slice()
            .get(index)
            .expect("state-dispatch worker index should be in range");

        build_dispatch_state(&context_for_states, dispatch_target)
    });

    let mut plan = StateDispatchPlan {
        states: Arena::with_capacity(state_count),
        edges: Arena::with_capacity(context.runtime_flow.edges.len()),
    };

    for dispatch_state in dispatch_states {
        let edges = plan.edges.insert_many(dispatch_state.edges.into_items());

        plan.states.insert(DispatchState {
            key: dispatch_state.key,
            dispatch_index: dispatch_state.dispatch_index,
            edges,
        });
    }

    plan
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CollectedDispatchState {
    key: StateKey,
    dispatch_index: u32,
    edges: Arena<DispatchEdge>,
}

fn build_dispatch_state(
    context: &StateDispatchContext,
    dispatch_target: &context::StateDispatchTarget,
) -> CollectedDispatchState {
    let mut edges = Arena::with_capacity(dispatch_edge_capacity(context, dispatch_target));

    for edge in context
        .runtime_flow
        .edges
        .iter()
        .map(|(_, edge)| edge)
        .filter(|edge| edge_leaves(edge, dispatch_target))
    {
        edges.insert(DispatchEdge {
            statement_index: edge.statement_index,
            target_dispatch_index: target_dispatch_index(context, &edge.target),
            target: edge.target,
            continuation_dispatch_index: target_dispatch_index(context, &edge.continuation),
            continuation: edge.continuation,
            expressions: edge.expressions,
            forms_cycle: edge.forms_cycle,
            call_result: edge.call_result,
        });
    }

    append_terminal_continuation_edges(context, dispatch_target, &mut edges);

    CollectedDispatchState {
        key: dispatch_target.key,
        dispatch_index: dispatch_target.dispatch_index,
        edges,
    }
}

pub fn state_dispatch_label(key: StateKey) -> String {
    format!(
        "omega_state_symbol{}_symbol{}",
        key.machine.arena_index(),
        key.state.arena_index()
    )
}

/// Whether `edge` leaves the specific clone `dispatch_target` represents -- both
/// the source state and its call-context must match, so clones of one source
/// state do not steal each other's edges.
fn edge_leaves(
    edge: &omega_state_graph::RuntimeEdge,
    dispatch_target: &context::StateDispatchTarget,
) -> bool {
    edge.from == dispatch_target.key && edge.from_context == dispatch_target.context
}

fn append_terminal_continuation_edges(
    context: &StateDispatchContext,
    dispatch_target: &context::StateDispatchTarget,
    edges: &mut Arena<DispatchEdge>,
) {
    let has_outgoing_edges = context
        .runtime_flow
        .edges
        .iter()
        .map(|(_, edge)| edge)
        .any(|edge| edge_leaves(edge, dispatch_target));
    if has_outgoing_edges {
        return;
    }

    for (_, edge) in context.runtime_flow.edges.iter() {
        let RuntimeTransitionTarget::State {
            key,
            context: target_context,
        } = &edge.target
        else {
            continue;
        };
        // Only the call edge that ENTERED this clone's context carries this
        // terminal state's return point. Matching the callee context (not just
        // the machine) keeps each clone returning to its own call site.
        if key.machine != dispatch_target.key.machine || *target_context != dispatch_target.context
        {
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
            target: edge.continuation,
            continuation_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            // When this terminal returns into the next chained call, materialize
            // that call's arguments (carried on the entering edge as the
            // continuation arguments) into the callee's parameter slots.
            expressions: omega_control_flow::TransitionExpressionRefs {
                target_arguments: edge.expressions.continuation_arguments,
                ..Default::default()
            },
            forms_cycle: false,
            call_result: None,
        });
    }
}

fn dispatch_edge_capacity(
    context: &StateDispatchContext,
    dispatch_target: &context::StateDispatchTarget,
) -> usize {
    let outgoing_count = context
        .runtime_flow
        .edges
        .iter()
        .map(|(_, edge)| edge)
        .filter(|edge| edge_leaves(edge, dispatch_target))
        .count();
    if outgoing_count > 0 {
        return outgoing_count;
    }

    context
        .runtime_flow
        .edges
        .iter()
        .map(|(_, edge)| edge)
        .filter(|edge| {
            matches!(
                edge.target,
                RuntimeTransitionTarget::State { key, context }
                    if key.machine == dispatch_target.key.machine
                        && context == dispatch_target.context
            )
        })
        .filter(|edge| matches!(edge.continuation, RuntimeTransitionTarget::State { .. }))
        .count()
}

fn target_dispatch_index(context: &StateDispatchContext, target: &RuntimeTransitionTarget) -> u32 {
    let RuntimeTransitionTarget::State {
        key,
        context: target_context,
    } = target
    else {
        return 0;
    };

    context
        .targets
        .iter()
        .map(|(_, target)| target)
        .find(|target| target.key == *key && target.context == *target_context)
        .map(|target| target.dispatch_index)
        .unwrap_or(0)
}
