mod collection;
mod context;
mod lookups;
mod model;

use collection::build_dispatch_body;
pub use context::RuntimeDispatchBodyContext;
pub use model::{
    RuntimeDispatchBody, RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind,
    RuntimeDispatchBodyPlan,
};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_state_dispatch::DispatchState;
use std::sync::Arc;

pub fn build_runtime_dispatch_body_plan(
    context: RuntimeDispatchBodyContext,
    dispatch_states: Vec<DispatchState>,
) -> RuntimeDispatchBodyPlan {
    let workers = WorkerPool::with_available_parallelism();

    build_runtime_dispatch_body_plan_with_workers(
        Arc::new(context),
        dispatch_states,
        workers.handle(),
    )
}

pub fn build_runtime_dispatch_body_plan_with_workers(
    context: Arc<RuntimeDispatchBodyContext>,
    dispatch_states: Vec<DispatchState>,
    workers: WorkerPoolHandle,
) -> RuntimeDispatchBodyPlan {
    if dispatch_states.is_empty() {
        return RuntimeDispatchBodyPlan::default();
    }

    let dispatch_states = Arc::new(dispatch_states);
    let state_count = dispatch_states.len();
    let context_for_bodies = Arc::clone(&context);
    let collected_bodies = workers.map_ordered(state_count, move |index| {
        let dispatch_state = dispatch_states
            .get(index)
            .expect("runtime-body worker index should be in range");

        build_dispatch_body(&context_for_bodies, dispatch_state)
    });

    let mut plan = RuntimeDispatchBodyPlan::default();

    for collected_body in collected_bodies {
        let operations = plan.operations.insert_many(collected_body.operations);

        plan.bodies.insert(RuntimeDispatchBody {
            key: collected_body.key,
            dispatch_index: collected_body.dispatch_index,
            operations,
        });
    }

    plan
}
