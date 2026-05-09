mod body;
mod context;
mod layout;
mod model;

pub use context::RuntimeStorageContext;
pub use model::{
    RuntimeFrameSlot, RuntimeStorageBodyInput, RuntimeStoragePlan, RuntimeStorageWrite,
};

use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use std::sync::Arc;

use body::build_runtime_storage_body_plan;

pub fn build_runtime_storage_plan(
    context: RuntimeStorageContext,
    runtime_bodies: &RuntimeDispatchBodyPlan,
) -> RuntimeStoragePlan {
    let workers = WorkerPool::with_available_parallelism();

    build_runtime_storage_plan_with_workers(
        Arc::new(context),
        runtime_storage_body_inputs(runtime_bodies),
        workers.handle(),
    )
}

pub fn build_runtime_storage_plan_with_workers(
    context: Arc<RuntimeStorageContext>,
    body_inputs: Vec<RuntimeStorageBodyInput>,
    workers: WorkerPoolHandle,
) -> RuntimeStoragePlan {
    if body_inputs.is_empty() {
        return RuntimeStoragePlan::default();
    }

    let body_inputs = Arc::new(body_inputs);
    let body_count = body_inputs.len();
    let context_for_bodies = Arc::clone(&context);
    let body_plans = workers.map_ordered(body_count, move |index| {
        let body_input = body_inputs
            .get(index)
            .expect("runtime-storage worker index should be in range");

        build_runtime_storage_body_plan(&context_for_bodies, body_input)
    });

    let mut plan = RuntimeStoragePlan::default();

    for body_plan in body_plans {
        plan.frame_slots.insert_many(
            body_plan
                .frame_slots
                .iter()
                .map(|(_, frame_slot)| frame_slot.clone()),
        );
        plan.writes
            .insert_many(body_plan.writes.iter().map(|(_, write)| write.clone()));
    }

    plan
}

pub fn runtime_storage_body_inputs(
    runtime_bodies: &RuntimeDispatchBodyPlan,
) -> Vec<RuntimeStorageBodyInput> {
    runtime_bodies
        .bodies
        .iter()
        .map(|(_, body)| RuntimeStorageBodyInput {
            body: body.clone(),
            operations: runtime_bodies
                .operations
                .span(body.operations)
                .unwrap_or(&[])
                .to_vec(),
        })
        .collect()
}

pub fn runtime_frame_storage_size(plan: &RuntimeStoragePlan) -> usize {
    plan.frame_slots
        .iter()
        .map(|(_, slot)| slot.byte_offset + slot.byte_size)
        .max()
        .unwrap_or(0)
}

pub fn runtime_frame_storage_alignment(plan: &RuntimeStoragePlan) -> usize {
    plan.frame_slots
        .iter()
        .map(|(_, slot)| slot.alignment)
        .max()
        .unwrap_or(1)
}
