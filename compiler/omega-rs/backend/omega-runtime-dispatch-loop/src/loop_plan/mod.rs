mod collection;
mod context;
mod guards;
mod inputs;
mod model;

use collection::{build_runtime_dispatch_loop_case, runtime_dispatch_loop_edges};
pub use context::RuntimeDispatchLoopContext;
pub use model::{
    RuntimeDispatchLoopAction, RuntimeDispatchLoopCase, RuntimeDispatchLoopEdge,
    RuntimeDispatchLoopPlan,
};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use psi_arena::Arena;
use std::sync::Arc;

pub fn build_runtime_dispatch_loop_plan(
    context: RuntimeDispatchLoopContext,
) -> RuntimeDispatchLoopPlan {
    let workers = WorkerPool::with_available_parallelism();

    build_runtime_dispatch_loop_plan_with_workers(Arc::new(context), workers.handle())
}

pub fn build_runtime_dispatch_loop_plan_with_workers(
    context: Arc<RuntimeDispatchLoopContext>,
    workers: WorkerPoolHandle,
) -> RuntimeDispatchLoopPlan {
    let case_capacity = context.state_dispatch.states.len();
    let edge_capacity = context.state_dispatch.edges.len();
    let mut plan = RuntimeDispatchLoopPlan {
        needed: context.needed,
        entry_dispatch_index: context.entry_dispatch_index,
        terminal_dispatch_index: 0,
        cases: Arena::with_capacity(case_capacity),
        edges: Arena::with_capacity(edge_capacity),
    };

    if !plan.needed || context.state_dispatch.states.is_empty() {
        return plan;
    }

    let case_count = context.state_dispatch.states.len();
    let context_for_cases = Arc::clone(&context);
    let cases = workers.map_ordered(case_count, move |index| {
        let dispatch_state = context_for_cases
            .state_dispatch
            .states
            .storage_slice()
            .get(index)
            .expect("runtime-dispatch-loop worker index should be in range");

        build_runtime_dispatch_loop_case(&context_for_cases, dispatch_state)
    });

    for (index, case) in cases.into_iter().enumerate() {
        let dispatch_state = context
            .state_dispatch
            .states
            .storage_slice()
            .get(index)
            .expect("runtime-dispatch-loop case index should be in range");
        let edges = plan
            .edges
            .insert_many(runtime_dispatch_loop_edges(&context, dispatch_state));

        plan.cases.insert(RuntimeDispatchLoopCase {
            key: case.key,
            dispatch_index: case.dispatch_index,
            operation_count: case.operation_count,
            edges,
        });
    }

    plan
}
